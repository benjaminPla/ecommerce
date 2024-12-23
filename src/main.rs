mod controllers;
mod utils;

use actix_files::Files;
use actix_web::{web, App, HttpResponse, HttpServer};
use controllers::{
    add_to_cart, cart, home, not_found, payment, product_details, remove_from_cart, stripe_webhook,
};
use dotenv::dotenv;
use sqlx::{Pool, Postgres};
use tera::Tera;
// use utils::{create_database_pool, populate_database_with_mock_products, setup_database};
use utils::create_database_pool;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let pool: Pool<Postgres> = create_database_pool()
        .await
        .expect("Error creating database pool");

    // setup_database(pool.clone())
    // .await
    // .expect("Error setting up the database");
    // populate_database_with_mock_products(pool.clone())
    // .await
    // .expect("Error populating the database with products");

    let pool_data = web::Data::new(pool);
    let tera = Tera::new("src/html/*").expect("Error initializing Tera");

    HttpServer::new(move || {
        App::new()
            .app_data(pool_data.clone())
            .app_data(web::Data::new(tera.clone()))
            .route("/", web::get().to(home::handler))
            .route(
                "/status",
                web::get().to(|| async { HttpResponse::Ok().body("ok") }),
            )
            .route("/product/{id}", web::get().to(product_details::handler))
            .route("/cart", web::get().to(cart::handler))
            .route("/add_to_cart/{id}", web::post().to(add_to_cart))
            .route("/remove_from_cart/{id}", web::post().to(remove_from_cart))
            .route("/payment", web::get().to(payment))
            .route("/stripe-webhook", web::get().to(stripe_webhook))
            .service(Files::new("/public", "src/public").show_files_listing())
            .default_service(web::route().to(not_found))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
