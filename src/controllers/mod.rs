use actix_web::{web, HttpResponse, Responder};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, Row};
use tera::{Context, Tera};

#[derive(Debug, Deserialize, Serialize)]
pub struct Product {
    id: i32,
    name: String,
    price: f64,

}

pub async fn home(pool: web::Data<Pool<Postgres>>, tmpl: web::Data<Tera>) -> impl Responder {
    let mut rows = sqlx::query("SELECT id, name, price FROM products;").fetch(pool.get_ref());
    let mut products:Vec<Product> = Vec::new();
    while let Some(row) = rows
        .try_next()
        .await
        .unwrap_or_else(|error| {
            eprint!("{:#?}", error);
            None
        })
    {
        let id: i32 = row
            .try_get("id")
            .unwrap_or_default();
        let name: String = row
            .try_get("name")
            .unwrap_or_default();
        let price: f64 = row
            .try_get("price")
            .unwrap_or_default();
        let product = Product{id, name,price};
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
