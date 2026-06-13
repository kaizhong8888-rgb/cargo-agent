use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ==================== Database Models ====================

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub name_zh: String,
    pub name_en: String,
    pub slug: String,
    pub description_zh: Option<String>,
    pub description_en: Option<String>,
    pub sort_order: i32,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
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
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProductWithCategory {
    pub id: i64,
    pub uuid: String,
    pub category_id: i64,
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

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub uuid: String,
    pub email: String,
    pub password_hash: String,
    pub name: String,
    pub role: String,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
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
    pub payment_method: String,
    pub payment_status: String,
    pub shipping_name: String,
    pub shipping_phone: String,
    pub shipping_address: String,
    pub notes: Option<String>,
    pub tracking_number: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct OrderItem {
    pub id: i64,
    pub order_id: i64,
    pub product_id: i64,
    pub product_name_zh: String,
    pub product_name_en: String,
    pub material: String,
    pub size_info: String,
    pub quantity: i64,
    pub unit_price: f64,
    pub total_price: f64,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CartItem {
    pub id: i64,
    pub user_id: i64,
    pub product_id: i64,
    pub material: String,
    pub size_width: f64,
    pub size_height: f64,
    pub quantity: i32,
    pub unit_price: f64,
    pub created_at: chrono::NaiveDateTime,
}

impl CartItem {
    pub fn total(&self) -> f64 {
        self.unit_price * self.quantity as f64
    }
}

// ==================== Request/Response Types ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteRequest {
    pub product_id: i64,
    pub material: String,
    pub width: f64,
    pub height: f64,
    pub depth: Option<f64>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: String,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: i64,
    pub uuid: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub phone: Option<String>,
    pub address: Option<String>,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        UserResponse {
            id: u.id,
            uuid: u.uuid,
            email: u.email,
            name: u.name,
            role: u.role,
            phone: u.phone,
            address: u.address,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrder {
    pub items: Vec<OrderItemInput>,
    pub payment_method: String,
    pub shipping_name: String,
    pub shipping_phone: String,
    pub shipping_address: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItemInput {
    pub product_id: i64,
    pub material: String,
    pub size_info: String,
    pub quantity: i64,
    pub unit_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order: Order,
    pub items: Vec<OrderItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartResponse {
    pub items: Vec<CartItem>,
    pub subtotal: f64,
    pub item_count: i32,
}
