use actix_web::{web, HttpRequest, HttpResponse, Responder};
use futures_channel::mpsc::unbounded;
use indicatif::ProgressBar;
use reqwest::Client;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tokio::task;
use zip::write::FileOptions;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use sanitize_filename::sanitize;

use crate::models::form_data::AppState;

pub async fn progress(_req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    // Create a channel for sending progress updates
    let (tx, rx) = unbounded::<String>();

    // Add the sender to the list
    data.progress_senders.lock().unwrap().push(tx);

    // Create an SSE response
    let stream = rx.map(|data| {
        Ok::<_, actix_web::Error>(web::Bytes::from(format!("data: {}\n\n", data)))
    });

    HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .streaming(stream)
}

pub async fn download_file(url: &str, data: web::Data<AppState>) -> io::Result<(PathBuf, PathBuf)> {
    let client = Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Get the final URL after redirects
    let final_url = response.url().clone();

    // Extract the filename from the final URL path
    let filename = match Path::new(final_url.path())
        .file_name()
        .and_then(|name| name.to_str())
    {
        Some(name) => name.to_string(),
        None => "downloaded_file".to_string(),
    };

    // Sanitize the filename
    let filename = sanitize(&filename);

    // Define file paths
    let file_path = PathBuf::from(&filename);
    let archive_filename = format!("{}.zip", filename);
    let archive_path = PathBuf::from(&archive_filename);

    // Create the file to write to
    let mut file = tokio::fs::File::create(&file_path).await?;

    let total_size = response.content_length().unwrap_or(0);
    let progress = ProgressBar::new(total_size);

    let mut downloaded: u64 = 0;

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        progress.inc(chunk.len() as u64);

        // Send progress update to all connected clients
        let percentage = if total_size > 0 {
            (downloaded as f64 / total_size as f64) * 100.0
        } else {
            0.0
        };
        let percentage = percentage.min(100.0).max(0.0); // Clamp between 0 and 100
        let message = format!("{:.2}", percentage);
        let senders = data.progress_senders.lock().unwrap().clone();
        for sender in senders {
            let _ = sender.unbounded_send(message.clone());
        }
    }

    progress.finish_with_message("Download complete");

    // Clear the progress senders after download
    data.progress_senders.lock().unwrap().clear();

    Ok((file_path, archive_path))
}

pub async fn archive_file(file_path: &Path, archive_path: &Path) -> io::Result<()> {
    // Use spawn_blocking for CPU-bound task
    let file_path = file_path.to_owned();
    let archive_path = archive_path.to_owned();

    task::spawn_blocking(move || {
        let file = std::fs::File::open(&file_path)?;
        let mut zip_file = std::fs::File::create(&archive_path)?;
        let mut zip = zip::ZipWriter::new(&mut zip_file);

        zip.start_file(
            file_path
                .file_name()
                .unwrap()
                .to_string_lossy(),
            FileOptions::default(),
        )?;
        let mut buffer = Vec::new();
        let mut source = file;
        source.read_to_end(&mut buffer)?;
        zip.write_all(&buffer)?;
        zip.finish()?;

        Ok(())
    })
    .await
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
}