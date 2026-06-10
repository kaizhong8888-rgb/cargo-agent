use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Category {
    pub id: i64,
    pub name_zh: String,
    pub name_en: String,
    pub slug: String,
    pub description_zh: Option<String>,
    pub description_en: Option<String>,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Product {
    pub id: i64,
    pub uuid: String,
    pub category_id: i64,
    pub name_zh: String,
    pub name_en: String,
    pub description_zh: Option<String>,
    pub description_en: Option<String>,
    pub image_url: Option<String>,
    pub base_price: f64,
    pub min_quantity: i32,
    pub unit: String,
    pub materials: Option<String>,
    pub specs: Option<String>,
    pub is_active: bool,
    pub seo_title_zh: Option<String>,
    pub seo_title_en: Option<String>,
    pub seo_description_zh: Option<String>,
    pub seo_description_en: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProductWithCategory {
    pub id: i64,
    pub uuid: String,
    pub category_id: i64,
    pub category_slug: String,
    pub category_name_zh: String,
    pub category_name_en: String,
    pub name_zh: String,
    pub name_en: String,
    pub description_zh: Option<String>,
    pub description_en: Option<String>,
    pub image_url: Option<String>,
    pub base_price: f64,
    pub min_quantity: i32,
    pub unit: String,
    pub materials: Option<String>,
    pub specs: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteRequest {
    pub product_id: i64,
    pub material: String,
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub quantity: i32,
    pub finishing: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteResponse {
    pub product_name: String,
    pub material: String,
    pub size: String,
    pub quantity: i32,
    pub unit_price: f64,
    pub total_price: f64,
    pub finishing: Option<String>,
}
