use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CartItem {
    pub id: i64,
    pub user_id: String,
    pub product_id: String,
    pub quantity: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    // Joined fields
    pub product_name: String,
    pub product_slug: String,
    pub product_price: f64,
    pub product_sale_price: Option<f64>,
    pub product_images: String,
}

impl CartItem {
    pub fn display_price(&self) -> f64 {
        self.product_sale_price.unwrap_or(self.product_price)
    }

    pub fn subtotal(&self) -> f64 {
        self.display_price() * self.quantity as f64
    }

    pub fn first_image(&self) -> Option<String> {
        let images: Vec<String> = serde_json::from_str(&self.product_images).unwrap_or_default();
        images.into_iter().next()
    }
}

#[derive(Debug, Deserialize)]
pub struct AddToCartRequest {
    pub product_id: String,
    pub quantity: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCartRequest {
    pub quantity: i64,
}
