use std::{fs, io::BufReader};

use crate::Pool;
use anyhow::Result;
use csv::Position;
use indicatif::ProgressStyle;
use rusqlite::{params, Connection};
use serde::Deserialize;
use tokio::sync::oneshot::Receiver;
use tracing::log::{info, warn};

#[derive(Deserialize, Debug)]
struct Record<'a> {
    #[serde(rename = "UUID")]
    id: &'a str,
    #[serde(rename = "videoID")]
    video_id: &'a str,
    #[serde(rename = "startTime")]
    start_time: f64,
    #[serde(rename = "endTime")]
    end_time: f64,
    #[serde(rename = "category")]
    category: &'a str,
    #[serde(rename = "userID")]
    user_id: &'a str,
    #[serde(rename = "hashedVideoID")]
    hash_full: &'a str,
    votes: i64,
    service: &'a str,
    #[serde(rename = "actionType")]
    action_type: &'a str,
    #[serde(rename = "videoDuration")]
    video_duration: f64,
    #[serde(rename = "locked")]
    locked: u8,
}

fn post_sync(conn: &Connection) -> Result<()> {
    info!("creating indexes and cleaning up, this may also take a while");
    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS segments_by_hash ON segments (hash_prefix, service, category);
        VACUUM;
        PRAGMA optimize;
    "#,
    )?;

    info!("sync complete");
    Ok(())
}

// this was using an async stream direct from the http request instead of caching the response on disk,
// but this wasn't actually faster, was more complicated, and didn't allow for things like
// zstd compressed files.
pub fn sync(csv_path: String, pool: &Pool, kill_rx: &mut Receiver<()>) -> Result<()> {
    info!("syncing segments from {}", csv_path);
    let mut conn = pool.get()?;
    let metadata = fs::metadata(&csv_path).expect("unable to read file");
    let last_offset_path = format!("{}.offset", csv_path);
    let last_offset = fs::read_to_string(&last_offset_path)
        .ok()
        .and_then(|s| s.parse::<u64>().ok());

    let pb = indicatif::ProgressBar::new(metadata.len());
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} at {bytes_per_sec} ({eta})",
        )
        .unwrap(),
    );

    let content = fs::File::open(&csv_path).expect("unable to read file");
    let reader = BufReader::new(content);
    let mut rdr = csv::Reader::from_reader(reader);
    let mut raw_record = csv::StringRecord::new();
    let headers = rdr.headers()?.clone();

    if let Some(last_offset) = last_offset {
        if metadata.len() == last_offset {
            info!("skipping sync, file is unchanged");
            return Ok(());
        }

        if metadata.len() < last_offset {
            panic!("file is smaller than last offset, this shouldn't happen");
        }

        let mut position = Position::new();
        position.set_byte(last_offset - 10000);
        rdr.seek(position)?;
    }

    let tx = conn.transaction()?;

    {
        let mut stmt = tx.prepare_cached(
                r#"
                    INSERT OR REPLACE INTO segments
                        (id, video_id, hash_full, start_time, end_time, category, user_id, votes, service, action_type, video_duration, locked)
                    VALUES
                        (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#
            )?;

        let mut failures = 0;
        loop {
            let result = rdr.read_record(&mut raw_record);
            if result.is_err() {
                if failures > 20 {
                    panic!("too many failures reading records from CSV file. consider deleting the local csv file and starting again.")
                }

                warn!("error reading record: {:?}", result);
                let mut pos = rdr.position().clone();
                pos.set_line(pos.line() + 1);
                rdr.seek(pos)?;
                failures += 1;
                continue;
            }

            if !result.unwrap() {
                break;
            }

            if kill_rx.try_recv().is_ok() {
                return Ok(());
            }

            let record: Record = raw_record.deserialize(Some(&headers))?;
            stmt.execute(params![
                record.id,
                record.video_id,
                record.hash_full,
                record.start_time,
                record.end_time,
                record.category,
                record.user_id,
                record.votes,
                record.service,
                record.action_type,
                record.video_duration,
                record.locked,
            ])?;

            let position = rdr.position();
            if position.line() % 5000 == 0 {
                pb.set_position(position.byte());
            }

            // todo: every 100k records, commit the transaction and store the offset
        }

        pb.finish_and_clear();
    }

    info!("committing transaction, this may take a while");
    tx.commit()?;
    fs::write(&last_offset_path, metadata.len().to_string())?;
    post_sync(&conn)?;
    Ok(())
}
