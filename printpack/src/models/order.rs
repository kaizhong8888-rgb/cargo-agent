use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Order {
    pub id: i64,
    pub uuid: String,
    pub user_id: i64,
    pub order_number: String,
    pub status: String,
    pub subtotal: f64,
    pub shipping_fee: f64,
    pub discount: f64,
    pub total: f64,
    pub payment_method: Option<String>,
    pub payment_status: String,
    pub shipping_name: String,
    pub shipping_phone: String,
    pub shipping_address: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OrderItem {
    pub id: i64,
    pub order_id: i64,
    pub product_id: i64,
    pub product_name_zh: String,
    pub product_name_en: String,
    pub material: Option<String>,
    pub size_info: Option<String>,
    pub quantity: i32,
    pub unit_price: f64,
    pub total_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrder {
    pub shipping_name: String,
    pub shipping_phone: String,
    pub shipping_address: String,
    pub notes: Option<String>,
    pub payment_method: String,
    pub items: Vec<OrderItemInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItemInput {
    pub product_id: i64,
    pub quantity: i32,
    pub material: Option<String>,
    pub size_info: Option<String>,
    pub unit_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order: Order,
    pub items: Vec<OrderItem>,
}
