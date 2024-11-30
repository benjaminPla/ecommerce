use crate::utils;
use actix_web::{web, HttpResponse, Responder};
use serde::Serialize;
use sqlx::{Pool, Postgres, Row};
use tera::{Context, Tera};

#[derive(Serialize)]
struct DetailsProduct {
    category: String,
    description: String,
    id: i32,
    image_url: String,
    name: String,
    price: f64,
    stock_quantity: i32,
}

async fn map_row_to_product(row: sqlx::postgres::PgRow) -> Result<DetailsProduct, String> {
    let category: String = row
        .try_get("category")
        .map_err(|_| "Error getting `category`".to_string())?;
    let description: String = row
        .try_get("description")
        .map_err(|_| "Error getting `description`".to_string())?;
    let id: i32 = row
        .try_get("id")
        .map_err(|_| "Error getting `id`".to_string())?;
    let image_url: String = row
        .try_get("image_url")
        .map_err(|_| "Error getting `image_url`".to_string())?;
    let name: String = row
        .try_get("name")
        .map_err(|_| "Error getting `name`".to_string())?;
    let price: f64 = row
        .try_get::<f64, _>("price")
        .map(utils::round_price)
        .map_err(|_| "Error getting `price`".to_string())?;
    let stock_quantity: i32 = row
        .try_get("stock_quantity")
        .map_err(|_| "Error getting `stock_quantity`".to_string())?;

    Ok(DetailsProduct {
        category,
        description,
        id,
        image_url,
        name,
        price,
        stock_quantity,
    })
}

pub async fn handler(
    pool: web::Data<Pool<Postgres>>,
    tmpl: web::Data<Tera>,
    path: web::Path<(i32,)>,
) -> impl Responder {
    let id = path.into_inner().0;

    let query = "
        SELECT id, name, description, price, stock_quantity, category, image_url
        FROM products
        WHERE id = $1;
        ";

    let row = match sqlx::query(query).bind(id).fetch_one(pool.get_ref()).await {
        Ok(row) => row,
        Err(err) => {
            eprintln!("Database query error: {:#?}", err);
            return HttpResponse::InternalServerError().body("Internal Server Error");
        }
    };

    let product: DetailsProduct = match map_row_to_product(row).await {
        Ok(product) => product,
        Err(err) => {
            eprintln!("Product mapping failed: {}", err);
            return HttpResponse::InternalServerError().body("Internal Server Error");
        }
    };

    let mut context = Context::new();
    context.insert("title", &product.name);
    context.insert("product", &product);

    utils::render_template(&tmpl, "product_details.html", &context)
}
