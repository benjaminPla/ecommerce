use crate::utils;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Serialize;
use sqlx::{Pool, Postgres, Row};
use std::collections::HashMap;
use tera::{Context, Tera};

#[derive(Serialize)]
struct CartProduct {
    id: i32,
    name: String,
    price: f64,
    quantity: i32,
    total_price_item: f64,
}

fn map_row_to_product(
    row: &sqlx::postgres::PgRow,
    cart_items: &HashMap<i32, i32>,
) -> Result<CartProduct, String> {
    let id: i32 = row.try_get("id").map_err(|_| "Error getting `id`")?;
    let name: String = row.try_get("name").map_err(|_| "Error getting `name`")?;
    let price: f64 = row
        .try_get("price")
        .map(utils::round_price)
        .map_err(|_| "Error getting `price`")?;
    let quantity: i32 = match cart_items.get(&id) {
        Some(quantity) => *quantity,
        None => return Err("Error getting `quantity`".to_string()),
    };
    let total_price_item: f64 = utils::round_price(price * (quantity as f64));

    Ok(CartProduct {
        id,
        name,
        price,
        quantity,
        total_price_item,
    })
}

fn parse_cart_cookie(cart_cookie_value: &str) -> Result<HashMap<i32, i32>, String> {
    let mut cart_items: HashMap<i32, i32> = HashMap::new();
    for product in cart_cookie_value.split(',') {
        match product.split_once(':') {
            Some((id_str, quantity_str)) => {
                let id = id_str.parse::<i32>().map_err(|_| "Error parsing `id`")?;
                let quantity = quantity_str
                    .parse::<i32>()
                    .map_err(|_| "Error parsing `quantity`")?;
                cart_items.insert(id, quantity);
            }
            None => return Err("Error splitting product".to_string()),
        }
    }
    Ok(cart_items)
}

pub async fn handler(
    pool: web::Data<Pool<Postgres>>,
    req: HttpRequest,
    tmpl: web::Data<Tera>,
) -> impl Responder {
    let cart_cookie = match req.cookie("cart") {
        Some(cookie) if !cookie.value().is_empty() => cookie.value().to_string(),
        Some(_) | None => {
            let mut context = Context::new();
            context.insert("title", "Cart");
            return utils::render_template(&tmpl, "empty_cart.html", &context);
        }
    };

    let cart_items = match parse_cart_cookie(&cart_cookie) {
        Ok(items) => items,
        Err(err) => {
            eprintln!("{}", err);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let product_ids: Vec<i32> = cart_items.keys().cloned().collect();
    let query = "SELECT id, name, price FROM products WHERE id = ANY($1)";
    let rows = match sqlx::query(query)
        .bind(&product_ids)
        .fetch_all(pool.get_ref())
        .await
    {
        Ok(rows) => rows,
        Err(err) => {
            eprintln!("Database query error: {:#?}", err);
            return HttpResponse::InternalServerError().body("Internal Server Error");
        }
    };

    let products: Vec<CartProduct> = rows
        .into_iter()
        .filter_map(|row| map_row_to_product(&row, &cart_items).ok())
        .collect();

    let total_price: f64 = utils::round_price(
        products
            .iter()
            .map(|product| product.total_price_item)
            .sum(),
    );

    let mut context = Context::new();
    context.insert("title", "Cart");
    context.insert("products", &products);
    context.insert("total_price", &total_price);

    utils::render_template(&tmpl, "cart.html", &context)
}
