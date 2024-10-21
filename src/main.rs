use actix_web::{web, App, HttpResponse, HttpServer, Result};
use indicatif::ProgressBar;
use reqwest::Client;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tokio::task;
use zip::write::FileOptions;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use actix_web::http::header::{ContentDisposition, DispositionParam, DispositionType};
use tokio_util::io::ReaderStream;
use actix_web::rt;
use tokio::io::AsyncWriteExt;
use tera::{Tera, Context};
use serde::Deserialize;
use sanitize_filename::sanitize;

#[derive(Deserialize)]
struct FormData {
    url: String,
}

async fn download_file(url: &str) -> io::Result<(PathBuf, PathBuf)> {
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

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        file.write_all(&chunk).await?;
        progress.inc(chunk.len() as u64);
    }

    progress.finish_with_message("Download complete");

    Ok((file_path, archive_path))
}

async fn archive_file(file_path: &Path, archive_path: &Path) -> io::Result<()> {
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

async fn index(tmpl: web::Data<Tera>, error: Option<String>) -> Result<HttpResponse> {
    let mut ctx = Context::new();
    ctx.insert("error", &error);

    let rendered = tmpl.render("index.html", &ctx).map_err(|e| {
        println!("Template error: {}", e);
        actix_web::error::ErrorInternalServerError("Template error")
    })?;

    Ok(HttpResponse::Ok().content_type("text/html").body(rendered))
}

async fn handle_form(
    form: web::Form<FormData>,
    tmpl: web::Data<Tera>,
) -> actix_web::Result<HttpResponse> {
    let url = form.url.trim().to_string();

    // Validate the URL
    if url.is_empty() {
        return index(tmpl, Some("URL cannot be empty".to_string())).await;
    }

    // Download the file and get file paths
    let (file_path, archive_path) = match download_file(&url).await {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("Failed to download file: {:?}", e);
            return index(tmpl, Some("Failed to download file".to_string())).await;
        }
    };

    // Archive the file
    if let Err(e) = archive_file(&file_path, &archive_path).await {
        eprintln!("Failed to archive file: {:?}", e);
        return index(tmpl, Some("Failed to archive file".to_string())).await;
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

    // Set the Content-Disposition header to include the correct filename
    let archive_filename = archive_path.file_name().unwrap().to_string_lossy();

    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", "application/zip"))
        .insert_header(ContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![DispositionParam::Filename(archive_filename.into_owned())],
        })
        .streaming(stream))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    log4rs::init_file("config/log4rs.yml", Default::default()).unwrap();
    // Initialize Tera templates
    let tera = Tera::new("templates/**/*").unwrap();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(tera.clone()))
            .route("/", web::get().to(index))
            .route("/", web::post().to(handle_form))
    })
    .bind("0.0.0.0:8119")?
    .run()
    .await
}