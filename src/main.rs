use actix_web::{web, App, HttpServer};
use tera::Tera;

mod service;
mod models;
use service::rest::{index, handle_form};



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