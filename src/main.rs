mod controllers;
mod utils;

use actix_files::Files;
use actix_web::{web, App, HttpResponse, HttpServer};
use controllers::{add_to_cart, cart, home, not_found, product_details, remove_from_cart};
use dotenv::dotenv;
use sqlx::{Pool, Postgres};
use tera::Tera;
// use utils::{create_database_pool, populate_database_with_mock_products, setup_database};
use utils::{create_database_pool, setup_database};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let pool: Pool<Postgres> = create_database_pool()
        .await
        .expect("Error creating database pool");

    setup_database(pool.clone())
        .await
        .expect("Error setting up the database");
    // populate_database_with_mock_products(pool.clone())
    // .await
    // .expect("Error populating the database with products");

    let pool_data = web::Data::new(pool);
    let tera = Tera::new("src/html/*").expect("Error initializing Tera");

    HttpServer::new(move || {
        App::new()
            .app_data(pool_data.clone())
            .app_data(web::Data::new(tera.clone()))
            .route("/", web::get().to(home))
            .route(
                "/status",
                web::get().to(|| async { HttpResponse::Ok().body("ok") }),
            )
            .route("/product/{id}", web::get().to(product_details))
            .route("/cart", web::get().to(cart))
            .route("/add_to_cart/{id}", web::post().to(add_to_cart))
            .route("/remove_from_cart/{id}", web::post().to(remove_from_cart))
            .service(Files::new("/styles", "src/styles").show_files_listing())
            .default_service(web::route().to(not_found))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
