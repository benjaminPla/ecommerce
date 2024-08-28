use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::env;

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
        "CREATE TABLE products (
        category VARCHAR(50),
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        description VARCHAR(500),
        id SERIAL PRIMARY KEY,
        image_url VARCHAR(255),
        is_active BOOLEAN DEFAULT TRUE,
        name VARCHAR(50) NOT NULL,
        price NUMERIC(10, 2) NOT NULL,
        stock_quantity INT NOT NULL,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP);",
    )
    .execute(&pool)
    .await?;
    Ok(())
}
