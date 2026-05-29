use serde::{Deserialize, Serialize};

/// MACD 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacdOutput {
    pub macd_line: Vec<f64>,
    pub signal_line: Vec<f64>,
    pub histogram: Vec<f64>,
}

/// 布林带 (Bollinger Bands)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BollingerBands {
    pub middle: Vec<f64>,
    pub upper: Vec<f64>,
    pub lower: Vec<f64>,
}

/// 随机指标 (KDJ / Stochastic Oscillator) 输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StochasticOutput {
    pub k: Vec<f64>, // %K 快线
    pub d: Vec<f64>, // %D 慢线 (信号线)
    pub j: Vec<f64>, // J 值 = 3*K - 2*D
}

/// 一目均衡表 (Ichimoku Cloud) 输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IchimokuOutput {
    pub tenkan_sen: Vec<f64>,    // 转换线 (9)
    pub kijun_sen: Vec<f64>,     // 基准线 (26)
    pub senkou_span_a: Vec<f64>, // 先行带A
    pub senkou_span_b: Vec<f64>, // 先行带B
    pub chikou_span: Vec<f64>,   // 迟行带
}

/// SuperTrend 输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuperTrendOutput {
    pub trend: Vec<f64>,    // 趋势线 (上轨或下轨)
    pub direction: Vec<i8>, // 方向: 1=多头, -1=空头
}

/// Keltner Channels 输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeltnerChannels {
    pub middle: Vec<f64>, // EMA 中线
    pub upper: Vec<f64>,  // 上轨
    pub lower: Vec<f64>,  // 下轨
}

// ========================================================================
// 简单移动平均线 (SMA)
// ========================================================================
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

// ========================================================================
// 指数移动平均线 (EMA)
// ========================================================================
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

// ========================================================================
// 加权移动平均线 (WMA - Weighted Moving Average)
// ========================================================================
pub fn wma(data: &[f64], period: usize) -> Vec<f64> {
    if data.len() < period || period == 0 {
        return vec![f64::NAN; data.len()];
    }

    let mut result = vec![f64::NAN; period - 1];
    let denominator = period * (period + 1) / 2;

    for i in (period - 1)..data.len() {
        let start = i + 1 - period;
        let weighted_sum: f64 = data[start..=i]
            .iter()
            .enumerate()
            .map(|(j, v)| v * (j as f64 + 1.0))
            .sum();
        result.push(weighted_sum / denominator as f64);
    }

    result
}

// ========================================================================
// 相对强弱指标 (RSI)
// ========================================================================
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

// ========================================================================
// 平滑异同移动平均线 (MACD)
// ========================================================================
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

// ========================================================================
// 布林带 (Bollinger Bands)
// ========================================================================
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

// ========================================================================
// 真实波幅 (ATR - Average True Range)
// ========================================================================
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

// ========================================================================
// 随机指标 (KDJ / Stochastic Oscillator)
// 公式: %K = (close - low_n) / (high_n - low_n) * 100
//       %D = SMA(%K, 3)
//       J  = 3 * %K - 2 * %D
// ========================================================================
pub fn stochastic(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    k_period: usize,
    d_period: usize,
) -> StochasticOutput {
    let n = closes.len();
    let mut raw_k = vec![f64::NAN; n];

    for i in (k_period - 1)..n {
        let start = i + 1 - k_period;
        let high_max = highs[start..=i]
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let low_min = lows[start..=i]
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min);

        if (high_max - low_min).abs() > f64::EPSILON {
            raw_k[i] = (closes[i] - low_min) / (high_max - low_min) * 100.0;
        } else {
            raw_k[i] = 50.0; // 价格无波动时居中
        }
    }

    let k = sma(&raw_k, d_period); // %K 通常使用 d_period=3 的平滑
    let d = sma(&k, d_period); // %D 是 %K 的移动平均
    let j: Vec<f64> = k
        .iter()
        .zip(d.iter())
        .map(|(kv, dv)| {
            if kv.is_nan() || dv.is_nan() {
                f64::NAN
            } else {
                3.0 * kv - 2.0 * dv
            }
        })
        .collect();

    StochasticOutput { k, d, j }
}

// ========================================================================
// 威廉指标 (Williams %R)
// 公式: %R = (high_n - close) / (high_n - low_n) * 100
// 与随机指标方向相反 (值越低越超卖)
// ========================================================================
pub fn williams_r(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    let n = closes.len();
    if n < period || period == 0 {
        return vec![f64::NAN; n];
    }

    let mut result = vec![f64::NAN; period - 1];

    for i in (period - 1)..n {
        let start = i + 1 - period;
        let high_max = highs[start..=i]
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let low_min = lows[start..=i]
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min);

        if (high_max - low_min).abs() > f64::EPSILON {
            result.push((closes[i] - high_max) / (high_max - low_min) * 100.0);
        } else {
            result.push(-50.0);
        }
    }

    result
}

// ========================================================================
// 能量潮 (OBV - On Balance Volume)
// ========================================================================
pub fn obv(closes: &[f64], volumes: &[f64]) -> Vec<f64> {
    let n = closes.len();
    if n < 2 {
        return vec![f64::NAN; n];
    }

    let mut result = vec![f64::NAN; n]; // 第一根为 NAN
    let mut obv_val = 0.0;

    for i in 1..n {
        if closes[i] > closes[i - 1] {
            obv_val += volumes[i];
        } else if closes[i] < closes[i - 1] {
            obv_val -= volumes[i];
        }
        // 收盘价不变，OBV 不变
        result[i] = obv_val;
    }

    result
}

// ========================================================================
// 一目均衡表 (Ichimoku Cloud)
// ========================================================================
pub fn ichimoku(highs: &[f64], lows: &[f64], closes: &[f64]) -> IchimokuOutput {
    let n = closes.len();

    // 转换线 (Tenkan-sen) = (9日内最高+最低)/2
    let tenkan_sen = ichimoku_line(highs, lows, 9, n);

    // 基准线 (Kijun-sen) = (26日内最高+最低)/2
    let kijun_sen = ichimoku_line(highs, lows, 26, n);

    // 先行带 A (Senkou Span A) = (转换线 + 基准线) / 2，向前平移26
    let mut senkou_span_a = vec![f64::NAN; n];
    for i in 0..n {
        if i + 26 < n && !tenkan_sen[i].is_nan() && !kijun_sen[i].is_nan() {
            senkou_span_a[i + 26] = (tenkan_sen[i] + kijun_sen[i]) / 2.0;
        }
    }

    // 先行带 B (Senkou Span B) = (52日内最高+最低)/2，向前平移26
    let span_b = ichimoku_line(highs, lows, 52, n);
    let mut senkou_span_b = vec![f64::NAN; n];
    for i in 0..n {
        if i + 26 < n && !span_b[i].is_nan() {
            senkou_span_b[i + 26] = span_b[i];
        }
    }

    // 迟行带 (Chikou Span) = 当前收盘价，向后平移26
    let mut chikou_span = vec![f64::NAN; n];
    for i in 0..n {
        if i >= 26 {
            chikou_span[i - 26] = closes[i];
        }
    }

    IchimokuOutput {
        tenkan_sen,
        kijun_sen,
        senkou_span_a,
        senkou_span_b,
        chikou_span,
    }
}

fn ichimoku_line(highs: &[f64], lows: &[f64], period: usize, n: usize) -> Vec<f64> {
    if n < period || period == 0 {
        return vec![f64::NAN; n];
    }
    let mut result = vec![f64::NAN; period - 1];
    for i in (period - 1)..n {
        let start = i + 1 - period;
        let h_max = highs[start..=i]
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let l_min = lows[start..=i]
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min);
        result.push((h_max + l_min) / 2.0);
    }
    result
}

// ========================================================================
// SuperTrend 指标
// ========================================================================
pub fn supertrend(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    atr_period: usize,
    multiplier: f64,
) -> SuperTrendOutput {
    let n = closes.len();
    let atr_values = atr(highs, lows, closes, atr_period);

    let mut trend = vec![f64::NAN; n];
    let mut direction = vec![0i8; n];

    if n < 2 {
        return SuperTrendOutput { trend, direction };
    }

    // 计算基础上下轨
    let mut upper_band = vec![f64::NAN; n];
    let mut lower_band = vec![f64::NAN; n];

    for i in 1..n {
        if atr_values[i].is_nan() {
            continue;
        }
        let hl2 = (highs[i] + lows[i]) / 2.0;
        let atr_val = atr_values[i];
        upper_band[i] = hl2 + multiplier * atr_val;
        lower_band[i] = hl2 - multiplier * atr_val;
    }

    // 确定初始方向
    if n > atr_period {
        direction[atr_period] = 1; // 默认多头
        trend[atr_period] = lower_band[atr_period];
    }

    // 逐根计算 SuperTrend
    for i in (atr_period + 1)..n {
        if upper_band[i].is_nan() || lower_band[i].is_nan() {
            continue;
        }

        if direction[i - 1] == 1 {
            // 之前是多头
            if closes[i] <= lower_band[i] {
                direction[i] = -1;
                trend[i] = upper_band[i];
            } else {
                direction[i] = 1;
                trend[i] = lower_band[i].min(trend[i - 1]); // 上移下轨
            }
        } else {
            // 之前是空头
            if closes[i] >= upper_band[i] {
                direction[i] = 1;
                trend[i] = lower_band[i];
            } else {
                direction[i] = -1;
                trend[i] = upper_band[i].max(trend[i - 1]); // 下移上轨
            }
        }
    }

    SuperTrendOutput { trend, direction }
}

// ========================================================================
// Keltner Channels (凯尔特纳通道)
// ========================================================================
pub fn keltner_channels(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    ema_period: usize,
    atr_period: usize,
    multiplier: f64,
) -> KeltnerChannels {
    let n = closes.len();
    let middle = ema(closes, ema_period);
    let atr_values = atr(highs, lows, closes, atr_period);

    let mut upper = vec![f64::NAN; n];
    let mut lower = vec![f64::NAN; n];

    for i in 0..n {
        if !middle[i].is_nan() && !atr_values[i].is_nan() {
            upper[i] = middle[i] + multiplier * atr_values[i];
            lower[i] = middle[i] - multiplier * atr_values[i];
        }
    }

    KeltnerChannels {
        middle,
        upper,
        lower,
    }
}

// ========================================================================
// 抛物线转向 (Parabolic SAR)
// ========================================================================
pub fn parabolic_sar(
    highs: &[f64],
    lows: &[f64],
    acceleration: f64,
    max_acceleration: f64,
) -> Vec<f64> {
    let n = highs.len();
    let mut sar = vec![f64::NAN; n];
    let mut ep = vec![0.0; n]; // 极点价格
    let mut af = acceleration; // 加速因子

    if n < 2 {
        return sar;
    }

    // 初始化趋势方向
    let mut trend: i8 = if highs[1] > highs[0] { 1 } else { -1 };
    if trend == 1 {
        sar[0] = lows[0];
        ep[0] = highs[0];
    } else {
        sar[0] = highs[0];
        ep[0] = lows[0];
    }

    for i in 1..n {
        sar[i] = sar[i - 1] + af * (ep[i - 1] - sar[i - 1]);

        // 确保 SAR 不会穿越价格
        if trend == 1 {
            if sar[i] > lows[i].min(lows[i - 1]) {
                sar[i] = lows[i].min(lows[i - 1]);
            }
        } else {
            if sar[i] < highs[i].max(highs[i - 1]) {
                sar[i] = highs[i].max(highs[i - 1]);
            }
        }

        // 检测趋势反转
        if trend == 1 && lows[i] < sar[i] {
            // 多头→空头
            trend = -1;
            sar[i] = ep[i - 1];
            af = acceleration;
            ep[i] = lows[i];
        } else if trend == -1 && highs[i] > sar[i] {
            // 空头→多头
            trend = 1;
            sar[i] = ep[i - 1];
            af = acceleration;
            ep[i] = highs[i];
        } else {
            // 更新极点价格和加速因子
            if trend == 1 {
                if highs[i] > ep[i - 1] {
                    ep[i] = highs[i];
                    af = (af + acceleration).min(max_acceleration);
                } else {
                    ep[i] = ep[i - 1];
                }
            } else {
                if lows[i] < ep[i - 1] {
                    ep[i] = lows[i];
                    af = (af + acceleration).min(max_acceleration);
                } else {
                    ep[i] = ep[i - 1];
                }
            }
        }
    }

    sar
}

// ========================================================================
// 测试
// ========================================================================
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
    fn test_wma() {
        let data = vec![1.0, 2.0, 3.0];
        let result = wma(&data, 3);
        // (1*1 + 2*2 + 3*3) / (1+2+3) = (1+4+9)/6 = 14/6 = 2.333...
        assert!((result[2] - 14.0 / 6.0).abs() < 1e-10);
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

    #[test]
    fn test_stochastic() {
        let highs = vec![
            110.0, 112.0, 115.0, 113.0, 116.0, 118.0, 120.0, 119.0, 117.0, 121.0,
        ];
        let lows = vec![90.0, 92.0, 95.0, 93.0, 96.0, 98.0, 100.0, 99.0, 97.0, 101.0];
        let closes = vec![
            105.0, 108.0, 110.0, 109.0, 112.0, 115.0, 117.0, 116.0, 114.0, 118.0,
        ];
        let stoch = stochastic(&highs, &lows, &closes, 5, 3);
        assert_eq!(stoch.k.len(), 10);
        assert_eq!(stoch.d.len(), 10);
        assert_eq!(stoch.j.len(), 10);
        // K,D,J 的值应在 0-100 范围内（非NaN值）
        for i in 6..10 {
            if !stoch.k[i].is_nan() {
                assert!(stoch.k[i] >= 0.0 && stoch.k[i] <= 100.0);
            }
            if !stoch.d[i].is_nan() {
                assert!(stoch.d[i] >= 0.0 && stoch.d[i] <= 100.0);
            }
        }
    }

    #[test]
    fn test_williams_r() {
        let highs = vec![110.0, 112.0, 115.0, 113.0, 116.0];
        let lows = vec![90.0, 92.0, 95.0, 93.0, 96.0];
        let closes = vec![105.0, 108.0, 110.0, 109.0, 112.0];
        let wr = williams_r(&highs, &lows, &closes, 5);
        assert_eq!(wr.len(), 5);
        assert!(wr[4] >= -100.0 && wr[4] <= 0.0);
    }

    #[test]
    fn test_obv() {
        let closes = vec![100.0, 102.0, 101.0, 103.0, 104.0];
        let volumes = vec![1000.0, 1500.0, 1200.0, 1800.0, 2000.0];
        let obv_vals = obv(&closes, &volumes);
        assert_eq!(obv_vals.len(), 5);
        assert!(obv_vals[0].is_nan());
        // 第二根上涨: 0 + 1500 = 1500
        assert!((obv_vals[1] - 1500.0).abs() < 1e-10);
        // 第三根下跌: 1500 - 1200 = 300
        assert!((obv_vals[2] - 300.0).abs() < 1e-10);
    }

    #[test]
    fn test_ichimoku() {
        let n = 100;
        let highs: Vec<f64> = (0..n)
            .map(|i| 100.0 + (i as f64).sin() * 10.0 + 5.0)
            .collect();
        let lows: Vec<f64> = (0..n)
            .map(|i| 100.0 + (i as f64).sin() * 10.0 - 5.0)
            .collect();
        let closes: Vec<f64> = (0..n).map(|i| 100.0 + (i as f64).sin() * 10.0).collect();
        let ichi = ichimoku(&highs, &lows, &closes);
        assert_eq!(ichi.tenkan_sen.len(), n);
        assert_eq!(ichi.kijun_sen.len(), n);
    }

    #[test]
    fn test_supertrend() {
        let n = 200;
        let highs: Vec<f64> = (0..n)
            .map(|i| 100.0 + (i as f64).sin() * 5.0 + 2.0)
            .collect();
        let lows: Vec<f64> = (0..n)
            .map(|i| 100.0 + (i as f64).sin() * 5.0 - 2.0)
            .collect();
        let closes: Vec<f64> = (0..n).map(|i| 100.0 + (i as f64).sin() * 5.0).collect();
        let st = supertrend(&highs, &lows, &closes, 10, 3.0);
        assert_eq!(st.trend.len(), n);
        assert_eq!(st.direction.len(), n);
        // 至少有一个非零的方向值
        let non_zero = st.direction.iter().filter(|d| **d != 0).count();
        assert!(non_zero > 0, "SuperTrend should have non-zero directions");
    }

    #[test]
    fn test_keltner() {
        let n = 100;
        let highs: Vec<f64> = (0..n)
            .map(|i| 100.0 + (i as f64).sin() * 5.0 + 1.0)
            .collect();
        let lows: Vec<f64> = (0..n)
            .map(|i| 100.0 + (i as f64).sin() * 5.0 - 1.0)
            .collect();
        let closes: Vec<f64> = (0..n).map(|i| 100.0 + (i as f64).sin() * 5.0).collect();
        let kc = keltner_channels(&highs, &lows, &closes, 20, 14, 2.0);
        assert_eq!(kc.upper.len(), n);
        assert_eq!(kc.lower.len(), n);
        // 上轨 >= 中线 >= 下轨（仅检查有效值）
        for i in 20..n {
            if !kc.upper[i].is_nan() && !kc.middle[i].is_nan() && !kc.lower[i].is_nan() {
                assert!(kc.upper[i] >= kc.middle[i]);
                assert!(kc.middle[i] >= kc.lower[i]);
            }
        }
    }

    #[test]
    fn test_parabolic_sar() {
        let n = 50;
        let highs: Vec<f64> = (0..n).map(|i| 100.0 + i as f64 * 0.5 + 1.0).collect();
        let lows: Vec<f64> = (0..n).map(|i| 100.0 + i as f64 * 0.5 - 1.0).collect();
        let sar = parabolic_sar(&highs, &lows, 0.02, 0.2);
        assert_eq!(sar.len(), n);
        // SAR 应在价格范围内
        for i in 1..n {
            if !sar[i].is_nan() {
                assert!(sar[i] >= lows[i] - 5.0 && sar[i] <= highs[i] + 5.0);
            }
        }
    }
}
