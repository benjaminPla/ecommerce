use actix_web::cookie::CookieBuilder;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use futures::TryStreamExt;
use serde::Serialize;
use sqlx::{Pool, Postgres, Row};
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
    tmlp: web::Data<Tera>,
    req: HttpRequest,
) -> impl Responder {
    let mut cart_items = Vec::new();

    if let Ok(cookies) = req.cookies() {
        for cookie in cookies.clone().into_iter() {
            if cookie.name().starts_with("cart_item_") {
                cart_items.push(cookie.value().to_string());
            }
        }
    } else {
        eprintln!("Failed to get cookies from request");
    }

    if cart_items.is_empty() {
        HttpResponse::Ok().body("No items in the cart")
    } else {
        HttpResponse::Ok().json(cart_items)
    }
}

pub async fn add_to_cart(path: web::Path<(i32,)>, req: HttpRequest) -> impl Responder {
    let id = path.into_inner().0;
    
    let mut cart = Vec::new();

    if let Some(cookie) = req.cookie("cart") {
        cart = cookie
            .value()
            .split(',')
            .filter_map(|str_value| str_value.parse::<i32>().ok())
            .collect();
    }

    cart.push(id);

    let cart_value = cart.into_iter().map(|i| i.to_string()).collect::<Vec<String>>().join(",");

    let cookie = CookieBuilder::new("cart", cart_value)
        .path("/")
        .secure(true)
        .http_only(true)
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
