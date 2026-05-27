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

impl DataSource {
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

            // 随机游走：每日波动 ±3%
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
