use askama::Template;
use serde::{Deserialize, Serialize};

// ==================== Template Structs ====================

#[derive(Template)]
#[template(path = "base.html")]
pub struct BaseTemplate {
    pub lang: String,
    pub title: String,
    pub content: String, // HTML content rendered inline
}

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate {
    pub lang: String,
    pub categories: Vec<Category>,
    pub featured_products: Vec<ProductCard>,
}

#[derive(Template)]
#[template(path = "products.html")]
pub struct ProductsTemplate {
    pub lang: String,
    pub categories: Vec<Category>,
    pub products: Vec<ProductCard>,
    pub current_category: Option<String>,
    pub current_page: i64,
}

#[derive(Template)]
#[template(path = "product_detail.html")]
pub struct ProductDetailTemplate {
    pub lang: String,
    pub product: ProductDetail,
    pub category: Category,
}

#[derive(Template)]
#[template(path = "quote.html")]
pub struct QuoteTemplate {
    pub lang: String,
    pub product: Option<ProductDetail>,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub lang: String,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    pub lang: String,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "cart.html")]
pub struct CartTemplate {
    pub lang: String,
    pub items: Vec<CartItemView>,
    pub subtotal: f64,
}

#[derive(Template)]
#[template(path = "orders.html")]
pub struct OrdersTemplate {
    pub lang: String,
    pub orders: Vec<OrderView>,
}

// ==================== Data Structs ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub name_zh: String,
    pub name_en: String,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductCard {
    pub uuid: String,
    pub category_name_zh: String,
    pub name_zh: String,
    pub description_zh: Option<String>,
    pub image_url: Option<String>,
    pub base_price: f64,
    pub min_quantity: i32,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductDetail {
    pub id: i64,
    pub uuid: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartItemView {
    pub id: i64,
    pub product_name_zh: String,
    pub product_image: Option<String>,
    pub material: String,
    pub quantity: i32,
    pub unit_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderView {
    pub uuid: String,
    pub order_number: String,
    pub status: String,
    pub total: f64,
    pub created_at: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutRequest {
    pub shipping_name: String,
    pub shipping_phone: String,
    pub shipping_address: String,
    pub notes: Option<String>,
    pub payment_method: String,
}
