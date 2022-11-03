use actix_web::{get, web, Error, HttpResponse};
use anyhow::Result;

use crate::segment::{Segment, SegmentFilter};
use crate::Pool;

#[get("/api/skipSegments/{hash_prefix}")]
pub async fn segments_by_hash(
    hash_prefix: web::Path<String>,
    query: web::Query<SegmentFilter>,
    data: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let pool = data.clone();
    let conn = web::block(move || pool.get())
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let segments =
        web::block(move || Segment::get_by_hash(conn, hash_prefix.to_string(), query.into_inner()))
            .await?
            .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(segments))
}
