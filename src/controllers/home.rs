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

fn map_row_to_product(row: &sqlx::postgres::PgRow) -> Result<HomeProduct, String> {
    let description: String = row
        .try_get("description")
        .map_err(|_| "Error getting `description`")?;
    let id: i32 = row.try_get("id").map_err(|_| "Error getting `id`")?;
    let image_url: String = row
        .try_get("image_url")
        .map_err(|_| "Error getting `image_url`")?;
    let name: String = row.try_get("name").map_err(|_| "Error getting `name`")?;
    let price: f64 = row
        .try_get("price")
        .map(utils::round_price)
        .map_err(|_| "Error getting `price`")?;

    Ok(HomeProduct {
        description,
        id,
        image_url,
        name,
        price,
    })
}

pub async fn handler(pool: web::Data<Pool<Postgres>>, tmpl: web::Data<Tera>) -> impl Responder {
    let query = "SELECT id, name, description, price, image_url FROM products;";
    let rows = match sqlx::query(query).fetch_all(pool.get_ref()).await {
        Ok(rows) => rows,
        Err(err) => {
            eprintln!("Database query error: {:#?}", err);
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

    utils::render_template(&tmpl, "index.html", &context)
}
