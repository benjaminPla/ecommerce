use crate::utils;
use actix_web::{web, HttpResponse, Responder};
use serde::Serialize;
use sqlx::{Pool, Postgres, Row};
use tera::{Context, Tera};

#[derive(Serialize)]
struct HomeProduct {
    description: String,
    id: i32,
    image_url: String,
    name: String,
    price: f64,
}

pub async fn handler(pool: web::Data<Pool<Postgres>>, tmpl: web::Data<Tera>) -> impl Responder {
    let query = "SELECT id, name, description, price, image_url FROM products;";
    let rows = match sqlx::query(query).fetch_all(pool.get_ref()).await {
        Ok(rows) => rows,
        Err(err) => {
            eprintln!("Database query failed: {:#?}", err);
            return HttpResponse::InternalServerError().body("Internal Server Error");
        }
    };

    let products: Vec<HomeProduct> = rows
        .into_iter()
        .filter_map(|row| map_row_to_product(&row).ok())
        .collect();

    let mut context = Context::new();
    context.insert("title", "Ecommerce");
    context.insert("products", &products);

    match tmpl.render("index.html", &context) {
        Ok(rendered) => HttpResponse::Ok().body(rendered),
        Err(err) => {
            eprintln!("Template rendering failed: {:#?}", err);
            HttpResponse::InternalServerError().body("Internal Server Error")
        }
    }
}

fn map_row_to_product(row: &sqlx::postgres::PgRow) -> Result<HomeProduct, sqlx::Error> {
    let id: i32 = row.try_get("id")?;
    let name: String = row.try_get("name")?;
    let description: String = row.try_get("description")?;
    let price: f64 = row.try_get("price").map(utils::round_price)?;
    let image_url: String = row.try_get("image_url")?;

    Ok(HomeProduct {
        id,
        name,
        description,
        price,
        image_url,
    })
}
