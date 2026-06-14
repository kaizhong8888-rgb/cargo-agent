use sqlx::SqlitePool;
use std::path::Path;

pub async fn init_db(database_url: &str) -> anyhow::Result<SqlitePool> {
    // Ensure data directory exists
    if let Some(dir) = Path::new(database_url)
        .strip_prefix("sqlite:")
        .ok()
        .and_then(|p| p.parent())
    {
        tokio::fs::create_dir_all(dir).await?;
    }

    let pool = SqlitePool::connect(database_url).await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

/// Create default admin user if it doesn't exist
pub async fn ensure_admin(pool: &SqlitePool, email: &str, password: &str) -> anyhow::Result<()> {
    use argon2::{
        password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
        Argon2,
    };

    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(pool)
            .await?;

    if existing.is_some() {
        return Ok(());
    }

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())?;

    sqlx::query(
        "INSERT INTO users (email, password_hash, name, role) VALUES (?, ?, 'Admin', 'admin')",
    )
    .bind(email)
    .bind(&password_hash)
    .execute(pool)
    .await?;

    tracing::info!("Default admin user created: {}", email);
    Ok(())
}

/// Seed sample data for development
pub async fn seed_data(pool: &SqlitePool) -> anyhow::Result<()> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM categories")
        .fetch_one(pool)
        .await?;

    if count.0 > 0 {
        return Ok(()); // Already seeded
    }

    // Insert categories
    sqlx::query("INSERT INTO categories (name, slug, description, sort_order) VALUES (?, ?, ?, ?)")
        .bind("文胸")
        .bind("bras")
        .bind("精致文胸系列，从日常舒适到特殊场合")
        .bind(1)
        .execute(pool)
        .await?;

    sqlx::query("INSERT INTO categories (name, slug, description, sort_order) VALUES (?, ?, ?, ?)")
        .bind("内裤")
        .bind("panties")
        .bind("舒适透气内裤，多种风格可选")
        .bind(2)
        .execute(pool)
        .await?;

    sqlx::query("INSERT INTO categories (name, slug, description, sort_order) VALUES (?, ?, ?, ?)")
        .bind("睡衣")
        .bind("sleepwear")
        .bind("丝滑睡衣，优雅入眠")
        .bind(3)
        .execute(pool)
        .await?;

    sqlx::query("INSERT INTO categories (name, slug, description, sort_order) VALUES (?, ?, ?, ?)")
        .bind("套装")
        .bind("sets")
        .bind("精心搭配的完整套装")
        .bind(4)
        .execute(pool)
        .await?;

    sqlx::query("INSERT INTO categories (name, slug, description, sort_order) VALUES (?, ?, ?, ?)")
        .bind("家居服")
        .bind("loungewear")
        .bind("居家休闲，舒适优雅")
        .bind(5)
        .execute(pool)
        .await?;

    sqlx::query("INSERT INTO categories (name, slug, description, sort_order) VALUES (?, ?, ?, ?)")
        .bind("配饰")
        .bind("accessories")
        .bind("搭配点缀，尽显细节之美")
        .bind(6)
        .execute(pool)
        .await?;

    // Insert sample products
    let products = vec![
        ("法式蕾丝文胸", "french-lace-bra", "精致法式蕾丝，无钢圈设计，舒适透气", 1, 299.0, 239.0, 100, 1),
        ("丝绸无痕文胸", "silk-seamless-bra", "100%桑蚕丝面料，无感穿着体验", 1, 399.0, None, 80, 1),
        ("运动防震文胸", "sports-support-bra", "高强度支撑，透气速干面料", 1, 199.0, 159.0, 150, 0),
        ("真丝蕾丝内裤", "silk-lace-panty", "真丝与蕾丝完美结合，极致舒适", 2, 129.0, 99.0, 200, 0),
        ("纯棉基础内裤套装", "cotton-basics-set", "5条装纯棉内裤，透气吸汗", 2, 199.0, None, 300, 0),
        ("冰丝无痕内裤", "ice-silk-seamless-panty", "冰丝面料，无痕剪裁，清凉透气", 2, 89.0, 69.0, 250, 0),
        ("真丝吊带睡裙", "silk-camisole-nightgown", "100%真丝面料，优雅性感", 3, 599.0, 499.0, 50, 1),
        ("蕾丝睡衣套装", "lace-pajama-set", "精致蕾丝点缀，丝滑面料", 3, 459.0, None, 60, 0),
        ("法兰绒家居套装", "flannel-loungewear-set", "柔软法兰绒，秋冬必备", 5, 359.0, 299.0, 80, 0),
        ("情人节限定套装", "valentines-limited-set", "红色蕾丝文胸+内裤套装，节日特别款", 4, 499.0, 399.0, 30, 1),
        ("新娘婚纱内衣套装", "bridal-lingerie-set", "纯白蕾丝，婚礼专属设计", 4, 699.0, None, 20, 1),
        ("丝质眼罩", "silk-eye-mask", "真丝眼罩，帮助深度睡眠", 6, 99.0, 79.0, 200, 0),
    ];

    for (name, slug, desc, cat_id, price, sale_price, stock, featured) in products {
        let slug_lower = slug.to_lowercase();
        sqlx::query(
            "INSERT INTO products (category_id, name, slug, description, price, sale_price, stock, is_featured) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(cat_id)
        .bind(name)
        .bind(&slug_lower)
        .bind(desc)
        .bind(price)
        .bind(sale_price)
        .bind(stock)
        .bind(featured)
        .execute(pool)
        .await?;
    }

    tracing::info!("Sample data seeded successfully");
    Ok(())
}
