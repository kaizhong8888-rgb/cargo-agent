use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use anyhow::Result;
use tracing::info;

pub async fn init(database_url: &str) -> Result<SqlitePool> {
    info!("Connecting to database: {}", database_url);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;
    info!("Database migrations completed");

    Ok(pool)
}

pub async fn seed(pool: &SqlitePool) -> Result<()> {
    use sqlx::QueryBuilder;

    // Seed categories
    let categories = [
        ("包装盒", "Packaging Box", "packaging-box", "各类精美包装盒", "Beautiful packaging boxes"),
        ("标签贴纸", "Labels & Stickers", "labels-stickers", "定制标签贴纸", "Custom labels and stickers"),
        ("画册宣传册", "Brochures", "brochures", "企业画册宣传册", "Corporate brochures"),
        ("手提袋", "Paper Bags", "paper-bags", "精美手提纸袋", "Exquisite paper bags"),
        ("贺卡/请柬", "Cards & Invitations", "cards-invitations", "贺卡请柬定制", "Custom greeting cards"),
    ];

    for (name_zh, name_en, slug, desc_zh, desc_en) in categories {
        sqlx::query!(
            "INSERT OR IGNORE INTO categories (name_zh, name_en, slug, description_zh, description_en) VALUES (?, ?, ?, ?, ?)",
            name_zh, name_en, slug, desc_zh, desc_en
        )
        .execute(pool)
        .await?;
    }

    // Seed products
    let products = [
        (1, "天地盖礼盒", "Lid and Base Gift Box", "高档天地盖礼盒，适用于礼品、茶叶、化妆品等", "Premium lid and base gift box for gifts, tea, cosmetics", 2.5, 100, "个", r#"["铜版纸","灰板纸","特种纸"]"#, r#"["20x15x5cm","25x20x8cm","30x25x10cm"]"#),
        (1, "抽屉式包装盒", "Drawer Box", "创意抽屉式包装盒，独特开启方式", "Creative drawer-style box with unique opening", 3.0, 100, "个", r#"["铜版纸","瓦楞纸","白卡纸"]"#, r#"["18x12x4cm","22x16x6cm","28x20x8cm"]"#),
        (2, "透明PVC贴纸", "Clear PVC Sticker", "防水透明PVC贴纸，适合产品标签", "Waterproof clear PVC sticker for product labels", 0.15, 500, "张", r#"["PVC透明","PET透明"]"#, r#"["5x5cm","8x5cm","10x7cm"]"#),
        (2, "铜版纸标签", "Coated Paper Label", "高品质铜版纸标签，色彩鲜艳", "High-quality coated paper label with vivid colors", 0.08, 1000, "张", r#"["157g铜版纸","200g铜版纸"]"#, r#"["3x2cm","5x3cm","8x5cm"]"#),
        (3, "企业画册", "Corporate Brochure", "专业企业画册设计与印刷", "Professional corporate brochure design and printing", 5.0, 50, "本", r#"["157g铜版纸","200g铜版纸"]"#, r#"["21x28.5cm/12P","21x28.5cm/24P","21x28.5cm/36P"]"#),
        (4, "白卡纸手提袋", "White Cardboard Bag", "高品质白卡纸手提袋，可定制LOGO", "Premium white cardboard bag with custom logo", 1.5, 200, "个", r#"["200g白卡","250g白卡","300g白卡"]"#, r#"["25x32x10cm","30x40x12cm","35x45x15cm"]"#),
        (4, "牛皮纸手提袋", "Kraft Paper Bag", "环保牛皮纸手提袋，简约风格", "Eco-friendly kraft paper bag with minimalist style", 1.2, 200, "个", r#"["120g牛皮纸","150g牛皮纸","180g牛皮纸"]"#, r#"["25x32x10cm","30x40x12cm","35x45x15cm"]"#),
        (5, "新年贺卡", "New Year Greeting Card", "精美新年贺卡，可烫金/UV工艺", "Exquisite New Year greeting card with hot stamping/UV", 0.8, 100, "张", r#"["200g特种纸","250g铜版纸"]"#, r#"["15x10cm","21x15cm","30x21cm"]"#),
    ];

    for (cat_id, name_zh, name_en, desc_zh, desc_en, price, min_qty, unit, materials, specs) in products {
        let uuid = uuid::Uuid::new_v4().to_string();
        sqlx::query!(
            "INSERT INTO products (uuid, category_id, name_zh, name_en, description_zh, description_en, base_price, min_quantity, unit, materials, specs, is_active)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
            uuid, cat_id, name_zh, name_en, desc_zh, desc_en, price, min_qty, unit, materials, specs
        )
        .execute(pool)
        .await?;
    }

    info!("Database seeded successfully");
    Ok(())
}
