/// 高级技术指标
/// 包括：SuperTrend、Keltner Channel、Ichimoku、Stochastic、Williams %R、OBV、ADX、Parabolic SAR、Hurst

use serde::{Deserialize, Serialize};

/// SuperTrend 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuperTrendOutput {
    pub trend: Vec<f64>,
    pub direction: Vec<i32>, // 1=up, -1=down
}

/// SuperTrend 指标
pub fn supertrend(highs: &[f64], lows: &[f64], closes: &[f64], period: usize, multiplier: f64) -> SuperTrendOutput {
    let len = highs.len();
    if len < period + 1 {
        return SuperTrendOutput {
            trend: vec![f64::NAN; len],
            direction: vec![0; len],
        };
    }

    let atr_values = crate::indicators::atr(highs, lows, closes, period);

    let mut trend = vec![f64::NAN; len];
    let mut direction = vec![0; len];

    let mut prev_upper = f64::NAN;
    let mut prev_lower = f64::NAN;
    let mut prev_direction = 0;

    for i in period..len {
        if atr_values[i].is_nan() {
            continue;
        }

        let hl2 = (highs[i] + lows[i]) / 2.0;
        let mut upper = hl2 + multiplier * atr_values[i];
        let mut lower = hl2 - multiplier * atr_values[i];

        if !prev_upper.is_nan() {
            upper = upper.min(prev_upper);
        }
        if !prev_lower.is_nan() {
            lower = lower.max(prev_lower);
        }

        let mut dir = prev_direction;
        if prev_direction == -1 && closes[i] > prev_upper {
            dir = 1;
        } else if prev_direction == 1 && closes[i] < prev_lower {
            dir = -1;
        } else if prev_direction == 0 {
            if closes[i] > upper {
                dir = 1;
            } else if closes[i] < lower {
                dir = -1;
            }
        }

        trend[i] = if dir == 1 { lower } else { upper };
        direction[i] = dir;

        prev_upper = upper;
        prev_lower = lower;
        prev_direction = dir;
    }

    SuperTrendOutput { trend, direction }
}

/// Keltner Channel 结果
#[derive(Debug, Clone)]
pub struct KeltnerChannel {
    pub middle: Vec<f64>,
    pub upper: Vec<f64>,
    pub lower: Vec<f64>,
}

/// Keltner Channel
pub fn keltner_channel(highs: &[f64], lows: &[f64], closes: &[f64], ema_period: usize, atr_period: usize, atr_multiplier: f64) -> KeltnerChannel {
    let len = closes.len();
    let middle = crate::indicators::ema(closes, ema_period);
    let atr_values = crate::indicators::atr(highs, lows, closes, atr_period);

    let mut upper = vec![f64::NAN; len];
    let mut lower = vec![f64::NAN; len];

    for i in 0..len {
        if !middle[i].is_nan() && !atr_values[i].is_nan() {
            upper[i] = middle[i] + atr_values[i] * atr_multiplier;
            lower[i] = middle[i] - atr_values[i] * atr_multiplier;
        }
    }

    KeltnerChannel { middle, upper, lower }
}

/// Stochastic Oscillator
pub fn stochastic(highs: &[f64], lows: &[f64], closes: &[f64], k_period: usize, d_period: usize) -> (Vec<f64>, Vec<f64>) {
    let len = closes.len();
    if len < k_period {
        return (vec![f64::NAN; len], vec![f64::NAN; len]);
    }

    let mut k_values = vec![f64::NAN; len];
    let mut d_values = vec![f64::NAN; len];

    for i in (k_period - 1)..len {
        let high_max = highs[i + 1 - k_period..=i].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let low_min = lows[i + 1 - k_period..=i].iter().cloned().fold(f64::INFINITY, f64::min);

        let range = high_max - low_min;
        if range > 0.0 {
            k_values[i] = (closes[i] - low_min) / range * 100.0;
        } else {
            k_values[i] = 50.0;
        }
    }

    // D = SMA of K
    let valid_k: Vec<f64> = k_values.iter().map(|&v| if v.is_nan() { 50.0 } else { v }).collect();
    let sma_d = crate::indicators::sma(&valid_k, d_period);

    for i in 0..len {
        d_values[i] = sma_d[i];
    }

    (k_values, d_values)
}

/// Williams %R
pub fn williams_r(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    let len = closes.len();
    if len < period {
        return vec![f64::NAN; len];
    }

    let mut result = vec![f64::NAN; len];

    for i in (period - 1)..len {
        let high_max = highs[i + 1 - period..=i].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let low_min = lows[i + 1 - period..=i].iter().cloned().fold(f64::INFINITY, f64::min);

        let range = high_max - low_min;
        if range > 0.0 {
            result[i] = (high_max - closes[i]) / range * -100.0;
        } else {
            result[i] = -50.0;
        }
    }

    result
}

/// OBV (On Balance Volume)
pub fn obv(closes: &[f64], volumes: &[f64]) -> Vec<f64> {
    let len = closes.len();
    if len == 0 {
        return vec![];
    }

    let mut result = vec![0.0; len];
    result[0] = volumes[0];

    for i in 1..len {
        if closes[i] > closes[i - 1] {
            result[i] = result[i - 1] + volumes[i];
        } else if closes[i] < closes[i - 1] {
            result[i] = result[i - 1] - volumes[i];
        } else {
            result[i] = result[i - 1];
        }
    }

    result
}

/// Parabolic SAR
pub fn parabolic_sar(highs: &[f64], lows: &[f64], step: f64, max_step: f64) -> Vec<f64> {
    let len = highs.len();
    if len < 2 {
        return vec![f64::NAN; len];
    }

    let mut sar = vec![f64::NAN; len];
    let mut is_long = true;
    let mut ep = highs[0]; // 极值点
    let mut af = step; // 加速因子

    sar[0] = lows[0];

    for i in 1..len {
        let prev_sar = sar[i - 1];
        sar[i] = prev_sar + af * (ep - prev_sar);

        if is_long {
            if lows[i] < sar[i] {
                // 反转
                is_long = false;
                sar[i] = ep;
                ep = lows[i];
                af = step;
            } else {
                if highs[i] > ep {
                    ep = highs[i];
                    af = (af + step).min(max_step);
                }
                // 确保 SAR 不高于最近两个低点
                if i >= 2 {
                    let min_low = lows[i - 1].min(lows[i - 2]);
                    if sar[i] > min_low {
                        sar[i] = min_low;
                    }
                }
            }
        } else {
            if highs[i] > sar[i] {
                // 反转
                is_long = true;
                sar[i] = ep;
                ep = highs[i];
                af = step;
            } else {
                if lows[i] < ep {
                    ep = lows[i];
                    af = (af + step).min(max_step);
                }
                // 确保 SAR 不低于最近两个高点
                if i >= 2 {
                    let max_high = highs[i - 1].max(highs[i - 2]);
                    if sar[i] < max_high {
                        sar[i] = max_high;
                    }
                }
            }
        }
    }

    sar
}

/// Ichimoku Cloud 结果
#[derive(Debug, Clone)]
pub struct IchimokuOutput {
    pub tenkan_sen: Vec<f64>, // 转换线 (9)
    pub kijun_sen: Vec<f64>,  // 基准线 (26)
    pub senkou_span_a: Vec<f64>, // 先行带 A
    pub senkou_span_b: Vec<f64>, // 先行带 B
    pub chikou_span: Vec<f64>,   // 延迟线
}

/// Donchian 中值 (用于 Ichimoku)
fn donchian_mid(data: &[f64], period: usize) -> Vec<f64> {
    let len = data.len();
    if len < period {
        return vec![f64::NAN; len];
    }
    let mut result = vec![f64::NAN; len];
    for i in (period - 1)..len {
        let high = data[i + 1 - period..=i].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let low = data[i + 1 - period..=i].iter().cloned().fold(f64::INFINITY, f64::min);
        result[i] = (high + low) / 2.0;
    }
    result
}

/// Ichimoku Cloud
pub fn ichimoku(highs: &[f64], lows: &[f64], closes: &[f64]) -> IchimokuOutput {
    let len = closes.len();

    let tenkan = donchian_mid(highs, 9);
    let kijun = donchian_mid(highs, 26);

    // Senkou Span A = (Tenkan + Kijun) / 2, 前移 26 期
    let mut senkou_a = vec![f64::NAN; len];
    let mut senkou_b = vec![f64::NAN; len];
    for i in 0..len {
        if !tenkan[i].is_nan() && !kijun[i].is_nan() {
            let future = i + 26;
            if future < len {
                senkou_a[future] = (tenkan[i] + kijun[i]) / 2.0;
            }
        }
    }

    // Senkou Span B = 52 期 Donchian 中值, 前移 26 期
    let span_b = donchian_mid(highs, 52);
    for i in 0..len {
        let future = i + 26;
        if future < len {
            senkou_b[future] = span_b[i];
        }
    }

    // Chikou Span = 收盘价后移 26 期
    let mut chikou = vec![f64::NAN; len];
    for i in 26..len {
        chikou[i - 26] = closes[i];
    }

    IchimokuOutput {
        tenkan_sen: tenkan,
        kijun_sen: kijun,
        senkou_span_a: senkou_a,
        senkou_span_b: senkou_b,
        chikou_span: chikou,
    }
}

/// ADX (Average Directional Index) - 简化版
pub fn adx(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    let len = highs.len();
    if len < period * 2 {
        return vec![f64::NAN; len];
    }

    // +DM, -DM, TR
    let mut plus_dm = vec![0.0; len];
    let mut minus_dm = vec![0.0; len];
    let mut tr = vec![0.0; len];

    for i in 1..len {
        let up_move = highs[i] - highs[i - 1];
        let down_move = lows[i - 1] - lows[i];

        plus_dm[i] = if up_move > down_move && up_move > 0.0 { up_move } else { 0.0 };
        minus_dm[i] = if down_move > up_move && down_move > 0.0 { down_move } else { 0.0 };

        let hl = highs[i] - lows[i];
        let hc = (highs[i] - closes[i - 1]).abs();
        let lc = (lows[i] - closes[i - 1]).abs();
        tr[i] = hl.max(hc).max(lc);
    }

    // 平滑
    let mut smooth_plus = vec![0.0; len];
    let mut smooth_minus = vec![0.0; len];
    let mut smooth_tr = vec![0.0; len];

    for i in 1..=period {
        smooth_plus[period] += plus_dm[i];
        smooth_minus[period] += minus_dm[i];
        smooth_tr[period] += tr[i];
    }

    for i in (period + 1)..len {
        smooth_plus[i] = smooth_plus[i - 1] - smooth_plus[i - 1] / period as f64 + plus_dm[i];
        smooth_minus[i] = smooth_minus[i - 1] - smooth_minus[i - 1] / period as f64 + minus_dm[i];
        smooth_tr[i] = smooth_tr[i - 1] - smooth_tr[i - 1] / period as f64 + tr[i];
    }

    // DI+, DI-
    let mut di_plus = vec![f64::NAN; len];
    let mut di_minus = vec![f64::NAN; len];
    let mut dx = vec![f64::NAN; len];

    for i in period..len {
        if smooth_tr[i] > 0.0 {
            di_plus[i] = smooth_plus[i] / smooth_tr[i] * 100.0;
            di_minus[i] = smooth_minus[i] / smooth_tr[i] * 100.0;
            let sum = di_plus[i] + di_minus[i];
            if sum > 0.0 {
                dx[i] = (di_plus[i] - di_minus[i]).abs() / sum * 100.0;
            }
        }
    }

    // ADX = EMA of DX
    crate::indicators::ema(&dx, period)
}

/// CCI (Commodity Channel Index)
pub fn cci(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    let len = closes.len();
    if len < period {
        return vec![f64::NAN; len];
    }

    // 典型价格
    let tp: Vec<f64> = highs.iter()
        .zip(lows.iter())
        .zip(closes.iter())
        .map(|((h, l), c)| (h + l + c) / 3.0)
        .collect();

    let mut result = vec![f64::NAN; len];

    for i in (period - 1)..len {
        let segment = &tp[i + 1 - period..=i];
        let mean: f64 = segment.iter().sum::<f64>() / period as f64;
        let mean_dev: f64 = segment.iter().map(|v| (v - mean).abs()).sum::<f64>() / period as f64;

        if mean_dev > 0.0 {
            result[i] = (tp[i] - mean) / (0.015 * mean_dev);
        }
    }

    result
}

/// Aroon 指标
pub fn aroon(highs: &[f64], lows: &[f64], period: usize) -> (Vec<f64>, Vec<f64>) {
    let len = highs.len();
    if len < period {
        return (vec![f64::NAN; len], vec![f64::NAN; len]);
    }

    let mut aroon_up = vec![f64::NAN; len];
    let mut aroon_down = vec![f64::NAN; len];

    for i in (period - 1)..len {
        let window = &highs[i + 1 - period..=i];
        let high_idx = window.iter().enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        let window = &lows[i + 1 - period..=i];
        let low_idx = window.iter().enumerate()
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        aroon_up[i] = ((period - 1 - high_idx) as f64 / (period - 1) as f64) * 100.0;
        aroon_down[i] = ((period - 1 - low_idx) as f64 / (period - 1) as f64) * 100.0;
    }

    (aroon_up, aroon_down)
}

/// 动量 (Momentum)
pub fn momentum(data: &[f64], period: usize) -> Vec<f64> {
    let len = data.len();
    if len < period {
        return vec![f64::NAN; len];
    }

    let mut result = vec![f64::NAN; len];
    for i in period..len {
        result[i] = data[i] - data[i - period];
    }
    result
}

/// 变化率 (Rate of Change)
pub fn roc(data: &[f64], period: usize) -> Vec<f64> {
    let len = data.len();
    if len < period {
        return vec![f64::NAN; len];
    }

    let mut result = vec![f64::NAN; len];
    for i in period..len {
        if data[i - period] != 0.0 {
            result[i] = (data[i] - data[i - period]) / data[i - period] * 100.0;
        }
    }
    result
}

/// 波动率 (历史波动率, 年化)
pub fn historical_volatility(returns: &[f64], period: usize) -> Vec<f64> {
    let len = returns.len();
    if len < period {
        return vec![f64::NAN; len];
    }

    let mut result = vec![f64::NAN; len];
    for i in (period - 1)..len {
        let segment = &returns[i + 1 - period..=i];
        let mean: f64 = segment.iter().sum::<f64>() / segment.len() as f64;
        let variance: f64 = segment.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / segment.len() as f64;
        result[i] = (variance * 252.0).sqrt() * 100.0; // 年化
    }
    result
}

/// 对数收益率
pub fn log_returns(data: &[f64]) -> Vec<f64> {
    let len = data.len();
    if len < 2 {
        return vec![f64::NAN; len];
    }

    let mut result = vec![f64::NAN; len];
    for i in 1..len {
        if data[i - 1] > 0.0 && data[i] > 0.0 {
            result[i] = (data[i] / data[i - 1]).ln();
        }
    }
    result
}
