use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CartItem {
    pub id: i64,
    pub user_id: i64,
    pub product_id: i64,
    pub product_name_zh: String,
    pub product_name_en: String,
    pub product_image: Option<String>,
    pub material: String,
    pub size_width: Option<f64>,
    pub size_height: Option<f64>,
    pub quantity: i32,
    pub unit_price: f64,
    pub created_at: DateTime<Utc>,
}

impl CartItem {
    pub fn total(&self) -> f64 {
        self.unit_price * self.quantity as f64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddToCartRequest {
    pub product_id: i64,
    pub material: String,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCartRequest {
    pub quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartResponse {
    pub items: Vec<CartItem>,
    pub subtotal: f64,
    pub item_count: i64,
}
