use anyhow::Result;
use futures::StreamExt;
use indicatif::ProgressStyle;
use reqwest::header;
use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
};
use tracing::info;

pub async fn download_http() -> Result<String> {
    let download_dir = env::var("DATA_PATH").unwrap_or_else(|_| "/data".to_string());
    let attribution_path = format!("{}/LICENSE.md", download_dir);
    let attribution_exists = fs::metadata(&attribution_path).is_ok();
    if !attribution_exists {
        let mut attribution_file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&attribution_path)?;

        attribution_file.write_all(
            b"# Attribution\n\nThis data is provided by [SponsorBlock](https://sponsor.ajay.app/) and is licensed under the [CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/) license.",
        )?;
    }

    let download_path = format!("{}/sponsorTimes.csv", download_dir);
    let etag_path = format!("{}.etag", download_path);
    let cached_stat = fs::metadata(&download_path).ok();

    let url = env::var("CSV_URL")
        .unwrap_or_else(|_| "https://mirror.sb.mchang.xyz/sponsorTimes.csv".to_string());

    info!("downloading sponsor times from {}", url);
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_str(
            format!(
                "sb-mirror (mirror-{}, https://github.com/sylv/sb-mirror)",
                env!("CARGO_PKG_VERSION")
            )
            .as_str(),
        )?,
    );

    if cached_stat.is_some() {
        let etag = fs::read_to_string(&etag_path).ok();
        if let Some(etag) = etag {
            headers.insert(header::IF_NONE_MATCH, header::HeaderValue::from_str(&etag)?);
        }
    }

    let mut using_range = false;
    if let Some(download_metadata) = &cached_stat {
        let range = format!("bytes={}-", download_metadata.len());
        headers.insert(header::RANGE, header::HeaderValue::from_str(&range)?);
        using_range = true;
    }

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .expect("failed to build client");

    let resp = client.get(url).send().await?;
    if resp.status() == 304 || resp.status() == 416 {
        info!(
            "local database is already up to date: {}",
            resp.status().as_str()
        );

        return Ok(download_path);
    }

    let expect_status = if using_range { 206 } else { 200 };
    if !resp.status().is_success()
        || resp.status() != expect_status
        || !resp
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("text/csv")
    {
        info!("error downloading sponsor times CSV: {:?}", resp);
        return Ok(download_path);
    }

    let content_length = resp.content_length().expect("missing content length");
    let etag = resp
        .headers()
        .get("etag")
        .map(|v| v.to_str().unwrap().to_string());

    let pb = indicatif::ProgressBar::new(content_length);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} at {bytes_per_sec} ({eta})",
        )
        .unwrap(),
    );

    let mut out = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&download_path)?;

    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        out.write_all(&chunk)?;
        pb.inc(chunk.len() as u64);
    }

    pb.finish_and_clear();
    out.flush()?;
    if let Some(etag) = etag {
        fs::write(etag_path, etag)?;
    } else {
        fs::remove_file(etag_path)?;
    }

    Ok(download_path)
}
