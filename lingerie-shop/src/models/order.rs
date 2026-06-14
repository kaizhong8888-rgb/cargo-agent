use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Order {
    pub id: String,
    pub user_id: String,
    pub total: f64,
    pub status: String,
    pub shipping_name: String,
    pub shipping_phone: String,
    pub shipping_address: String,
    pub shipping_city: String,
    pub shipping_state: String,
    pub shipping_zip: String,
    pub shipping_country: String,
    pub payment_method: String,
    pub payment_status: String,
    pub notes: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OrderItem {
    pub id: i64,
    pub order_id: String,
    pub product_id: String,
    pub product_name: String,
    pub product_image: String,
    pub quantity: i64,
    pub price: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutRequest {
    pub shipping_name: String,
    pub shipping_phone: String,
    pub shipping_address: String,
    pub shipping_city: String,
    pub shipping_state: String,
    pub shipping_zip: String,
    pub shipping_country: String,
    pub payment_method: String,
    pub notes: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrderStatusRequest {
    pub status: String,
}

pub struct OrderDetail {
    pub order: Order,
    pub items: Vec<OrderItem>,
}

impl Order {
    pub fn status_label(&self) -> &str {
        match self.status.as_str() {
            "pending" => "待确认",
            "confirmed" => "已确认",
            "shipped" => "已发货",
            "delivered" => "已送达",
            "cancelled" => "已取消",
            "refunded" => "已退款",
            _ => "未知",
        }
    }

    pub fn payment_status_label(&self) -> &str {
        match self.payment_status.as_str() {
            "unpaid" => "未支付",
            "paid" => "已支付",
            "failed" => "支付失败",
            "refunded" => "已退款",
            _ => "未知",
        }
    }
}
