-- 005_create_cart.sql
CREATE TABLE IF NOT EXISTS cart_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    product_id INTEGER NOT NULL REFERENCES products(id),
    material TEXT NOT NULL,
    size_width REAL,
    size_height REAL,
    quantity INTEGER NOT NULL DEFAULT 1,
    unit_price REAL NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(user_id, product_id, material, size_width, size_height)
);

CREATE INDEX IF NOT EXISTS idx_cart_user ON cart_items(user_id);
