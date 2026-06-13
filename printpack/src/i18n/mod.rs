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
    // Nav
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
    // Home
    m.insert("home.hero_title", "专业印刷包装定制服务");
    m.insert("home.hero_subtitle", "高品质包装盒、标签、画册一站式定制");
    m.insert("home.hero_cta", "立即获取报价");
    m.insert("home.features_title", "我们的优势");
    // Products
    m.insert("products.title", "产品中心");
    m.insert("products.filter_category", "按分类筛选");
    m.insert("products.all_categories", "全部分类");
    m.insert("products.get_quote", "获取报价");
    m.insert("products.detail", "查看详情");
    // Quote
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
    // Cart
    m.insert("cart.title", "购物车");
    m.insert("cart.empty", "购物车是空的");
    m.insert("cart.empty_hint", "快去挑选您需要的产品吧");
    m.insert("cart.browse_products", "浏览产品");
    m.insert("cart.material", "材质");
    m.insert("cart.size", "尺寸");
    m.insert("cart.finishing", "工艺");
    m.insert("cart.remove", "移除");
    m.insert("cart.summary", "订单摘要");
    m.insert("cart.items_count", "商品数量");
    m.insert("cart.items", "件");
    m.insert("cart.total_quantity", "总数量");
    m.insert("cart.units", "个");
    m.insert("cart.total", "合计");
    m.insert("cart.free_shipping", "🎉 已满足免运费条件");
    m.insert("cart.free_shipping_hint", "再买 ¥");
    m.insert("cart.checkout", "去结算");
    m.insert("cart.continue_shopping", "继续购物");
    // Auth
    m.insert("auth.login", "登录");
    m.insert("auth.register", "注册");
    m.insert("auth.email", "邮箱");
    m.insert("auth.password", "密码");
    m.insert("auth.no_account", "还没有账号？");
    m.insert("auth.has_account", "已有账号？");
    // Register
    m.insert("register.title", "注册账号");
    m.insert("register.subtitle", "创建您的 PrintPack 账号");
    m.insert("register.username", "用户名");
    m.insert("register.username_hint", "3-32个字符");
    m.insert("register.email", "邮箱");
    m.insert("register.email_hint", "example@company.com");
    m.insert("register.phone", "手机号");
    m.insert("register.phone_hint", "选填");
    m.insert("register.company", "公司名称");
    m.insert("register.company_hint", "选填");
    m.insert("register.password", "密码");
    m.insert("register.password_hint", "至少8个字符");
    m.insert("register.confirm_password", "确认密码");
    m.insert("register.confirm_password_hint", "再次输入密码");
    m.insert("register.agree_terms", "我已阅读并同意");
    m.insert("register.terms", "服务条款");
    m.insert("register.submit", "立即注册");
    m.insert("register.has_account", "已有账号？");
    m.insert("register.login_link", "立即登录");
    m.insert("register.password_mismatch", "两次输入的密码不一致");
    m.insert("register.success", "注册成功！正在跳转到登录页面...");
    m.insert("register.error", "注册失败，请稍后重试");
    m.insert("register.network_error", "网络错误，请检查网络连接");
    // Orders
    m.insert("orders.title", "我的订单");
    m.insert("orders.empty", "暂无订单");
    m.insert("orders.empty_hint", "下单后这里会显示您的订单");
    m.insert("orders.browse_products", "浏览产品");
    m.insert("orders.all_status", "全部状态");
    m.insert("orders.status.pending", "待确认");
    m.insert("orders.status.confirmed", "已确认");
    m.insert("orders.status.producing", "生产中");
    m.insert("orders.status.shipped", "已发货");
    m.insert("orders.status.delivered", "已送达");
    m.insert("orders.status.completed", "已完成");
    m.insert("orders.status.cancelled", "已取消");
    m.insert("orders.view_detail", "查看详情");
    m.insert("orders.cancel", "取消订单");
    m.insert("orders.confirm_delivery", "确认收货");
    m.insert("orders.confirm_cancel", "确定要取消此订单吗？");
    m.insert("orders.cancel_failed", "取消失败，请联系客服");
    m.insert("orders.confirm_failed", "操作失败，请重试");
    m.insert("orders.confirm_delivery_prompt", "确认已收到货物？");
    m.insert("orders.total", "总金额");
    m.insert("orders.detail_title", "订单 {order_number}");
    m.insert("orders.order_date", "下单时间");
    m.insert("orders.tracking", "物流单号");
    m.insert("orders.tracking_history", "物流跟踪");
    m.insert("orders.items", "商品明细");
    m.insert("orders.material", "材质");
    m.insert("orders.size", "尺寸");
    m.insert("orders.quantity", "数量");
    m.insert("orders.finishing", "工艺");
    m.insert("orders.notes", "备注");
    m.insert("orders.summary", "费用明细");
    m.insert("orders.subtotal", "商品小计");
    m.insert("orders.discount", "优惠");
    m.insert("orders.shipping", "运费");
    m.insert("orders.free_shipping", "免运费");
    m.insert("orders.shipping_info", "收货信息");
    m.insert("orders.payment_info", "支付信息");
    m.insert("orders.payment_method", "支付方式");
    m.insert("orders.payment_status", "支付状态");
    m.insert("orders.paid_at", "支付时间");
    m.insert("orders.payment.alipay", "支付宝");
    m.insert("orders.payment.wechat", "微信支付");
    m.insert("orders.payment.bank", "银行转账");
    m.insert("orders.payment.cod", "货到付款");
    m.insert("orders.payment_status.unpaid", "未支付");
    m.insert("orders.payment_status.paid", "已支付");
    m.insert("orders.payment_status.refunded", "已退款");
    m.insert("orders.back_to_list", "返回订单列表");
    // Contact
    m.insert("contact.title", "联系我们");
    m.insert("contact.subtitle", "有任何问题？请随时联系我们");
    m.insert("contact.info_title", "联系方式");
    m.insert("contact.phone", "电话");
    m.insert("contact.phone_hours", "周一至周六 9:00-18:00");
    m.insert("contact.email", "邮箱");
    m.insert("contact.address", "地址");
    m.insert("contact.address_detail", "深圳市宝安区印刷产业园A栋");
    m.insert("contact.wechat", "微信");
    m.insert("contact.form_title", "在线留言");
    m.insert("contact.name", "姓名");
    m.insert("contact.company", "公司名称");
    m.insert("contact.email_label", "邮箱");
    m.insert("contact.phone_label", "电话");
    m.insert("contact.subject", "主题");
    m.insert("contact.subject_quote", "产品咨询");
    m.insert("contact.subject_order", "订单问题");
    m.insert("contact.subject_complaint", "投诉建议");
    m.insert("contact.subject_cooperation", "合作洽谈");
    m.insert("contact.subject_other", "其他");
    m.insert("contact.message", "留言内容");
    m.insert("contact.submit", "提交留言");
    m.insert("contact.success", "留言已发送，我们会尽快回复您！");
    m.insert("contact.error", "发送失败，请稍后重试");
    m.insert("contact.network_error", "网络错误，请检查网络连接");
    // About
    m.insert("about.title", "关于我们");
    m.insert("about.story_title", "品牌故事");
    m.insert("about.story_p1", "PrintPack 成立于2010年，是一家集设计、生产、销售于一体的专业印刷包装企业。");
    m.insert("about.story_p2", "我们拥有海德堡印刷机、自动模切机等先进设备，为客户提供从设计到成品的一站式服务。");
    m.insert("about.values_title", "我们的价值观");
    m.insert("about.quality", "品质至上");
    m.insert("about.quality_desc", "每一件产品都经过严格质检");
    m.insert("about.trust", "诚信为本");
    m.insert("about.trust_desc", "以诚相待，赢得客户信赖");
    m.insert("about.sustainability", "绿色环保");
    m.insert("about.sustainability_desc", "使用环保材料，践行可持续发展");
    m.insert("about.innovation", "持续创新");
    m.insert("about.innovation_desc", "引入新技术，提升产品质量");
    m.insert("about.capabilities_title", "生产能力");
    m.insert("about.cap_heidelberg", "海德堡对开四色印刷机，日产5万印");
    m.insert("about.cap_diecut", "全自动模切机，精度±0.1mm");
    m.insert("about.cap_ctp", "CTP直接制版，色彩还原度98%");
    m.insert("about.cap_qc", "三道质检流程，合格率99.5%");
    m.insert("about.stat_years", "年行业经验");
    m.insert("about.stat_clients", "服务客户");
    m.insert("about.stat_orders", "累计订单");
    m.insert("about.stat_quality", "产品合格率");
    // Common
    m.insert("common.currency", "¥");
    m.insert("common.save", "保存");
    m.insert("common.cancel", "取消");
    m.insert("common.confirm", "确认");
    m.insert("common.back", "返回");
    m.insert("common.prev", "上一页");
    m.insert("common.next", "下一页");
    m.insert("common.page", "第");
    // Footer
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
    m.insert("home.hero_subtitle", "High-quality custom boxes, labels, brochures");
    m.insert("home.hero_cta", "Get Quote Now");
    m.insert("products.title", "Products");
    m.insert("products.filter_category", "Filter by Category");
    m.insert("products.all_categories", "All Categories");
    m.insert("products.get_quote", "Get Quote");
    m.insert("products.detail", "View Details");
    m.insert("quote.title", "Online Quote");
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
    m.insert("cart.empty_hint", "Browse our products to get started");
    m.insert("cart.browse_products", "Browse Products");
    m.insert("cart.material", "Material");
    m.insert("cart.size", "Size");
    m.insert("cart.finishing", "Finishing");
    m.insert("cart.remove", "Remove");
    m.insert("cart.summary", "Order Summary");
    m.insert("cart.items_count", "Items");
    m.insert("cart.items", "items");
    m.insert("cart.total_quantity", "Total Qty");
    m.insert("cart.units", "pcs");
    m.insert("cart.total", "Total");
    m.insert("cart.free_shipping", "🎉 Free shipping unlocked!");
    m.insert("cart.free_shipping_hint", "Add ¥");
    m.insert("cart.checkout", "Checkout");
    m.insert("cart.continue_shopping", "Continue Shopping");
    m.insert("auth.login", "Login");
    m.insert("auth.register", "Register");
    m.insert("auth.email", "Email");
    m.insert("auth.password", "Password");
    m.insert("auth.no_account", "Don't have an account?");
    m.insert("auth.has_account", "Already have an account?");
    m.insert("register.title", "Create Account");
    m.insert("register.subtitle", "Join PrintPack today");
    m.insert("register.username", "Username");
    m.insert("register.email", "Email");
    m.insert("register.phone", "Phone");
    m.insert("register.company", "Company");
    m.insert("register.password", "Password");
    m.insert("register.confirm_password", "Confirm Password");
    m.insert("register.agree_terms", "I agree to the");
    m.insert("register.terms", "Terms of Service");
    m.insert("register.submit", "Register");
    m.insert("register.has_account", "Already have an account?");
    m.insert("register.login_link", "Login");
    m.insert("register.password_mismatch", "Passwords do not match");
    m.insert("register.success", "Registration successful! Redirecting to login...");
    m.insert("register.error", "Registration failed, please try again");
    m.insert("register.network_error", "Network error, please check your connection");
    m.insert("orders.title", "My Orders");
    m.insert("orders.empty", "No orders yet");
    m.insert("orders.empty_hint", "Your orders will appear here");
    m.insert("orders.browse_products", "Browse Products");
    m.insert("orders.all_status", "All Status");
    m.insert("orders.status.pending", "Pending");
    m.insert("orders.status.confirmed", "Confirmed");
    m.insert("orders.status.producing", "In Production");
    m.insert("orders.status.shipped", "Shipped");
    m.insert("orders.status.delivered", "Delivered");
    m.insert("orders.status.completed", "Completed");
    m.insert("orders.status.cancelled", "Cancelled");
    m.insert("orders.view_detail", "View Details");
    m.insert("orders.cancel", "Cancel Order");
    m.insert("orders.confirm_delivery", "Confirm Delivery");
    m.insert("orders.confirm_cancel", "Are you sure you want to cancel?");
    m.insert("orders.cancel_failed", "Failed to cancel, please contact support");
    m.insert("orders.confirm_failed", "Operation failed, please try again");
    m.insert("orders.confirm_delivery_prompt", "Confirm you have received the goods?");
    m.insert("orders.total", "Total");
    m.insert("orders.detail_title", "Order {order_number}");
    m.insert("orders.order_date", "Order Date");
    m.insert("orders.tracking", "Tracking Number");
    m.insert("orders.tracking_history", "Tracking History");
    m.insert("orders.items", "Items");
    m.insert("orders.material", "Material");
    m.insert("orders.size", "Size");
    m.insert("orders.quantity", "Quantity");
    m.insert("orders.finishing", "Finishing");
    m.insert("orders.notes", "Notes");
    m.insert("orders.summary", "Summary");
    m.insert("orders.subtotal", "Subtotal");
    m.insert("orders.discount", "Discount");
    m.insert("orders.shipping", "Shipping");
    m.insert("orders.free_shipping", "Free Shipping");
    m.insert("orders.shipping_info", "Shipping Address");
    m.insert("orders.payment_info", "Payment Info");
    m.insert("orders.payment_method", "Payment Method");
    m.insert("orders.payment_status", "Payment Status");
    m.insert("orders.paid_at", "Paid At");
    m.insert("orders.payment.alipay", "Alipay");
    m.insert("orders.payment.wechat", "WeChat Pay");
    m.insert("orders.payment.bank", "Bank Transfer");
    m.insert("orders.payment.cod", "Cash on Delivery");
    m.insert("orders.payment_status.unpaid", "Unpaid");
    m.insert("orders.payment_status.paid", "Paid");
    m.insert("orders.payment_status.refunded", "Refunded");
    m.insert("orders.back_to_list", "Back to Orders");
    m.insert("contact.title", "Contact Us");
    m.insert("contact.subtitle", "Have questions? We'd love to hear from you");
    m.insert("contact.info_title", "Contact Info");
    m.insert("contact.phone", "Phone");
    m.insert("contact.phone_hours", "Mon-Sat 9:00-18:00");
    m.insert("contact.email", "Email");
    m.insert("contact.address", "Address");
    m.insert("contact.address_detail", "Block A, Printing Industrial Park, Shenzhen");
    m.insert("contact.wechat", "WeChat");
    m.insert("contact.form_title", "Send a Message");
    m.insert("contact.name", "Name");
    m.insert("contact.company", "Company");
    m.insert("contact.email_label", "Email");
    m.insert("contact.phone_label", "Phone");
    m.insert("contact.subject", "Subject");
    m.insert("contact.subject_quote", "Product Inquiry");
    m.insert("contact.subject_order", "Order Issue");
    m.insert("contact.subject_complaint", "Complaint");
    m.insert("contact.subject_cooperation", "Cooperation");
    m.insert("contact.subject_other", "Other");
    m.insert("contact.message", "Message");
    m.insert("contact.submit", "Send Message");
    m.insert("contact.success", "Message sent! We'll get back to you soon.");
    m.insert("contact.error", "Failed to send, please try again");
    m.insert("contact.network_error", "Network error, please check your connection");
    m.insert("about.title", "About Us");
    m.insert("about.story_title", "Our Story");
    m.insert("about.story_p1", "PrintPack was founded in 2010 as a professional printing and packaging company.");
    m.insert("about.story_p2", "We provide one-stop service from design to finished product.");
    m.insert("about.values_title", "Our Values");
    m.insert("about.quality", "Quality First");
    m.insert("about.quality_desc", "Strict quality control on every product");
    m.insert("about.trust", "Integrity");
    m.insert("about.trust_desc", "Building trust through honest business");
    m.insert("about.sustainability", "Sustainability");
    m.insert("about.sustainability_desc", "Eco-friendly materials and practices");
    m.insert("about.innovation", "Innovation");
    m.insert("about.innovation_desc", "Embracing new technologies");
    m.insert("about.capabilities_title", "Production Capabilities");
    m.insert("about.cap_heidelberg", "Heidelberg 4-color press, 50K impressions/day");
    m.insert("about.cap_diecut", "Auto die-cutting machine, ±0.1mm precision");
    m.insert("about.cap_ctp", "CTP platemaking, 98% color accuracy");
    m.insert("about.cap_qc", "3-step QC process, 99.5% pass rate");
    m.insert("about.stat_years", "Years Experience");
    m.insert("about.stat_clients", "Clients Served");
    m.insert("about.stat_orders", "Total Orders");
    m.insert("about.stat_quality", "Quality Rate");
    m.insert("common.currency", "$");
    m.insert("common.save", "Save");
    m.insert("common.cancel", "Cancel");
    m.insert("common.confirm", "Confirm");
    m.insert("common.back", "Back");
    m.insert("common.prev", "Previous");
    m.insert("common.next", "Next");
    m.insert("common.page", "Page");
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

/// I18n state for Axum
#[derive(Clone)]
pub struct I18nState {
    pub default_lang: String,
}

impl I18nState {
    pub fn new() -> Self {
        I18nState {
            default_lang: "zh".to_string(),
        }
    }
}
