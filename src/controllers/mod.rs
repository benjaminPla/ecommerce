use actix_web::cookie::{time::Duration, CookieBuilder, SameSite};
use actix_web::http::header::HeaderValue;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use futures::TryStreamExt;
use serde::Serialize;
use sqlx::{Pool, Postgres, Row};
use std::collections::HashMap;
use std::env;
use stripe::{CreatePaymentIntent, Currency, PaymentIntent};
use tera::{Context, Tera};

#[derive(Serialize)]
struct HomeProduct {
    description: String,
    id: i32,
    image_url: String,
    name: String,
    price: f64,
}

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

#[derive(Serialize)]
struct CartProduct {
    id: i32,
    name: String,
    price: f64,
    quantity: i32,
    total_price_item: f64,
}

pub async fn home(pool: web::Data<Pool<Postgres>>, tmpl: web::Data<Tera>) -> impl Responder {
    let mut rows = sqlx::query("SELECT id, name, description, price, image_url FROM products;")
        .fetch(pool.get_ref());
    let mut products: Vec<HomeProduct> = Vec::new();
    while let Some(row) = rows.try_next().await.unwrap_or_else(|error| {
        eprintln!("{:#?}", error);
        None
    }) {
        let id: i32 = row.try_get("id").unwrap_or_default();
        let name: String = row.try_get("name").unwrap_or_default();
        let description: String = row.try_get("description").unwrap_or_default();
        let price: f64 = match row.try_get::<f64, _>("price") {
            Ok(value) => (value * 100.0).round() / 100.0,
            Err(error) => {
                eprintln!("Error retrieving price for product {}: {:#?}", id, error);
                continue;
            }
        };
        let image_url: String = row.try_get("image_url").unwrap_or_default();
        let product = HomeProduct {
            description,
            id,
            image_url,
            name,
            price,
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
            let price: f64 = match row.try_get::<f64, _>("price") {
                Ok(value) => (value * 100.0).round() / 100.0,
                Err(error) => {
                    eprintln!("Error retrieving price for product {}: {:#?}", id, error);
                    return HttpResponse::InternalServerError()
                        .body("Error retrieving product details");
                }
            };
            let stock_quantity: i32 = row.try_get("stock_quantity").unwrap_or_default();
            let category: String = row.try_get("category").unwrap_or_default();
            let image_url: String = row.try_get("image_url").unwrap_or_default();

            DetailsProduct {
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
    match req.cookie("cart") {
        Some(cookie) => {
            if cookie.value().is_empty() {
                let mut context = Context::new();
                context.insert("title", "Cart");

                return match tmpl.render("empty_cart.html", &context) {
                    Ok(rendered) => HttpResponse::Ok().body(rendered),
                    Err(err) => {
                        eprintln!("Error rendering empty cart template: {:#?}", err);
                        HttpResponse::InternalServerError()
                            .body("Error rendering template")
                    }
                };
            }
            let mut cart_items: HashMap<i32, i32> = HashMap::new();
            for product in cookie.value().split(',') {
                match product.split_once(':') {
                    Some((id_str, quantity_str)) => {
                        let id = match id_str.parse::<i32>() {
                            Ok(id) => id,
                            Err(err) => {
                                eprintln!("Error parsing product ID: {:#?}", err);
                                return HttpResponse::InternalServerError().finish();
                            }
                        };
                        let quantity = match quantity_str.parse::<i32>() {
                            Ok(quantity) => quantity,
                            Err(err) => {
                                eprintln!("Error parsing product quantity: {:#?}", err);
                                return HttpResponse::InternalServerError().finish();
                            }
                        };
                        cart_items.insert(id, quantity);
                    }
                    None => {
                        eprintln!("Error splitting product entry: {}", product);
                        return HttpResponse::InternalServerError().finish();
                    }
                }
            }

            let product_ids: Vec<i32> = cart_items.keys().cloned().collect();
            let mut rows = sqlx::query("SELECT id, name, price FROM products WHERE id = ANY($1)")
                .bind(&product_ids)
                .fetch(pool.get_ref());

            let mut products: Vec<CartProduct> = Vec::new();
            let mut total_price: f64 = 0.0;

            while let Some(row) = rows.try_next().await.unwrap_or_else(|error| {
                eprintln!("Database query error: {:#?}", error);
                None
            }) {
                let id: i32 = row.try_get("id").unwrap_or_default();
                let name: String = row.try_get("name").unwrap_or_default();
                let price: f64 = match row.try_get::<f64, _>("price") {
                    Ok(value) => (value * 100.0).round() / 100.0,
                    Err(error) => {
                        eprintln!("Error retrieving price for product {}: {:#?}", id, error);
                        continue;
                    }
                };
                if let Some(&quantity) = cart_items.get(&id) {
                    let total_price_item = price * quantity as f64;
                    total_price += total_price_item;
                    products.push(CartProduct {
                        id,
                        name,
                        price,
                        quantity,
                        total_price_item,
                    });
                }
            }

            total_price = (total_price * 100.0).round() / 100.0;

            let mut context = Context::new();
            context.insert("title", "Cart");
            context.insert("products", &products);
            context.insert("total_price", &total_price);

            match tmpl.render("cart.html", &context) {
                Ok(rendered) => HttpResponse::Ok().body(rendered),
                Err(err) => {
                    eprintln!("Error rendering cart template: {:#?}", err);
                    HttpResponse::InternalServerError().body("Error rendering template")
                }
            }
        }
        None => {
            let mut context = Context::new();
            context.insert("title", "Cart");

            match tmpl.render("empty_cart.html", &context) {
                Ok(rendered) => HttpResponse::Ok().body(rendered),
                Err(err) => {
                    eprintln!("Error rendering empty cart template: {:#?}", err);
                    HttpResponse::InternalServerError().body("Error rendering template")
                }
            }
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
        .max_age(Duration::weeks(1))
        .finish();

    HttpResponse::SeeOther()
        .insert_header(("HX-Redirect", HeaderValue::from_static("/cart")))
        .cookie(cookie)
        .finish()
}

pub async fn remove_from_cart(path: web::Path<(i32,)>, req: HttpRequest) -> impl Responder {
    let id_to_remove = path.into_inner().0;
    let mut cart: HashMap<i32, i32> = HashMap::new();
    if let Some(cookie) = req.cookie("cart") {
        for item in cookie.value().split(',') {
            if let Some((id_str, qty_str)) = item.split_once(':') {
                if let (Ok(id), Ok(qty)) = (id_str.parse::<i32>(), qty_str.parse::<i32>()) {
                    if id != id_to_remove {
                        cart.insert(id, qty);
                    }
                }
            }
        }
    }
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
        .max_age(Duration::weeks(1))
        .finish();
    HttpResponse::Gone()
        .insert_header(("Location", "/cart"))
        .insert_header(("HX-Refresh", "true"))
        .cookie(cookie)
        .finish()
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

pub async fn payment(
    pool: web::Data<Pool<Postgres>>,
    tmpl: web::Data<Tera>,
    req: HttpRequest,
) -> impl Responder {
    let cart_cookie = match req.cookie("cart") {
        Some(cookie) if !cookie.value().is_empty() => cookie,
        _ => {
            let mut context = Context::new();
            context.insert("title", "Payment");
            return match tmpl.render("empty_cart.html", &context) {
                Ok(rendered) => HttpResponse::Ok().body(rendered),
                Err(err) => {
                    eprintln!("Error rendering empty cart template: {:?}", err);
                    HttpResponse::InternalServerError().body("Error rendering template")
                }
            };
        }
    };

    let mut cart_items: HashMap<i32, i32> = HashMap::new();
    for product in cart_cookie.value().split(',') {
        if let Some((id_str, quantity_str)) = product.split_once(':') {
            let id = match id_str.parse::<i32>() {
                Ok(id) => id,
                Err(err) => {
                    eprintln!("Error parsing product ID: {:?}", err);
                    return HttpResponse::InternalServerError().finish();
                }
            };
            let quantity = match quantity_str.parse::<i32>() {
                Ok(quantity) => quantity,
                Err(err) => {
                    eprintln!("Error parsing product quantity: {:?}", err);
                    return HttpResponse::InternalServerError().finish();
                }
            };
            cart_items.insert(id, quantity);
        } else {
            eprintln!("Error splitting product entry: {}", product);
            return HttpResponse::InternalServerError().finish();
        }
    }

    let product_ids: Vec<i32> = cart_items.keys().cloned().collect();
    let mut description: String = String::new();
    let mut products = Vec::new();
    let mut total_price: f64 = 0.0;

    let mut rows = sqlx::query("SELECT id, name, price FROM products WHERE id = ANY($1)")
        .bind(&product_ids)
        .fetch(pool.get_ref());

    while let Some(row) = rows.try_next().await.unwrap_or_else(|error| {
        eprintln!("Database query error: {:?}", error);
        None
    }) {
        let id: i32 = row.try_get("id").unwrap_or_default();
        let name: String = row.try_get("name").unwrap_or_default();
        let price: f64 = match row.try_get::<f64, _>("price") {
            Ok(value) => (value * 100.0).round() / 100.0,
            Err(error) => {
                eprintln!("Error retrieving price for product {}: {:?}", id, error);
                continue;
            }
        };
        if let Some(&quantity) = cart_items.get(&id) {
            let total_price_item = price * quantity as f64;
            total_price += total_price_item;
            products.push(CartProduct {
                id,
                name,
                price,
                quantity,
                total_price_item,
            });
        }
    }

    total_price = (total_price * 100.0).round() / 100.0;

    let stripe_private_key = match env::var("STRIPE_PRIVATE_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("Error: Missing `STRIPE_PRIVATE_KEY` environment variable");
            return HttpResponse::InternalServerError().body("Missing Stripe private key");
        }
    };
    let stripe_public_key = match env::var("STRIPE_PUBLIC_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("Error: Missing `STRIPE_PUBLIC_KEY` environment variable");
            return HttpResponse::InternalServerError().body("Missing Stripe public key");
        }
    };

    let client = stripe::Client::new(stripe_private_key);

    let client_secret = {
        let mut create_intent =
            CreatePaymentIntent::new((total_price * 100.0) as i64, Currency::EUR);
        create_intent.confirm = Some(false);
        // create_intent.return_url = Some("http://localhost:8080/stripe-webhook");
        // create_intent.shipping = Some(&shipping);

        let mut description_vec: Vec<String> = Vec::new();
        for product in &products {
            let product_info = format!("{} (x{})", product.name, product.quantity);
            description_vec.push(product_info);
        }
        description = description_vec.join(" + ");
        create_intent.description = Some(&description);

        match PaymentIntent::create(&client, create_intent).await {
            Ok(payment_intent) => match payment_intent.client_secret {
                Some(secret) => secret,
                None => {
                    eprintln!("No client secret found in payment intent");
                    return HttpResponse::InternalServerError().body("Error rendering template");
                }
            },
            Err(error) => {
                eprintln!("Failed to create payment intent: {:?}", error);
                return HttpResponse::InternalServerError().body("Error rendering template");
            }
        }
    };

    let mut context = Context::new();
    context.insert("CLIENT_SECRET", &client_secret);
    context.insert("description", &description);
    context.insert("STRIPE_PUBLIC_KEY", &stripe_public_key);
    context.insert("title", "Ecommerce - Payment");
    context.insert("total_price", &total_price);

    match tmpl.render("payment.html", &context) {
        Ok(rendered) => HttpResponse::Ok().body(rendered),
        Err(err) => {
            eprintln!("Template rendering error: {:?}", err);
            HttpResponse::InternalServerError().body("Error rendering template")
        }
    }
}

pub async fn stripe_webhook(tmpl: web::Data<Tera>) -> impl Responder {
    let cookie = CookieBuilder::new("cart", "")
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Strict)
        .max_age(Duration::weeks(1))
        .finish();

    let mut context = Context::new();
    context.insert("title", "Thank You!");

    match tmpl.render("stripe-webhook.html", &context) {
        Ok(rendered) => HttpResponse::Ok().cookie(cookie).body(rendered),
        Err(err) => {
            eprintln!("Template rendering error: {:?}", err);
            HttpResponse::InternalServerError().body("Error rendering template")
        }
    }
}
