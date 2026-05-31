//! 腾讯财经 K 线数据获取模块
//! 
//! 支持 A 股、港股、美股历史K线数据获取
//! API 文档: https://qt.gtimg.cn / https://web.ifzq.gtimg.cn

use super::data::Candle;
use chrono::{DateTime, NaiveDate, Utc};
use reqwest::blocking::get;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 腾讯财经 API 数据源
pub struct TencentDataSource;

/// K 线周期
#[derive(Debug, Clone, Copy, Default)]
pub enum TencentInterval {
    /// 日线
    #[default]
    Day,
    /// 周线
    Week,
    /// 月线
    Month,
    /// 1分钟
    Min1,
    /// 5分钟
    Min5,
    /// 15分钟
    Min15,
    /// 30分钟
    Min30,
    /// 60分钟
    Min60,
}

impl TencentInterval {
    /// 腾讯 API 使用的周期标识
    fn as_str(&self) -> &'static str {
        match self {
            TencentInterval::Day => "day",
            TencentInterval::Week => "week",
            TencentInterval::Month => "month",
            TencentInterval::Min1 => "1",
            TencentInterval::Min5 => "5",
            TencentInterval::Min15 => "15",
            TencentInterval::Min30 => "30",
            TencentInterval::Min60 => "60",
        }
    }
}

/// 股票代码解析（支持多种格式）
/// 支持: sh000001, sz000001, HK00700, US_AAPL, 000001.sh 等格式
pub fn normalize_symbol(symbol: &str) -> String {
    let s = symbol.trim().to_uppercase();
    
    // 已经是标准格式
    if s.starts_with("SH") || s.starts_with("SZ") || s.starts_with("HK") {
        return s.to_lowercase();
    }
    
    // 美股格式: US_AAPL -> us_aapl
    if s.starts_with("US_") {
        return s.to_lowercase();
    }
    
    // 纯数字，根据首位判断市场
    if let Ok(_num) = s.parse::<u64>() {
        if _num < 100_000 {
            // 5位数可能是港股
            if s.len() == 5 {
                return format!("hk{}", s);
            }
        }
        // A股：6开头->上海，0/3开头->深圳
        if s.starts_with('6') {
            return format!("sh{}", s);
        } else {
            return format!("sz{}", s);
        }
    }
    
    // 其他格式：xxx.SH / xxx.SZ
    if s.ends_with(".SH") {
        return format!("sh{}", s.trim_end_matches(".SH"));
    }
    if s.ends_with(".SZ") {
        return format!("sz{}", s.trim_end_matches(".SZ"));
    }
    
    // 默认当作深圳
    format!("sz{}", s)
}

/// 从腾讯财经获取K线数据
/// 
/// # 参数
/// * `symbol` - 股票代码（如 "sh000001", "sz000001", "HK00700"）
/// * `interval` - K线周期
/// * `limit` - 获取数量（最大320）
/// 
/// # 返回
/// Vec<Candle> 按时间升序排列
pub fn fetch_klines(
    symbol: &str,
    interval: TencentInterval,
    limit: usize,
) -> Result<Vec<Candle>, String> {
    let limit = limit.min(320); // 腾讯 API 单次最大 320 条
    
    // 使用复权日线 API
    let api_url = format!(
        "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get?param={},{},,,{},qqt",
        symbol,
        interval.as_str(),
        limit
    );

    let resp = get(&api_url).map_err(|e| format!("请求腾讯财经API失败: {}", e))?;
    
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(format!("腾讯财经API返回错误 ({}): {}", status, body));
    }

    let json_resp: TencentResponse = resp
        .json()
        .map_err(|e| format!("解析腾讯财经数据失败: {}", e))?;

    if json_resp.code != 0 {
        return Err(format!("腾讯财经API返回非零code: {}", json_resp.code));
    }

    // 解析数据 - 取第一个市场的数据
    let mut candles = Vec::new();
    
    if let Some((_, market_data)) = json_resp.data.iter().next() {
        let kline_data = match interval {
            TencentInterval::Day => &market_data.qfqday,
            TencentInterval::Week => &market_data.qfqweek,
            TencentInterval::Month => &market_data.qfqmonth,
            _ => &market_data.day,
        };
        
        for record in kline_data {
            if record.len() < 6 {
                continue;
            }
            
            // 解析日期: "2024-01-15"
            let date_str = record[0].as_str().unwrap_or("");
            let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
                .unwrap_or_default();
            
            let open = parse_json_f64(record.get(1));
            let close = parse_json_f64(record.get(2));
            let high = parse_json_f64(record.get(3));
            let low = parse_json_f64(record.get(4));
            let volume = parse_json_f64(record.get(5));
            
            candles.push(Candle::new(date, open, high, low, close, volume));
        }
    }

    // 按时间升序排列
    candles.sort_by_key(|c| c.timestamp);
    
    if candles.is_empty() {
        return Err(format!(
            "未获取到K线数据 (symbol={}, interval={:?})",
            symbol, interval
        ));
    }

    Ok(candles)
}

/// 批量获取多只股票的K线数据
pub fn fetch_klines_batch(
    symbols: &[&str],
    interval: TencentInterval,
    limit: usize,
) -> HashMap<String, Vec<Candle>> {
    let mut results = HashMap::new();
    
    for symbol in symbols {
        match fetch_klines(symbol, interval, limit) {
            Ok(candles) => {
                results.insert(symbol.to_string(), candles);
            }
            Err(e) => {
                eprintln!("获取 {} 数据失败: {}", symbol, e);
            }
        }
    }
    
    results
}

/// 获取股票实时行情（简洁版）
pub fn fetch_realtime_quote(symbols: &[&str]) -> Result<Vec<RealtimeQuote>, String> {
    let symbols_str: Vec<String> = symbols.iter().map(|s| normalize_symbol(s)).collect();
    let query = symbols_str.join(",");
    
    let url = format!("https://qt.gtimg.cn/q={}", query);
    let resp = get(&url).map_err(|e| format!("请求腾讯实时行情失败: {}", e))?;
    
    let text = resp.text().map_err(|e| format!("读取响应失败: {}", e))?;
    
    // 解析腾讯行情数据格式: v_sh000001="1~上证指数~000001~3032.00~...";
    let mut quotes = Vec::new();
    
    for line in text.lines() {
        if line.starts_with("v_") && line.contains('=') {
            if let Some(data) = line.split('"').nth(1) {
                let fields: Vec<&str> = data.split('~').collect();
                if fields.len() >= 6 {
                    let quote = RealtimeQuote {
                        symbol: fields[2].to_string(),
                        name: fields[1].to_string(),
                        current_price: parse_str_f64(fields.get(3)),
                        prev_close: parse_str_f64(fields.get(4)),
                        open: parse_str_f64(fields.get(5)),
                        volume: parse_str_f64(fields.get(6)),
                        high: parse_str_f64(fields.get(33)),
                        low: parse_str_f64(fields.get(34)),
                        change_pct: parse_str_f64(fields.get(32)),
                    };
                    quotes.push(quote);
                }
            }
        }
    }
    
    Ok(quotes)
}

/// 实时行情数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeQuote {
    pub symbol: String,
    pub name: String,
    pub current_price: f64,
    pub prev_close: f64,
    pub open: f64,
    pub volume: f64,
    pub high: f64,
    pub low: f64,
    pub change_pct: f64,
}

/// 腾讯财经API响应结构
#[derive(Debug, Deserialize)]
struct TencentResponse {
    code: i32,
    data: HashMap<String, TencentMarketData>,
}

#[derive(Debug, Deserialize)]
struct TencentMarketData {
    #[serde(default)]
    day: Vec<Vec<serde_json::Value>>,
    #[serde(default, rename = "qfqday")]
    qfqday: Vec<Vec<serde_json::Value>>,
    #[serde(default, rename = "qfqweek")]
    qfqweek: Vec<Vec<serde_json::Value>>,
    #[serde(default, rename = "qfqmonth")]
    qfqmonth: Vec<Vec<serde_json::Value>>,
}

/// 安全解析 f64 from JSON value
fn parse_json_f64(value: Option<&serde_json::Value>) -> f64 {
    value
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
}

/// 安全解析 f64 from string slice
fn parse_str_f64(s: Option<&&str>) -> f64 {
    s.and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_normalize_symbol() {
        assert_eq!(normalize_symbol("000001"), "sz000001");
        assert_eq!(normalize_symbol("600000"), "sh600000");
        assert_eq!(normalize_symbol("sh000001"), "sh000001");
        assert_eq!(normalize_symbol("sz000001"), "sz000001");
        assert_eq!(normalize_symbol("000001.SZ"), "sz000001");
        assert_eq!(normalize_symbol("600000.SH"), "sh600000");
        assert_eq!(normalize_symbol("700"), "hk700");
    }
    
    #[test]
    fn test_parse_json_f64() {
        assert_eq!(parse_json_f64(None), 0.0);
    }
    
    #[test]
    fn test_parse_str_f64() {
        let s = "123.45";
        assert_eq!(parse_str_f64(Some(&s)), 123.45);
        assert_eq!(parse_str_f64(None), 0.0);
    }
}
