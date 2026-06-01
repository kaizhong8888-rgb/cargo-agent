use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};

/// K 线数据 (OHLCV)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub timestamp: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl Candle {
    pub fn new(timestamp: DateTime<Utc>, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Self {
        Self {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
        }
    }
}

/// 数据源
pub struct DataSource;

/// 腾讯财经 API 数据格式
#[derive(Deserialize, Debug)]
struct TencentResponse {
    code: String,
    data: TencentData,
}

#[derive(Deserialize, Debug)]
struct TencentData {
    #[serde(rename = "data")]
    data_field: std::collections::HashMap<String, TencentStockData>,
}

#[derive(Deserialize, Debug)]
struct TencentStockData {
    day: Vec<Vec<String>>,
}

impl DataSource {
    /// 从腾讯财经API获取真实日K线数据
    /// symbol: sz000001(上证指数), sz399001(深证成指), sh600519(贵州茅台), hk00700(腾讯)
    /// count: 获取K线数量 (最大1000)
    pub fn from_tencent_api(symbol: &str, count: usize) -> anyhow::Result<Vec<Candle>> {
        let url = format!(
            "https://ifzq.gtimg.cn/appstock/app/fqkline/get?param={symbol},day,,,{count},qfq",
            symbol = symbol
        );

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        let resp = client.get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .header("Referer", "https://finance.qq.com")
            .send()?;

        if !resp.status().is_success() {
            anyhow::bail!("腾讯财经API请求失败: HTTP {}", resp.status());
        }

        let text = resp.text()?;

        // Debug: print first 500 chars
        if text.len() > 500 {
            println!("   📡 API响应 (前500字符): {}", &text[..500]);
        }

        let json: serde_json::Value = serde_json::from_str(&text)?;

        // Check error code
        if let Some(code) = json.get("code").and_then(|c| c.as_i64()) {
            if code != 0 {
                anyhow::bail!("腾讯财经API返回错误: code={}", code);
            }
        }

        // Navigate data structure: {"code":0, "data":{"sh600519":{"day":[...]}, "sh600519":{"data":{"day":[...]}}}}
        let data_obj = json.get("data")
            .ok_or_else(|| anyhow::anyhow!("API响应缺少data字段"))?;

        // Try to find day array - check multiple nesting levels
        let mut day_array: Option<&serde_json::Value> = None;
        let mut found_key = String::new();

        if let Some(obj) = data_obj.as_object() {
            for (key, val) in obj {
                found_key = key.clone();
                // Level 1: data.sh600519.day
                if let Some(day_val) = val.get("day").and_then(|d| d.as_array()) {
                    if !day_val.is_empty() {
                        day_array = Some(val);
                        println!("   📡 腾讯API返回: {} ({} 条记录)", key, day_val.len());
                        break;
                    }
                }
                // Level 2: data.sh600519.data.day (nested)
                if let Some(nested) = val.get("data") {
                    if let Some(day_val) = nested.get("day").and_then(|d| d.as_array()) {
                        if !day_val.is_empty() {
                            day_array = Some(nested);
                            found_key = format!("{}.data", key);
                            println!("   📡 腾讯API返回: {}.data ({} 条记录)", key, day_val.len());
                            break;
                        }
                    }
                }
                // Level 3: check if val itself is the data object with "qfqday" or "hfqday"
                for day_key in &["qfqday", "hfqday", "day"] {
                    if let Some(day_val) = val.get(*day_key).and_then(|d| d.as_array()) {
                        if !day_val.is_empty() {
                            day_array = Some(val);
                            found_key = format!("{}.{}", key, day_key);
                            println!("   📡 腾讯API返回: {}.{} ({} 条记录)", key, day_key, day_val.len());
                            break;
                        }
                    }
                    // Check nested
                    if let Some(nested) = val.get("data") {
                        if let Some(day_val) = nested.get(*day_key).and_then(|d| d.as_array()) {
                            if !day_val.is_empty() {
                                day_array = Some(nested);
                                found_key = format!("{}.data.{}", key, day_key);
                                println!("   📡 腾讯API返回: {}.data.{} ({} 条记录)", key, day_key, day_val.len());
                                break;
                            }
                        }
                    }
                }
                if day_array.is_some() {
                    break;
                }
            }
        }

        let day_array = day_array
            .and_then(|d| d.get("day").or_else(|| d.get("qfqday")).or_else(|| d.get("hfqday")))
            .and_then(|d| d.as_array())
            .ok_or_else(|| anyhow::anyhow!("API返回数据中未找到K线数据 (已检查key: {})", found_key))?;

        let mut candles = Vec::with_capacity(day_array.len());

        for item in day_array {
            let arr = item.as_array()
                .ok_or_else(|| anyhow::anyhow!("K线数据格式错误"))?;

            if arr.len() < 5 {
                continue;
            }

            // 腾讯格式: [日期, 开盘, 收盘, 最高, 最低, 成交量, 成交额, ...]
            let date_str = arr[0].as_str().unwrap_or("");
            let open: f64 = arr[1].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            let close: f64 = arr[2].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            let high: f64 = arr[3].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            let low: f64 = arr[4].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            let volume: f64 = if arr.len() > 5 {
                arr[5].as_str().unwrap_or("0").parse().unwrap_or(0.0)
            } else {
                0.0
            };

            let timestamp = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .ok()
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
                .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
                .unwrap_or_else(Utc::now);

            candles.push(Candle::new(timestamp, open, high, low, close, volume));
        }

        if candles.is_empty() {
            anyhow::bail!("未能解析到任何K线数据");
        }

        // 确保按时间排序 (腾讯可能返回倒序)
        candles.sort_by_key(|c| c.timestamp);

        println!("   ✅ 成功获取 {} 条真实K线数据", candles.len());
        println!("   📅 日期范围: {} ~ {}",
            candles.first().unwrap().timestamp.format("%Y-%m-%d"),
            candles.last().unwrap().timestamp.format("%Y-%m-%d"));

        Ok(candles)
    }

    /// 从 CSV 文件加载 K 线数据
    /// 期望 CSV 格式: timestamp, open, high, low, close, volume
    pub fn from_csv(path: &str) -> anyhow::Result<Vec<Candle>> {
        let mut reader = csv::Reader::from_path(path)?;
        let mut candles = Vec::new();

        for result in reader.records() {
            let record = result?;
            let timestamp: DateTime<Utc> = record[0].parse()?;
            let open: f64 = record[1].parse()?;
            let high: f64 = record[2].parse()?;
            let low: f64 = record[3].parse()?;
            let close: f64 = record[4].parse()?;
            let volume: f64 = record[5].parse()?;

            candles.push(Candle::new(timestamp, open, high, low, close, volume));
        }

        Ok(candles)
    }

    /// 生成模拟价格数据（随机游走）
    pub fn generate_mock(count: usize, start_price: f64) -> Vec<Candle> {
        let mut rng = rand::thread_rng();
        let mut candles = Vec::with_capacity(count);

        let start = Utc::now() - chrono::Duration::hours(count as i64);
        let mut price = start_price;

        for i in 0..count {
            let timestamp = start + chrono::Duration::hours(i as i64);

            let change = rng.gen_range(-0.03..0.03);
            let open = price;
            let close = price * (1.0 + change);
            let high = open.max(close) * (1.0 + rng.gen_range(0.0..0.015));
            let low = open.min(close) * (1.0 - rng.gen_range(0.0..0.015));
            let volume = rng.gen_range(100.0..10_000.0);

            candles.push(Candle::new(timestamp, open, high, low, close, volume));
            price = close;
        }

        candles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mock() {
        let candles = DataSource::generate_mock(100, 100.0);
        assert_eq!(candles.len(), 100);
        assert!(candles[0].close > 0.0);
        // 验证 OHLC 关系
        for c in &candles {
            assert!(c.high >= c.open.max(c.close), "high >= max(open,close)");
            assert!(c.low <= c.open.min(c.close), "low <= min(open,close)");
        }
    }

    #[test]
    fn test_candle_creation() {
        let now = Utc::now();
        let c = Candle::new(now, 100.0, 105.0, 95.0, 102.0, 1000.0);
        assert_eq!(c.open, 100.0);
        assert_eq!(c.close, 102.0);
        assert_eq!(c.volume, 1000.0);
    }
}
