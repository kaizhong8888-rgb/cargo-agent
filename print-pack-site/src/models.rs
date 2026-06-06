use serde::{Deserialize, Serialize};

/// 产品类别
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductCategory {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub icon: String,
}

/// 产品
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: u32,
    pub name: String,
    pub category_id: u32,
    pub description: String,
    pub features: Vec<String>,
    pub images: Vec<String>,
    pub min_order: u32,
    pub price_range: String,
}

/// 询价表单
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InquiryForm {
    pub name: String,
    pub email: String,
    pub company: String,
    pub phone: String,
    pub product_type: String,
    pub quantity: u32,
    pub specifications: String,
    pub message: String,
}

/// 报价请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteRequest {
    pub product_id: u32,
    pub quantity: u32,
    pub size: String,
    pub material: String,
    pub printing_colors: u8,
    pub finishing: String,
    pub delivery_date: String,
    pub contact: ContactInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactInfo {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub company: String,
    pub address: String,
}

/// 初始化示例数据
pub fn get_categories() -> Vec<ProductCategory> {
    vec![
        ProductCategory {
            id: 1,
            name: "纸盒包装".into(),
            description: "高品质折叠纸盒、礼品盒、展示盒等各类纸盒包装解决方案".into(),
            icon: "📦".into(),
        },
        ProductCategory {
            id: 2,
            name: "标签贴纸".into(),
            description: "不干胶标签、防伪标签、特种标签等专业印刷服务".into(),
            icon: "🏷️".into(),
        },
        ProductCategory {
            id: 3,
            name: "手提袋".into(),
            description: "纸制手提袋、环保购物袋、品牌宣传袋等".into(),
            icon: "🛍️".into(),
        },
        ProductCategory {
            id: 4,
            name: "说明书/画册".into(),
            description: "产品说明书、企业画册、宣传册等印刷品".into(),
            icon: "📖".into(),
        },
    ]
}

pub fn get_products() -> Vec<Product> {
    vec![
        Product {
            id: 1,
            name: "折叠纸盒".into(),
            category_id: 1,
            description: "适用于化妆品、食品、电子产品等行业的标准折叠纸盒，支持多种印刷工艺和表面处理。".into(),
            features: vec![
                "支持1-6色印刷".into(),
                "覆膜/烫金/UV可选".into(),
                "起订量500个".into(),
                "7-10天交货".into(),
            ],
            images: vec!["/static/images/folding-box-1.jpg".into(), "/static/images/folding-box-2.jpg".into()],
            min_order: 500,
            price_range: "¥0.5 - ¥5.0".into(),
        },
        Product {
            id: 2,
            name: "精品礼盒".into(),
            category_id: 1,
            description: "高端礼品盒，适用于珠宝、茶叶、酒类等高端产品包装，多种材质和工艺可选。".into(),
            features: vec![
                "硬纸板+灰板结构".into(),
                "烫金/压纹/丝印".into(),
                "定制内衬".into(),
                "起订量300个".into(),
            ],
            images: vec!["/static/images/gift-box-1.jpg".into()],
            min_order: 300,
            price_range: "¥5.0 - ¥30.0".into(),
        },
        Product {
            id: 3,
            name: "不干胶标签".into(),
            category_id: 2,
            description: "高品质不干胶标签，适用于食品、日化、医药等行业，防水防撕。".into(),
            features: vec![
                "铜版纸/合成纸/PP材质".into(),
                "防水防油防撕".into(),
                "可变数据印刷".into(),
                "起订量1000张".into(),
            ],
            images: vec!["/static/images/label-1.jpg".into()],
            min_order: 1000,
            price_range: "¥0.01 - ¥0.5".into(),
        },
        Product {
            id: 4,
            name: "品牌手提袋".into(),
            category_id: 3,
            description: "品牌宣传手提袋，多种材质和尺寸，提升品牌形象。".into(),
            features: vec![
                "157g-250g铜版纸".into(),
                "覆膜+绳把".into(),
                "支持烫金/UV".into(),
                "起订量500个".into(),
            ],
            images: vec!["/static/images/bag-1.jpg".into()],
            min_order: 500,
            price_range: "¥1.0 - ¥8.0".into(),
        },
    ]
}
