use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_web::cookie::{CookieBuilder, SameSite};
use futures::TryStreamExt;
use serde::Serialize;
use sqlx::{Pool, Postgres, Row};
use std::collections::HashMap;
use tera::{Context, Tera};

#[derive(Serialize)]
struct Product {
    category: String,
    description: String,
    id: i32,
    image_url: String,
    name: String,
    price: f64,
    stock_quantity: i32,
}

#[derive(Serialize)]
struct CartProduct {
    id: i32,
    name: String,
    price: f64,
    quantity: i32,
    total_price: f64,
}

pub async fn home(pool: web::Data<Pool<Postgres>>, tmpl: web::Data<Tera>) -> impl Responder {
    let mut rows = sqlx::query(
        "SELECT id, name, description, price, stock_quantity, category, image_url FROM products;",
    )
    .fetch(pool.get_ref());
    let mut products: Vec<Product> = Vec::new();
    while let Some(row) = rows.try_next().await.unwrap_or_else(|error| {
        eprint!("{:#?}", error);
        None
    }) {
        let id: i32 = row.try_get("id").unwrap_or_default();
        let name: String = row.try_get("name").unwrap_or_default();
        let description: String = row.try_get("description").unwrap_or_default();
        let price: f64 = row.try_get::<f64, _>("price").unwrap_or_default();
        let stock_quantity: i32 = row.try_get("stock_quantity").unwrap_or_default();
        let category: String = row.try_get("category").unwrap_or_default();
        let image_url: String = row.try_get("image_url").unwrap_or_default();
        let product = Product {
            category,
            description,
            id,
            image_url,
            name,
            price,
            stock_quantity,
        };
        products.push(product);
    }

    let mut context = Context::new();
    context.insert("title", "Ecommerce");
    context.insert("products", &products);

    match tmpl.render("index.html", &context) {
        Ok(rendered) => HttpResponse::Ok().body(rendered),
        Err(err) => {
            eprintln!("{:#?}", err);
            HttpResponse::InternalServerError().body("Error rendering template")
        }
    }
}

pub async fn product_details(
    pool: web::Data<Pool<Postgres>>,
    tmpl: web::Data<Tera>,
    path: web::Path<(i32,)>,
) -> impl Responder {
    let id = path.into_inner().0;

    let result = sqlx::query(
        "
        SELECT id, name, description, price, stock_quantity, category, image_url
        FROM products
        WHERE id = $1;
        ",
    )
    .bind(id)
    .fetch_one(pool.get_ref())
    .await;

    let product = match result {
        Ok(row) => {
            let id: i32 = row.try_get("id").unwrap_or_default();
            let name: String = row.try_get("name").unwrap_or_default();
            let description: String = row.try_get("description").unwrap_or_default();
            let price: f64 = row.try_get::<f64, _>("price").unwrap_or_default();
            let stock_quantity: i32 = row.try_get("stock_quantity").unwrap_or_default();
            let category: String = row.try_get("category").unwrap_or_default();
            let image_url: String = row.try_get("image_url").unwrap_or_default();

            Product {
                id,
                name,
                description,
                price,
                stock_quantity,
                category,
                image_url,
            }
        }
        Err(error) => {
            eprintln!("{:#?}", error);
            return HttpResponse::InternalServerError().body("Product not found");
        }
    };

    let mut context = Context::new();
    context.insert("title", &format!("{}", &product.name));
    context.insert("product", &product);

    match tmpl.render("product_details.html", &context) {
        Ok(rendered) => HttpResponse::Ok().body(rendered),
        Err(err) => {
            eprintln!("{:#?}", err);
            HttpResponse::InternalServerError().body("Error rendering template")
        }
    }
}

pub async fn cart(
    pool: web::Data<Pool<Postgres>>,
    tmpl: web::Data<Tera>,
    req: HttpRequest,
) -> impl Responder {
    let mut cart_items: HashMap<i32, i32> = HashMap::new();

    if let Some(cookie) = req.cookie("cart") {
        for item in cookie.value().split(',') {
            if let Some((id_str, qty_str)) = item.split_once(':') {
                if let (Ok(id), Ok(qty)) = (id_str.parse::<i32>(), qty_str.parse::<i32>()) {
                    cart_items.insert(id, qty);
                }
            }
        }
    }

    let product_ids: Vec<i32> = cart_items.keys().cloned().collect();

    let rows = if !product_ids.is_empty() {
        sqlx::query!(
            "SELECT id, name, price FROM products WHERE id = ANY($1)",
            &product_ids
        )
        .fetch_all(pool.get_ref())
        .await
        .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut products: Vec<CartProduct> = Vec::new();
    let mut total_amount = 0.0;

    for row in rows {
        if let Some(quantity) = cart_items.get(&row.id) {
            let total_price = row.price * *quantity as f64;
            products.push(CartProduct {
                id: row.id,
                name: row.name,
                price: row.price,
                quantity: *quantity,
                total_price,
            });
            total_amount += total_price;
        }
    }

    let mut context = Context::new();
    context.insert("title", "Cart");
    context.insert("products", &products);
    context.insert("total_amount", &total_amount);

    match tmpl.render("cart.html", &context) {
        Ok(rendered) => HttpResponse::Ok().body(rendered),
        Err(err) => {
            eprintln!("Error rendering template: {:#?}", err);
            HttpResponse::InternalServerError().body("Error rendering template")
        }
    }
}

pub async fn add_to_cart(
    path: web::Path<(i32,)>,
    req: HttpRequest,
    form: web::Form<HashMap<String, String>>,
) -> impl Responder {
    let id = path.into_inner().0;
    let quantity: i32 = form
        .get("quantity")
        .and_then(|q| q.parse::<i32>().ok())
        .map(|q| q.clamp(1, 100))
        .unwrap_or(1);

    let mut cart: HashMap<i32, i32> = HashMap::new();

    if let Some(cookie) = req.cookie("cart") {
        for item in cookie.value().split(',') {
            if let Some((id_str, qty_str)) = item.split_once(':') {
                if let (Ok(id), Ok(qty)) = (id_str.parse::<i32>(), qty_str.parse::<i32>()) {
                    cart.insert(id, qty);
                }
            }
        }
    }

    cart.entry(id)
        .and_modify(|q| *q = quantity)
        .or_insert(quantity);

    let cart_value = cart
        .into_iter()
        .map(|(id, qty)| format!("{}:{}", id, qty))
        .collect::<Vec<String>>()
        .join(",");

    let cookie = CookieBuilder::new("cart", cart_value)
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Strict)
        .finish();

    HttpResponse::Ok().cookie(cookie).finish()
}

pub async fn not_found(tmpl: web::Data<Tera>) -> impl Responder {
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
