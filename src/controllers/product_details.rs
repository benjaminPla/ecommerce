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

pub async fn handler(
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
