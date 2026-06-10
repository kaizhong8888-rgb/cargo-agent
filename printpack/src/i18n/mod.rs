use std::collections::HashMap;
use std::sync::RwLock;
use once_cell::sync::Lazy;

static TRANSLATIONS: Lazy<RwLock<HashMap<String, HashMap<String, String>>>> = Lazy::new(|| {
    RwLock::new(HashMap::from([
        ("zh".to_string(), load_zh()),
        ("en".to_string(), load_en()),
    ]))
});

fn load_zh() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("nav.home", "首页");
    m.insert("nav.products", "产品中心");
    m.insert("nav.quote", "在线报价");
    m.insert("nav.cart", "购物车");
    m.insert("nav.orders", "我的订单");
    m.insert("nav.login", "登录");
    m.insert("nav.register", "注册");
    m.insert("nav.logout", "退出");
    m.insert("nav.profile", "个人中心");
    m.insert("nav.admin", "后台管理");
    m.insert("home.hero_title", "专业印刷包装定制服务");
    m.insert("home.hero_subtitle", "高品质包装盒、标签、画册一站式定制");
    m.insert("home.hero_cta", "立即获取报价");
    m.insert("home.features_title", "我们的优势");
    m.insert("home.features_quality", "品质保证");
    m.insert("home.features_quality_desc", "进口设备，环保材料，严格质检");
    m.insert("home.features_fast", "快速交付");
    m.insert("home.features_fast_desc", "最快3天出货，急单可加急");
    m.insert("home.features_custom", "量身定制");
    m.insert("home.features_custom_desc", "免费设计打样，满足个性需求");
    m.insert("home.features_price", "价格透明");
    m.insert("home.features_price_desc", "工厂直供，无中间商差价");
    m.insert("products.title", "产品中心");
    m.insert("products.filter_category", "按分类筛选");
    m.insert("products.all_categories", "全部分类");
    m.insert("products.get_quote", "获取报价");
    m.insert("products.detail", "查看详情");
    m.insert("products.description", "产品描述");
    m.insert("products.specifications", "规格参数");
    m.insert("products.material_options", "可选材质");
    m.insert("products.min_qty", "起订量");
    m.insert("products.add_to_cart", "加入购物车");
    m.insert("quote.title", "在线报价");
    m.insert("quote.product", "选择产品");
    m.insert("quote.material", "材质");
    m.insert("quote.width", "宽度(cm)");
    m.insert("quote.height", "高度(cm)");
    m.insert("quote.depth", "深度(cm)");
    m.insert("quote.quantity", "数量");
    m.insert("quote.finishing", "表面工艺");
    m.insert("quote.notes", "备注说明");
    m.insert("quote.submit", "提交报价");
    m.insert("quote.price_estimate", "预估价格");
    m.insert("quote.unit_price", "单价");
    m.insert("quote.total_price", "总价");
    m.insert("cart.title", "购物车");
    m.insert("cart.empty", "购物车是空的");
    m.insert("cart.subtotal", "小计");
    m.insert("cart.checkout", "去结算");
    m.insert("cart.remove", "移除");
    m.insert("cart.quantity", "数量");
    m.insert("auth.login", "登录");
    m.insert("auth.register", "注册");
    m.insert("auth.email", "邮箱");
    m.insert("auth.password", "密码");
    m.insert("auth.name", "姓名");
    m.insert("auth.phone", "手机号");
    m.insert("auth.no_account", "还没有账号？");
    m.insert("auth.has_account", "已有账号？");
    m.insert("order.title", "我的订单");
    m.insert("order.number", "订单号");
    m.insert("order.status", "状态");
    m.insert("order.total", "总金额");
    m.insert("order.date", "下单时间");
    m.insert("order.detail", "订单详情");
    m.insert("order.pending", "待确认");
    m.insert("order.confirmed", "已确认");
    m.insert("order.production", "生产中");
    m.insert("order.shipped", "已发货");
    m.insert("order.completed", "已完成");
    m.insert("order.cancelled", "已取消");
    m.insert("order.shipping_info", "收货信息");
    m.insert("order.items", "商品明细");
    m.insert("common.currency", "¥");
    m.insert("common.save", "保存");
    m.insert("common.cancel", "取消");
    m.insert("common.confirm", "确认");
    m.insert("common.back", "返回");
    m.insert("footer.about", "关于我们");
    m.insert("footer.contact", "联系方式");
    m.insert("footer.privacy", "隐私政策");
    m.insert("footer.terms", "服务条款");
    m.insert("footer.copyright", "© 2024 PrintPack 印刷包装. All rights reserved.");
    m.insert("footer.address", "地址: 深圳市宝安区印刷产业园A栋");
    m.insert("footer.phone", "电话: 400-888-8888");
    m.insert("footer.email", "邮箱: service@printpack.com");
    m
}

fn load_en() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("nav.home", "Home");
    m.insert("nav.products", "Products");
    m.insert("nav.quote", "Get Quote");
    m.insert("nav.cart", "Cart");
    m.insert("nav.orders", "My Orders");
    m.insert("nav.login", "Login");
    m.insert("nav.register", "Register");
    m.insert("nav.logout", "Logout");
    m.insert("nav.profile", "Profile");
    m.insert("nav.admin", "Admin");
    m.insert("home.hero_title", "Professional Printing & Packaging");
    m.insert("home.hero_subtitle", "High-quality custom boxes, labels, brochures one-stop service");
    m.insert("home.hero_cta", "Get Quote Now");
    m.insert("home.features_title", "Why Choose Us");
    m.insert("home.features_quality", "Quality Guaranteed");
    m.insert("home.features_quality_desc", "Imported equipment, eco-friendly materials");
    m.insert("home.features_fast", "Fast Delivery");
    m.insert("home.features_fast_desc", "Ships in as fast as 3 days");
    m.insert("home.features_custom", "Custom Made");
    m.insert("home.features_custom_desc", "Free design & prototyping");
    m.insert("home.features_price", "Transparent Pricing");
    m.insert("home.features_price_desc", "Factory direct, no middlemen");
    m.insert("products.title", "Products");
    m.insert("products.filter_category", "Filter by Category");
    m.insert("products.all_categories", "All Categories");
    m.insert("products.get_quote", "Get Quote");
    m.insert("products.detail", "View Details");
    m.insert("products.description", "Description");
    m.insert("products.specifications", "Specifications");
    m.insert("products.material_options", "Material Options");
    m.insert("products.min_qty", "Min Order");
    m.insert("products.add_to_cart", "Add to Cart");
    m.insert("quote.title", "Online Quote");
    m.insert("quote.product", "Select Product");
    m.insert("quote.material", "Material");
    m.insert("quote.width", "Width(cm)");
    m.insert("quote.height", "Height(cm)");
    m.insert("quote.depth", "Depth(cm)");
    m.insert("quote.quantity", "Quantity");
    m.insert("quote.finishing", "Finishing");
    m.insert("quote.notes", "Notes");
    m.insert("quote.submit", "Submit Quote");
    m.insert("quote.price_estimate", "Price Estimate");
    m.insert("quote.unit_price", "Unit Price");
    m.insert("quote.total_price", "Total Price");
    m.insert("cart.title", "Shopping Cart");
    m.insert("cart.empty", "Your cart is empty");
    m.insert("cart.subtotal", "Subtotal");
    m.insert("cart.checkout", "Checkout");
    m.insert("cart.remove", "Remove");
    m.insert("cart.quantity", "Quantity");
    m.insert("auth.login", "Login");
    m.insert("auth.register", "Register");
    m.insert("auth.email", "Email");
    m.insert("auth.password", "Password");
    m.insert("auth.name", "Name");
    m.insert("auth.phone", "Phone");
    m.insert("auth.no_account", "Don't have an account?");
    m.insert("auth.has_account", "Already have an account?");
    m.insert("order.title", "My Orders");
    m.insert("order.number", "Order Number");
    m.insert("order.status", "Status");
    m.insert("order.total", "Total");
    m.insert("order.date", "Date");
    m.insert("order.detail", "Order Details");
    m.insert("order.pending", "Pending");
    m.insert("order.confirmed", "Confirmed");
    m.insert("order.production", "In Production");
    m.insert("order.shipped", "Shipped");
    m.insert("order.completed", "Completed");
    m.insert("order.cancelled", "Cancelled");
    m.insert("order.shipping_info", "Shipping Info");
    m.insert("order.items", "Items");
    m.insert("common.currency", "$");
    m.insert("common.save", "Save");
    m.insert("common.cancel", "Cancel");
    m.insert("common.confirm", "Confirm");
    m.insert("common.back", "Back");
    m.insert("footer.about", "About Us");
    m.insert("footer.contact", "Contact");
    m.insert("footer.privacy", "Privacy Policy");
    m.insert("footer.terms", "Terms of Service");
    m.insert("footer.copyright", "© 2024 PrintPack. All rights reserved.");
    m.insert("footer.address", "Address: Block A, Printing Industrial Park, Shenzhen");
    m.insert("footer.phone", "Phone: +86 400-888-8888");
    m.insert("footer.email", "Email: service@printpack.com");
    m
}

pub fn t(key: &str, lang: &str) -> String {
    let translations = TRANSLATIONS.read().unwrap();
    translations
        .get(lang)
        .and_then(|m| m.get(key))
        .cloned()
        .unwrap_or_else(|| {
            translations
                .get("zh")
                .and_then(|m| m.get(key))
                .cloned()
                .unwrap_or(key.to_string())
        })
}
