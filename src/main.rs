use actix_web::{web, App, HttpServer};
use models::form_data::AppState;
use service::{rest::{handle_form, index}, utils::progress};
use tera::Tera;

use std::sync::{Arc, Mutex};

mod models;
mod service;


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    log4rs::init_file("config/log4rs.yml", Default::default()).unwrap();
    // Initialize Tera templates and wrap in `web::Data`
    let tera = web::Data::new(Tera::new("templates/**/*").unwrap());

    // Initialize the shared state
    let app_state = web::Data::new(AppState {
        progress_senders: Arc::new(Mutex::new(Vec::new())),
    });

    HttpServer::new(move || {
        App::new()
            .app_data(tera.clone())
            .app_data(app_state.clone())
            .route("/", web::get().to(index))
            .route("/", web::post().to(handle_form))
            .route("/progress", web::get().to(progress))
    })
    .bind("0.0.0.0:8119")?
    .run()
    .await
}