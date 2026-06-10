-- 002_create_products.sql
CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name_zh TEXT NOT NULL,
    name_en TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    description_zh TEXT,
    description_en TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT NOT NULL UNIQUE,
    category_id INTEGER NOT NULL REFERENCES categories(id),
    name_zh TEXT NOT NULL,
    name_en TEXT NOT NULL,
    description_zh TEXT,
    description_en TEXT,
    image_url TEXT,
    base_price REAL NOT NULL DEFAULT 0.0,
    min_quantity INTEGER NOT NULL DEFAULT 100,
    unit TEXT NOT NULL DEFAULT '个',
    materials TEXT,  -- JSON array of available materials
    specs TEXT,      -- JSON array of specifications
    is_active BOOLEAN NOT NULL DEFAULT 1,
    seo_title_zh TEXT,
    seo_title_en TEXT,
    seo_description_zh TEXT,
    seo_description_en TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_products_category ON products(category_id);
CREATE INDEX IF NOT EXISTS idx_products_uuid ON products(uuid);
CREATE INDEX IF NOT EXISTS idx_products_active ON products(is_active);
