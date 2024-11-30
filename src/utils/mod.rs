use actix_web::{web, HttpResponse};
use csv::ReaderBuilder;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;
use std::error::Error;
use std::fs::File;
use tera::{Context, Tera};

pub fn render_template(
    tmpl: &web::Data<Tera>,
    template_name: &str,
    context: &Context,
) -> HttpResponse {
    match tmpl.render(template_name, context) {
        Ok(rendered) => HttpResponse::Ok().body(rendered),
        Err(err) => {
            eprintln!("Error rendering _{}_ template: {:#?}", template_name, err);
            HttpResponse::InternalServerError().body("Internal Server Error")
        }
    }
}

pub fn round_price(price: f64) -> f64 {
    (price * 100.0).round() / 100.0
}

pub async fn create_database_pool() -> Result<Pool<Postgres>, sqlx::Error> {
    let database_url = env::var("DATABASE_URL").expect("Missing `DATABASE_URL` env variable");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    Ok(pool)
}

pub async fn setup_database(pool: Pool<Postgres>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS products (
        category VARCHAR(50),
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        description VARCHAR(255),
        id SERIAL PRIMARY KEY,
        image_url VARCHAR(255),
        is_active BOOLEAN DEFAULT TRUE,
        name VARCHAR(50) NOT NULL,
        price DOUBLE PRECISION NOT NULL,
        stock_quantity INT NOT NULL,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP);",
    )
    .execute(&pool)
    .await?;
    Ok(())
}

pub async fn populate_database_with_mock_products(
    pool: Pool<Postgres>,
) -> Result<(), Box<dyn Error>> {
    let file = File::open("src/mock_data/products.csv")?;
    let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(file);

    for result in rdr.records() {
        let record = result?;

        let id: i32 = record[0].parse()?;
        let name = &record[1];
        let description = &record[2];
        let price: f64 = record[3].trim_start_matches('$').parse()?;
        let stock_quantity: i32 = record[4].parse()?;
        let category = &record[5];
        let image_url = &record[6];
        println!("Inserting product: id={}, name={}, description={}, price={}, stock_quantity={}, category={}, image_url={}", 
            id, name, description, price, stock_quantity, category, image_url);

        sqlx::query(
            "INSERT INTO products (id, name, description, price, stock_quantity, category, image_url) 
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (id) DO NOTHING"
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(price)
        .bind(stock_quantity)
        .bind(category)
        .bind(image_url)
        .execute(&pool)
        .await?;
    }
    Ok(())
}
