use actix_web::{web, App, HttpResponse, HttpServer};
use indicatif::ProgressBar;
use reqwest::Client;
use std::io::{self, Read, Write};
use std::path::Path;
use tokio::task;
use zip::write::FileOptions;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use actix_web::http::header::{ContentDisposition, DispositionParam, DispositionType};
use tokio_util::io::ReaderStream;
use actix_web::rt;
use tokio::io::AsyncWriteExt; // <-- Add this line

#[derive(serde::Deserialize)]
struct DownloadRequest {
    url: String,
}

async fn download_file(url: String, file_path: String) -> io::Result<()> {
    let client = Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let total_size = response.content_length().unwrap_or(0);
    let progress = ProgressBar::new(total_size);

    // Create the file to write to
    let mut file = tokio::fs::File::create(&file_path).await?;

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        file.write_all(&chunk).await?; // This method is now in scope
        progress.inc(chunk.len() as u64);
    }

    progress.finish_with_message("Download complete");
    Ok(())
}

async fn archive_file(file_path: String, archive_path: String) -> io::Result<()> {
    // Use spawn_blocking for CPU-bound task
    task::spawn_blocking(move || {
        let file = std::fs::File::open(&file_path)?;
        let mut zip_file = std::fs::File::create(&archive_path)?;
        let mut zip = zip::ZipWriter::new(&mut zip_file);

        zip.start_file(
            Path::new(&file_path)
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

async fn handle_download(req: web::Query<DownloadRequest>) -> actix_web::Result<HttpResponse> {
    let url = req.url.clone();
    let file_path = "downloaded_file".to_string();
    let archive_path = "archive.zip".to_string();

    // Download and archive the file
    if let Err(e) = download_file(url, file_path.clone()).await {
        eprintln!("Failed to download file: {:?}", e);
        return Ok(HttpResponse::InternalServerError().body("Failed to download file"));
    }
    if let Err(e) = archive_file(file_path.clone(), archive_path.clone()).await {
        eprintln!("Failed to archive file: {:?}", e);
        return Ok(HttpResponse::InternalServerError().body("Failed to archive file"));
    }

    // Stream the archived file back to the user
    let file = tokio::fs::File::open(&archive_path).await?;
    let stream = ReaderStream::new(file).map_err(|e| actix_web::Error::from(e));

    // Schedule deletion of the files after sending the response
    let file_paths = vec![file_path.clone(), archive_path.clone()];
    rt::spawn(async move {
        // Wait a bit to ensure the response is sent
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        for path in file_paths {
            let _ = tokio::fs::remove_file(path).await;
        }
    });

    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", "application/zip"))
        .insert_header(ContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![DispositionParam::Filename(String::from("archive.zip"))],
        })
        .streaming(stream))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    log4rs::init_file("config/log4rs.yml", Default::default()).unwrap();
    
    HttpServer::new(|| App::new().route("/download", web::get().to(handle_download)))
        .bind("0.0.0.0:8081")?
        .run()
        .await
}