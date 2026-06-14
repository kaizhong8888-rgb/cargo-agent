use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub image_url: String,
    pub sort_order: i64,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Product {
    pub id: String,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub category_slug: Option<String>,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub price: f64,
    pub sale_price: Option<f64>,
    pub images: String,
    pub stock: i64,
    pub is_active: bool,
    pub is_featured: bool,
    pub sort_order: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProductRequest {
    pub name: String,
    pub category_id: Option<i64>,
    pub description: String,
    pub price: f64,
    pub sale_price: Option<f64>,
    pub stock: i64,
    pub is_featured: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProductRequest {
    pub name: Option<String>,
    pub category_id: Option<i64>,
    pub description: Option<String>,
    pub price: Option<f64>,
    pub sale_price: Option<f64>,
    pub stock: Option<i64>,
    pub is_featured: Option<bool>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProductListItem {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub price: f64,
    pub sale_price: Option<f64>,
    pub images: String,
    pub is_active: bool,
    pub is_featured: bool,
    pub category_name: Option<String>,
    pub category_slug: Option<String>,
}

impl Product {
    pub fn display_price(&self) -> f64 {
        self.sale_price.unwrap_or(self.price)
    }

    pub fn is_on_sale(&self) -> bool {
        self.sale_price.is_some()
    }

    pub fn first_image(&self) -> Option<String> {
        let images: Vec<String> = serde_json::from_str(&self.images).unwrap_or_default();
        images.into_iter().next()
    }
}

impl ProductListItem {
    pub fn display_price(&self) -> f64 {
        self.sale_price.unwrap_or(self.price)
    }

    pub fn is_on_sale(&self) -> bool {
        self.sale_price.is_some()
    }

    pub fn first_image(&self) -> Option<String> {
        let images: Vec<String> = serde_json::from_str(&self.images).unwrap_or_default();
        images.into_iter().next()
    }
}
