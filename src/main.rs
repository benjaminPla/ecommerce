mod utils;

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use std::sync::Mutex;
use tera::{Context, Tera};
use utils::{create_database_pool, setup_database};
use sqlx::{Pool, Postgres};
use dotenv::dotenv;
// use serde::Serialize;

// #[derive(Serialize)]
struct AppState {
    count: Mutex<u8>,
}

async fn home(tmpl: web::Data<Tera>, data: web::Data<AppState>) -> impl Responder {
    let count = data.count.lock().unwrap();
    let count_value = *count;

    let mut context = Context::new();
    context.insert("title", "Ecommerce");
    context.insert("content", "Testing Rust + HTMLX");
    context.insert("count", &count_value);

    match tmpl.render("index.html", &context) {
        Ok(rendered) => HttpResponse::Ok().body(rendered),
        Err(err) => {
            eprintln!("{:#?}", err);
            HttpResponse::InternalServerError().body("Error rendering template")
        }
    }
}

async fn not_found(tmpl: web::Data<Tera>) -> impl Responder {
    let mut context = Context::new();
    context.insert("title", "Ecommerce - Not Found");

    match tmpl.render("404.html", &context) {
        Ok(rendered) => HttpResponse::Ok().body(rendered),
        Err(err) => {
            eprintln!("{:#?}", err);
            HttpResponse::InternalServerError().body("Error rendering template")
        }
    }
}

async fn count(data: web::Data<AppState>) -> impl Responder {
    let mut count = data.count.lock().unwrap();
    *count += 1;

    HttpResponse::Ok().body(format!("Count: {}", count))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let pool:Pool<Postgres> = create_database_pool().await.expect("Error creating database pool");
    setup_database(pool.clone()).await.expect("Error setting up the database");

    let tera = Tera::new("src/templates/*").expect("Error initializing Tera");

    let app_state = web::Data::new(AppState {
        count: Mutex::new(0),
    });

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .app_data(web::Data::new(tera.clone()))
            .route("/", web::get().to(home))
            .route("/count", web::post().to(count))
            .route(
                "/status",
                web::get().to(|| async { HttpResponse::Ok().body("ok") }),
            )
            .default_service(web::route().to(not_found))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
