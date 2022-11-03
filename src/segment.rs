use std::collections::HashMap;

use anyhow::Result;
use rusqlite::params;
use rusqlite::types::Value;
use serde::de;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use tracing::debug;

use crate::Connection;

#[derive(Serialize, Deserialize, Clone)]
pub struct Segment {
    category: String,
    #[serde(rename = "actionType")]
    action_type: String,
    segment: (f64, f64),
    #[serde(rename = "UUID")]
    id: String,
    locked: u8,
    votes: i32,
    #[serde(rename = "videoDuration")]
    video_duration: f64,
    #[serde(rename = "userID")]
    user_id: String,
    description: String,
}

#[derive(Serialize, Deserialize)]
pub struct SegmentFilter {
    #[serde(deserialize_with = "deserialize_json_string")]
    pub categories: Vec<String>,
    pub service: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SegmentHashContainer {
    #[serde(rename = "videoID")]
    video_id: String,
    #[serde(rename = "hash")]
    hash_full: String,
    segments: Vec<Segment>,
}

impl Segment {
    pub fn get_by_hash(
        conn: Connection,
        hash: String,
        filter: SegmentFilter,
    ) -> Result<Vec<SegmentHashContainer>> {
        let service = filter.service.unwrap_or_else(|| "YouTube".to_string());
        let mut stmt = conn.prepare_cached(r#"
            SELECT id, video_id, hash_full, start_time, end_time, category, user_id, votes, action_type, video_duration, locked
            FROM segments
            WHERE hash_prefix = ?
            AND service = ?
            AND category IN rarray(?)
            ORDER BY start_time
        "#)?;

        // https://www.youtube.com/watch?v=ld2nWfIap2k
        let categories = Rc::new(
            filter
                .categories
                .into_iter()
                .map(Value::from)
                .collect::<Vec<Value>>(),
        );

        let mut rows = stmt.query(params![hash, service, categories])?;
        let mut segments = HashMap::new();
        while let Some(row) = rows.next()? {
            let segment = Segment {
                id: row.get(0)?,
                segment: (row.get(3)?, row.get(4)?),
                category: row.get(5)?,
                user_id: row.get(6)?,
                votes: row.get(7)?,
                action_type: row.get(8)?,
                video_duration: row.get(9)?,
                locked: row.get(10)?,
                description: "".to_string(),
            };

            let video_id: String = row.get(1)?;
            let hash_full: String = row.get(2)?;
            let container = segments
                .entry((video_id.clone(), &hash))
                .or_insert_with(|| SegmentHashContainer {
                    segments: Vec::new(),
                    video_id,
                    hash_full,
                });

            container.segments.push(segment);
        }

        debug!("return");
        Ok(segments.into_iter().map(|(_, v)| v).collect())
    }
}

pub fn deserialize_json_string<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct JsonStringVisitor;

    impl<'de> de::Visitor<'de> for JsonStringVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string containing json data")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            serde_json::from_str(v).map_err(E::custom)
        }
    }

    deserializer.deserialize_any(JsonStringVisitor)
}
