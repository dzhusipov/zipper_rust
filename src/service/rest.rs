use tokio_util::io::ReaderStream;
use futures_util::TryStreamExt;
use actix_web::rt;
use actix_web::http::header::{ContentDisposition, DispositionParam, DispositionType};
use actix_web::{web, HttpResponse, Result};
use tera::{Tera, Context};

use crate::models::form_data::FormData;
use crate::service::utils::{download_file, archive_file}; // Adjust the path according to your project structure

pub async fn index(tmpl: web::Data<Tera>, error: Option<String>) -> Result<HttpResponse> {
    let mut ctx = Context::new();
    ctx.insert("error", &error);

    let rendered = tmpl.render("index.html", &ctx).map_err(|e| {
        println!("Template error: {}", e);
        actix_web::error::ErrorInternalServerError("Template error")
    })?;

    Ok(HttpResponse::Ok().content_type("text/html").body(rendered))
}

pub  async fn handle_form(
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
