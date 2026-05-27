use serde::{Deserialize, Serialize};

/// MACD 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacdOutput {
    pub macd_line: Vec<f64>,
    pub signal_line: Vec<f64>,
    pub histogram: Vec<f64>,
}

/// 简单移动平均线 (SMA)
pub fn sma(data: &[f64], period: usize) -> Vec<f64> {
    if data.len() < period || period == 0 {
        return vec![f64::NAN; data.len()];
    }

    let mut result = vec![f64::NAN; period - 1];
    let mut sum: f64 = data[..period].iter().sum();

    result.push(sum / period as f64);

    for i in period..data.len() {
        sum += data[i] - data[i - period];
        result.push(sum / period as f64);
    }

    result
}

/// 指数移动平均线 (EMA)
pub fn ema(data: &[f64], period: usize) -> Vec<f64> {
    if data.len() < period || period == 0 {
        return vec![f64::NAN; data.len()];
    }

    let mut result = vec![f64::NAN; period - 1];
    let multiplier = 2.0 / (period as f64 + 1.0);

    // 初始值使用 SMA
    let initial_sma: f64 = data[..period].iter().sum::<f64>() / period as f64;
    result.push(initial_sma);

    for i in period..data.len() {
        let ema_val = (data[i] - result[i - 1]) * multiplier + result[i - 1];
        result.push(ema_val);
    }

    result
}

/// 相对强弱指标 (RSI)
pub fn rsi(data: &[f64], period: usize) -> Vec<f64> {
    if data.len() < period + 1 || period == 0 {
        return vec![f64::NAN; data.len()];
    }

    let mut result = vec![f64::NAN; period]; // 前 period 个无法计算
    let mut gains = Vec::with_capacity(data.len());
    let mut losses = Vec::with_capacity(data.len());

    // 计算涨跌
    for i in 1..data.len() {
        let diff = data[i] - data[i - 1];
        if diff > 0.0 {
            gains.push(diff);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(-diff);
        }
    }

    // 初始平均涨跌幅 (SMA)
    let mut avg_gain: f64 = gains[..period].iter().sum::<f64>() / period as f64;
    let mut avg_loss: f64 = losses[..period].iter().sum::<f64>() / period as f64;

    // 第一个 RSI
    let rs = if avg_loss == 0.0 {
        100.0
    } else {
        avg_gain / avg_loss
    };
    result.push(100.0 - (100.0 / (1.0 + rs)));

    // 后续使用平滑计算
    for i in period..gains.len() {
        avg_gain = (avg_gain * (period as f64 - 1.0) + gains[i]) / period as f64;
        avg_loss = (avg_loss * (period as f64 - 1.0) + losses[i]) / period as f64;

        let rs = if avg_loss == 0.0 {
            100.0
        } else {
            avg_gain / avg_loss
        };
        result.push(100.0 - (100.0 / (1.0 + rs)));
    }

    result
}

/// 平滑异同移动平均线 (MACD)
pub fn macd(data: &[f64], fast: usize, slow: usize, signal: usize) -> MacdOutput {
    let ema_fast = ema(data, fast);
    let ema_slow = ema(data, slow);

    // MACD 线 = 快线EMA - 慢线EMA
    let macd_line: Vec<f64> = ema_fast
        .iter()
        .zip(ema_slow.iter())
        .map(|(f, s)| f - s)
        .collect();

    // Signal 线 = MACD 的 EMA
    let signal_line = ema(&macd_line, signal);

    // 柱状图 = MACD 线 - Signal 线
    let histogram: Vec<f64> = macd_line
        .iter()
        .zip(signal_line.iter())
        .map(|(m, s)| m - s)
        .collect();

    MacdOutput {
        macd_line,
        signal_line,
        histogram,
    }
}

/// 布林带 (Bollinger Bands)
#[derive(Debug, Clone)]
pub struct BollingerBands {
    pub middle: Vec<f64>,
    pub upper: Vec<f64>,
    pub lower: Vec<f64>,
}

pub fn bollinger_bands(data: &[f64], period: usize, std_dev: f64) -> BollingerBands {
    if data.len() < period || period == 0 {
        return BollingerBands {
            middle: vec![f64::NAN; data.len()],
            upper: vec![f64::NAN; data.len()],
            lower: vec![f64::NAN; data.len()],
        };
    }
    let middle = sma(data, period);
    let mut upper = vec![f64::NAN; data.len()];
    let mut lower = vec![f64::NAN; data.len()];

    // Use saturating_sub to avoid underflow in debug mode
    for i in (period.saturating_sub(1))..data.len() {
        let mean = middle[i];
        let start_idx = i.saturating_sub(period).saturating_add(1);
        let variance: f64 = data[start_idx..=i]
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>()
            / period as f64;
        let std = variance.sqrt();

        upper[i] = mean + std_dev * std;
        lower[i] = mean - std_dev * std;
    }

    BollingerBands {
        middle,
        upper,
        lower,
    }
}

/// 真实波幅 (ATR - Average True Range)
pub fn atr(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    if highs.len() < 2 || period == 0 {
        return vec![f64::NAN; highs.len()];
    }

    let mut tr_values = Vec::with_capacity(highs.len());
    tr_values.push(f64::NAN); // 第一根没有TR

    for i in 1..highs.len() {
        let high_low = highs[i] - lows[i];
        let high_close = (highs[i] - closes[i - 1]).abs();
        let low_close = (lows[i] - closes[i - 1]).abs();
        let tr = high_low.max(high_close).max(low_close);
        tr_values.push(tr);
    }

    // 使用 EMA 计算 ATR
    ema(&tr_values, period)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sma() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = sma(&data, 3);
        assert!(result[0..2].iter().all(|v| v.is_nan()));
        assert!((result[2] - 2.0).abs() < 1e-10);
        assert!((result[3] - 3.0).abs() < 1e-10);
        assert!((result[4] - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_rsi() {
        // 简单的上涨趋势：RSI 应该 > 50
        let data: Vec<f64> = (1..=20).map(|x| x as f64).collect();
        let result = rsi(&data, 14);
        let last = result.last().unwrap();
        assert!(*last > 50.0);
        assert!(*last <= 100.0);
    }

    #[test]
    fn test_macd() {
        let data: Vec<f64> = (1..=50).map(|x| x as f64).collect();
        let result = macd(&data, 12, 26, 9);
        assert_eq!(result.macd_line.len(), 50);
        assert_eq!(result.signal_line.len(), 50);
        assert_eq!(result.histogram.len(), 50);
        // 上涨趋势中 MACD 应该为正
        assert!(result.macd_line.last().unwrap() > &0.0);
    }
}
