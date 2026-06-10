-- 003_create_quotes.sql
CREATE TABLE IF NOT EXISTS quotes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT NOT NULL UNIQUE,
    user_id INTEGER REFERENCES users(id),
    product_id INTEGER NOT NULL REFERENCES products(id),
    material TEXT NOT NULL,
    size_width REAL NOT NULL,
    size_height REAL NOT NULL,
    quantity INTEGER NOT NULL,
    finishing TEXT,
    unit_price REAL NOT NULL,
    total_price REAL NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',  -- pending, confirmed, rejected
    notes TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_quotes_user ON quotes(user_id);
CREATE INDEX IF NOT EXISTS idx_quotes_product ON quotes(product_id);
CREATE INDEX IF NOT EXISTS idx_quotes_status ON quotes(status);
