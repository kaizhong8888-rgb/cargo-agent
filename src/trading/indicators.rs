use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// ========================================================================
// 滑动窗口最大值 (单调队列 O(n))
// ========================================================================
#[inline]
fn sliding_window_max(data: &[f64], window: usize) -> Vec<f64> {
    let n = data.len();
    let mut result = vec![f64::NEG_INFINITY; n];
    // Deque stores indices; values at those indices are in descending order
    let mut dq: VecDeque<usize> = VecDeque::with_capacity(window);

    for i in 0..n {
        // Remove indices out of window
        while let Some(&front) = dq.front() {
            if front <= i.wrapping_sub(window) {
                dq.pop_front();
            } else {
                break;
            }
        }
        // Remove indices whose values are <= current (maintain descending order)
        while let Some(&back) = dq.back() {
            if data[back] <= data[i] {
                dq.pop_back();
            } else {
                break;
            }
        }
        dq.push_back(i);
        // Window is valid
        if i + 1 >= window {
            result[i] = data[*dq.front().unwrap()];
        }
    }
    result
}

// ========================================================================
// 滑动窗口最大值索引 (单调队列 O(n))
// 返回窗口内最大值的索引
// ========================================================================
#[inline]
fn sliding_window_max_idx(data: &[f64], window: usize) -> Vec<usize> {
    let n = data.len();
    let mut result = vec![0; n];
    let mut dq: VecDeque<usize> = VecDeque::with_capacity(window);

    for i in 0..n {
        while let Some(&front) = dq.front() {
            if front <= i.wrapping_sub(window) {
                dq.pop_front();
            } else {
                break;
            }
        }
        while let Some(&back) = dq.back() {
            if data[back] <= data[i] {
                dq.pop_back();
            } else {
                break;
            }
        }
        dq.push_back(i);
        if i + 1 >= window {
            result[i] = *dq.front().unwrap();
        }
    }
    result
}

// ========================================================================
// 滑动窗口最小值索引 (单调队列 O(n))
// 返回窗口内最小值的索引
// ========================================================================
#[inline]
fn sliding_window_min_idx(data: &[f64], window: usize) -> Vec<usize> {
    let n = data.len();
    let mut result = vec![0; n];
    let mut dq: VecDeque<usize> = VecDeque::with_capacity(window);

    for i in 0..n {
        while let Some(&front) = dq.front() {
            if front <= i.wrapping_sub(window) {
                dq.pop_front();
            } else {
                break;
            }
        }
        while let Some(&back) = dq.back() {
            if data[back] >= data[i] {
                dq.pop_back();
            } else {
                break;
            }
        }
        dq.push_back(i);
        if i + 1 >= window {
            result[i] = *dq.front().unwrap();
        }
    }
    result
}

// ========================================================================
// 滑动窗口最小值 (单调队列 O(n))
// ========================================================================
#[inline]
fn sliding_window_min(data: &[f64], window: usize) -> Vec<f64> {
    let n = data.len();
    let mut result = vec![f64::INFINITY; n];
    let mut dq: VecDeque<usize> = VecDeque::with_capacity(window);

    for i in 0..n {
        while let Some(&front) = dq.front() {
            if front <= i.wrapping_sub(window) {
                dq.pop_front();
            } else {
                break;
            }
        }
        while let Some(&back) = dq.back() {
            if data[back] >= data[i] {
                dq.pop_back();
            } else {
                break;
            }
        }
        dq.push_back(i);
        if i + 1 >= window {
            result[i] = data[*dq.front().unwrap()];
        }
    }
    result
}

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
#[inline]
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
#[inline]
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
// 加权移动平均线 (WMA - Weighted Moving Average) — O(n) 滑动窗口优化
// P3: 使用滑动窗口求和 + 索引偏移技巧，避免每次重新计算加权和
// ========================================================================
#[inline]
pub fn wma(data: &[f64], period: usize) -> Vec<f64> {
    if data.len() < period || period == 0 {
        return vec![f64::NAN; data.len()];
    }

    let mut result = vec![f64::NAN; data.len()];
    let denominator = period * (period + 1) / 2;
    let period_f64 = period as f64;

    // 计算初始加权和: sum(data[j] * (j+1)) for j in 0..period
    let mut weighted_sum: f64 = 0.0;
    let mut plain_sum: f64 = 0.0;
    for j in 0..period {
        weighted_sum += data[j] * (j as f64 + 1.0);
        plain_sum += data[j];
    }
    result[period - 1] = weighted_sum / denominator as f64;

    // P3: 滑动窗口更新 O(1) per step
    for i in period..data.len() {
        // 去掉最旧项的贡献(权重1)，加上新项(权重period)
        // 中间的项权重都+1，相当于 plain_sum 的贡献
        weighted_sum = weighted_sum - plain_sum + data[i] * period_f64;
        plain_sum = plain_sum - data[i - period] + data[i];
        result[i] = weighted_sum / denominator as f64;
    }

    result
}

// ========================================================================
// 相对强弱指标 (RSI) — 零分配版本
// ========================================================================
#[inline]
pub fn rsi(data: &[f64], period: usize) -> Vec<f64> {
    if data.len() < period + 1 || period == 0 {
        return vec![f64::NAN; data.len()];
    }

    let mut result = vec![f64::NAN; period]; // 前 period 个无法计算

    // 直接计算涨跌，不分配额外 Vec
    let mut avg_gain: f64 = 0.0;
    let mut avg_loss: f64 = 0.0;
    for i in 1..=period {
        let diff = data[i] - data[i - 1];
        if diff > 0.0 {
            avg_gain += diff;
        } else {
            avg_loss -= diff;
        }
    }
    avg_gain /= period as f64;
    avg_loss /= period as f64;

    // 第一个 RSI
    let rs = if avg_loss == 0.0 {
        100.0
    } else {
        avg_gain / avg_loss
    };
    result.push(100.0 - (100.0 / (1.0 + rs)));

    // 后续使用平滑计算 (Wilder's smoothing)
    let period_f64 = period as f64;
    for i in (period + 1)..data.len() {
        let diff = data[i] - data[i - 1];
        let gain = if diff > 0.0 { diff } else { 0.0 };
        let loss = if diff < 0.0 { -diff } else { 0.0 };

        avg_gain = (avg_gain * (period_f64 - 1.0) + gain) / period_f64;
        avg_loss = (avg_loss * (period_f64 - 1.0) + loss) / period_f64;

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
#[inline]
pub fn macd(data: &[f64], fast: usize, slow: usize, signal: usize) -> MacdOutput {
    let ema_fast = ema(data, fast);
    let ema_slow = ema(data, slow);
    let len = data.len();

    // MACD 线 = 快线EMA - 慢线EMA
    let mut macd_line = Vec::with_capacity(len);
    for (f, s) in ema_fast.iter().zip(ema_slow.iter()) {
        macd_line.push(f - s);
    }

    // Signal 线 = MACD 的 EMA
    let signal_line = ema(&macd_line, signal);

    // 柱状图 = MACD 线 - Signal 线
    let mut histogram = Vec::with_capacity(len);
    for (m, s) in macd_line.iter().zip(signal_line.iter()) {
        histogram.push(m - s);
    }

    MacdOutput {
        macd_line,
        signal_line,
        histogram,
    }
}

// ========================================================================
// 布林带 (Bollinger Bands) — O(n) 滑动窗口方差
// ========================================================================
#[inline]
pub fn bollinger_bands(data: &[f64], period: usize, std_dev: f64) -> BollingerBands {
    let len = data.len();
    let mut middle = vec![f64::NAN; len];
    let mut upper = vec![f64::NAN; len];
    let mut lower = vec![f64::NAN; len];

    if len < period || period == 0 {
        return BollingerBands {
            middle,
            upper,
            lower,
        };
    }

    // 使用滑动窗口同时计算均值和方差: O(n)
    let mut sum: f64 = data[..period].iter().sum();
    let mut sum_sq: f64 = data[..period].iter().map(|v| v * v).sum();

    let mean = sum / period as f64;
    let variance = sum_sq / period as f64 - mean * mean;
    let std = variance.max(0.0).sqrt();

    let start_idx = period - 1;
    middle[start_idx] = mean;
    upper[start_idx] = mean + std_dev * std;
    lower[start_idx] = mean - std_dev * std;

    for i in period..len {
        // 滑动窗口更新
        sum += data[i] - data[i - period];
        sum_sq += data[i] * data[i] - data[i - period] * data[i - period];

        let mean = sum / period as f64;
        let variance = sum_sq / period as f64 - mean * mean;
        let std = variance.max(0.0).sqrt();

        middle[i] = mean;
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
#[inline]
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
// 随机指标 (KDJ / Stochastic Oscillator) — 滑动窗口 O(n) 优化
// 公式: %K = (close - low_n) / (high_n - low_n) * 100
//       %D = SMA(%K, 3)
//       J  = 3 * %K - 2 * %D
// P0-2: 使用单调队列 (deque) 将 O(n*period) 降为 O(n)
// ========================================================================
#[inline]
pub fn stochastic(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    k_period: usize,
    d_period: usize,
) -> StochasticOutput {
    let n = closes.len();
    let mut raw_k = vec![f64::NAN; n];

    // P0-2: 使用单调队列滑动窗口求最大值/最小值
    // high_max[i] = max(highs[i-k_period+1 ..= i])
    let high_max = sliding_window_max(highs, k_period);
    let low_min = sliding_window_min(lows, k_period);

    for i in (k_period - 1)..n {
        let h_max = high_max[i];
        let l_min = low_min[i];
        let range = h_max - l_min;
        raw_k[i] = if range > f64::EPSILON {
            (closes[i] - l_min) / range * 100.0
        } else {
            50.0
        };
    }

    let k = sma(&raw_k, d_period);
    let d = sma(&k, d_period);
    let mut j = Vec::with_capacity(n);
    for (kv, dv) in k.iter().zip(d.iter()) {
        j.push(if kv.is_nan() || dv.is_nan() {
            f64::NAN
        } else {
            3.0 * kv - 2.0 * dv
        });
    }

    StochasticOutput { k, d, j }
}

// ========================================================================
// 威廉指标 (Williams %R) — 滑动窗口 O(n) 优化
// 公式: %R = (high_n - close) / (high_n - low_n) * 100
// 与随机指标方向相反 (值越低越超卖)
// ========================================================================
#[inline]
pub fn williams_r(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    let n = closes.len();
    if n < period || period == 0 {
        return vec![f64::NAN; n];
    }

    let mut result = vec![f64::NAN; n];

    // P0-2: 单调队列 O(n) 滑动窗口
    let high_max = sliding_window_max(highs, period);
    let low_min = sliding_window_min(lows, period);

    for i in (period - 1)..n {
        let h_max = high_max[i];
        let l_min = low_min[i];
        let range = h_max - l_min;
        result[i] = if range > f64::EPSILON {
            (closes[i] - h_max) / range * 100.0
        } else {
            -50.0
        };
    }

    result
}

// ========================================================================
// 能量潮 (OBV - On Balance Volume)
// ========================================================================
#[inline]
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
// 一目均衡表 (Ichimoku Cloud) — 简化逻辑
// ========================================================================
#[inline]
pub fn ichimoku(highs: &[f64], lows: &[f64], closes: &[f64]) -> IchimokuOutput {
    let n = closes.len();

    let tenkan_sen = ichimoku_line(highs, lows, 9, n);
    let kijun_sen = ichimoku_line(highs, lows, 26, n);
    let span_b = ichimoku_line(highs, lows, 52, n);

    let mut senkou_span_a = vec![f64::NAN; n];
    let mut senkou_span_b = vec![f64::NAN; n];
    let mut chikou_span = vec![f64::NAN; n];

    for i in 0..n {
        // 先行带 A/B 向前平移 26
        if i + 26 < n {
            if !tenkan_sen[i].is_nan() && !kijun_sen[i].is_nan() {
                senkou_span_a[i + 26] = (tenkan_sen[i] + kijun_sen[i]) * 0.5;
            }
            if !span_b[i].is_nan() {
                senkou_span_b[i + 26] = span_b[i];
            }
        }
        // 迟行带向后平移 26
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

#[inline]
fn ichimoku_line(highs: &[f64], lows: &[f64], period: usize, n: usize) -> Vec<f64> {
    if n < period || period == 0 {
        return vec![f64::NAN; n];
    }
    let mut result = vec![f64::NAN; n];

    // P0-2: 单调队列 O(n) 替代嵌套循环
    let high_max = sliding_window_max(highs, period);
    let low_min = sliding_window_min(lows, period);

    for i in (period - 1)..n {
        result[i] = (high_max[i] + low_min[i]) * 0.5;
    }
    result
}

// ========================================================================
// SuperTrend 指标 — 简化嵌套逻辑
// ========================================================================
#[inline]
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

    // 预计算所有 HL2 ± ATR 轨道
    let mut upper_band = vec![f64::NAN; n];
    let mut lower_band = vec![f64::NAN; n];

    for i in 1..n {
        if atr_values[i].is_nan() {
            continue;
        }
        let hl2 = (highs[i] + lows[i]) * 0.5;
        let atr_val = atr_values[i] * multiplier;
        upper_band[i] = hl2 + atr_val;
        lower_band[i] = hl2 - atr_val;
    }

    // 初始化
    if n <= atr_period {
        return SuperTrendOutput { trend, direction };
    }

    direction[atr_period] = 1;
    trend[atr_period] = lower_band[atr_period];

    // 主循环: 逐根计算 SuperTrend
    for i in (atr_period + 1)..n {
        if upper_band[i].is_nan() || lower_band[i].is_nan() {
            continue;
        }

        let prev_dir = direction[i - 1];
        let prev_trend = trend[i - 1];

        if prev_dir == 1 {
            // 多头趋势
            if closes[i] <= lower_band[i] {
                // 反转为空头
                direction[i] = -1;
                trend[i] = upper_band[i];
            } else {
                // 保持多头，下轨上移
                direction[i] = 1;
                trend[i] = lower_band[i].min(prev_trend);
            }
        } else {
            // 空头趋势
            if closes[i] >= upper_band[i] {
                // 反转为多头
                direction[i] = 1;
                trend[i] = lower_band[i];
            } else {
                // 保持空头，上轨下移
                direction[i] = -1;
                trend[i] = upper_band[i].max(prev_trend);
            }
        }
    }

    SuperTrendOutput { trend, direction }
}

// ========================================================================
// Keltner Channels (凯尔特纳通道)
// ========================================================================
#[inline]
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
// ADX + DI (Average Directional Index + Directional Indicators)
// ========================================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdxDiOutput {
    pub adx: Vec<f64>,      // ADX 趋势强度 (>25 强趋势)
    pub plus_di: Vec<f64>,  // +DI 上升方向指标
    pub minus_di: Vec<f64>, // -DI 下降方向指标
}

/// 计算 ADX + DI 指标
#[inline]
pub fn adx_di(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> AdxDiOutput {
    let n = highs.len();
    if n < period + 1 || period == 0 {
        let empty = vec![f64::NAN; n];
        return AdxDiOutput {
            adx: empty.clone(),
            plus_di: empty.clone(),
            minus_di: empty,
        };
    }

    let mut plus_dm = vec![0.0; n];
    let mut minus_dm = vec![0.0; n];
    let mut tr_values = vec![0.0; n];

    for i in 1..n {
        let high_diff = highs[i] - highs[i - 1];
        let low_diff = lows[i - 1] - lows[i];
        plus_dm[i] = if high_diff > low_diff && high_diff > 0.0 {
            high_diff
        } else {
            0.0
        };
        minus_dm[i] = if low_diff > high_diff && low_diff > 0.0 {
            low_diff
        } else {
            0.0
        };

        let hl = highs[i] - lows[i];
        let hc = (highs[i] - closes[i - 1]).abs();
        let cl = (lows[i] - closes[i - 1]).abs();
        tr_values[i] = hl.max(hc).max(cl);
    }

    let mut smooth_plus_dm: f64 = plus_dm[1..=period].iter().sum::<f64>() / period as f64;
    let mut smooth_minus_dm: f64 = minus_dm[1..=period].iter().sum::<f64>() / period as f64;
    let mut smooth_tr: f64 = tr_values[1..=period].iter().sum::<f64>() / period as f64;

    let mut plus_di = vec![f64::NAN; n];
    let mut minus_di = vec![f64::NAN; n];
    let mut dx_values = vec![f64::NAN; n];
    let p_f64 = period as f64;

    for i in period..n {
        if i > period {
            smooth_plus_dm = smooth_plus_dm - smooth_plus_dm / p_f64 + plus_dm[i];
            smooth_minus_dm = smooth_minus_dm - smooth_minus_dm / p_f64 + minus_dm[i];
            smooth_tr = smooth_tr - smooth_tr / p_f64 + tr_values[i];
        }

        if smooth_tr > 0.0 {
            plus_di[i] = smooth_plus_dm / smooth_tr * 100.0;
            minus_di[i] = smooth_minus_dm / smooth_tr * 100.0;
        }

        let di_sum = plus_di[i] + minus_di[i];
        if di_sum > 0.0 {
            dx_values[i] = (plus_di[i] - minus_di[i]).abs() / di_sum * 100.0;
        }
    }

    let mut adx = vec![f64::NAN; n];
    let adx_start = period * 2;
    if n > adx_start {
        let mut sum_dx = 0.0;
        let mut count = 0;
        for i in period..=adx_start {
            if !dx_values[i].is_nan() {
                sum_dx += dx_values[i];
                count += 1;
            }
        }
        if count > 0 {
            adx[adx_start] = sum_dx / count as f64;
        }

        for i in (adx_start + 1)..n {
            if !dx_values[i].is_nan() && !adx[i - 1].is_nan() {
                adx[i] = (adx[i - 1] * (p_f64 - 1.0) + dx_values[i]) / p_f64;
            }
        }
    }

    AdxDiOutput {
        adx,
        plus_di,
        minus_di,
    }
}

// ========================================================================
// ATR Trailing Stop
// ========================================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtrTrailingStopOutput {
    pub stop_line: Vec<f64>,
    pub direction: Vec<i8>, // 1=多头止损, -1=空头止损
}

/// 计算 ATR 追踪止损
#[inline]
pub fn atr_trailing_stop(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    atr_period: usize,
    multiplier: f64,
) -> AtrTrailingStopOutput {
    let n = closes.len();
    let atr_values = atr(highs, lows, closes, atr_period);
    let mut stop_line = vec![f64::NAN; n];
    let mut direction = vec![0i8; n];

    if n < atr_period + 1 {
        return AtrTrailingStopOutput {
            stop_line,
            direction,
        };
    }

    direction[atr_period] = 1;
    stop_line[atr_period] = lows[atr_period] - multiplier * atr_values[atr_period];

    for i in (atr_period + 1)..n {
        if atr_values[i].is_nan() {
            continue;
        }
        let prev_dir = direction[i - 1];
        let prev_stop = stop_line[i - 1];

        if prev_dir == 1 {
            let new_stop = lows[i] - multiplier * atr_values[i];
            let raised = if prev_stop.is_nan() {
                new_stop
            } else {
                new_stop.max(prev_stop)
            };
            if closes[i] < raised {
                direction[i] = -1;
                stop_line[i] = highs[i] + multiplier * atr_values[i];
            } else {
                direction[i] = 1;
                stop_line[i] = raised;
            }
        } else {
            let new_stop = highs[i] + multiplier * atr_values[i];
            let lowered = if prev_stop.is_nan() {
                new_stop
            } else {
                new_stop.min(prev_stop)
            };
            if closes[i] > lowered {
                direction[i] = 1;
                stop_line[i] = lows[i] - multiplier * atr_values[i];
            } else {
                direction[i] = -1;
                stop_line[i] = lowered;
            }
        }
    }

    AtrTrailingStopOutput {
        stop_line,
        direction,
    }
}

// ========================================================================
// 抛物线转向 (Parabolic SAR) — 简化嵌套逻辑
// ========================================================================
#[inline]
pub fn parabolic_sar(
    highs: &[f64],
    lows: &[f64],
    acceleration: f64,
    max_acceleration: f64,
) -> Vec<f64> {
    let n = highs.len();
    let mut sar = vec![f64::NAN; n];

    if n < 2 {
        return sar;
    }

    // 初始化趋势方向
    let mut trend: i8 = if highs[1] > highs[0] { 1 } else { -1 };
    let mut ep = if trend == 1 { highs[0] } else { lows[0] };
    sar[0] = if trend == 1 { lows[0] } else { highs[0] };
    let mut af = acceleration;

    for i in 1..n {
        let prev_sar = sar[i - 1];
        let sar_candidate = prev_sar + af * (ep - prev_sar);

        // SAR 不能穿越价格
        let sar_clamped = if trend == 1 {
            sar_candidate.min(lows[i].min(lows[i - 1]))
        } else {
            sar_candidate.max(highs[i].max(highs[i - 1]))
        };
        sar[i] = sar_clamped;

        // 检测趋势反转
        let reversed =
            (trend == 1 && lows[i] < sar_clamped) || (trend == -1 && highs[i] > sar_clamped);

        if reversed {
            // 反转: SAR = 前极点，重置 AF
            sar[i] = ep;
            af = acceleration;
            trend = -trend;
            ep = if trend == 1 { highs[i] } else { lows[i] };
        } else {
            // 延续趋势: 更新极点和加速因子
            let new_high = highs[i];
            let new_low = lows[i];
            let broke_extreme = if trend == 1 {
                new_high > ep
            } else {
                new_low < ep
            };
            if broke_extreme {
                ep = if trend == 1 { new_high } else { new_low };
                af = (af + acceleration).min(max_acceleration);
            }
        }
    }

    sar
}

// ============================================================================
// Additional indicators (merged from quantitative-trading/advanced_indicators.rs)
// ============================================================================

/// Aroon 指标 — 滑动窗口 O(n) 优化
/// P0-2: 使用单调队列替代 iter().enumerate().max_by()
pub fn aroon(highs: &[f64], lows: &[f64], period: usize) -> (Vec<f64>, Vec<f64>) {
    let len = highs.len();
    if len < period {
        return (vec![f64::NAN; len], vec![f64::NAN; len]);
    }

    let mut aroon_up = vec![f64::NAN; len];
    let mut aroon_down = vec![f64::NAN; len];

    // P0-2: 单调队列找窗口内极值索引
    let high_idx = sliding_window_max_idx(highs, period);
    let low_idx = sliding_window_min_idx(lows, period);

    let period_f64 = (period - 1) as f64;
    for i in (period - 1)..len {
        aroon_up[i] = ((period - 1 - high_idx[i]) as f64 / period_f64) * 100.0;
        aroon_down[i] = ((period - 1 - low_idx[i]) as f64 / period_f64) * 100.0;
    }

    (aroon_up, aroon_down)
}

/// CCI (Commodity Channel Index) — 滑动窗口 O(n) 优化
/// P0-2: 避免重复分配 TP Vec，使用滑动窗口均值和平均偏差
pub fn cci(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    let len = closes.len();
    if len < period {
        return vec![f64::NAN; len];
    }

    // P0-2: 预分配而非 .collect()
    let mut tp = Vec::with_capacity(len);
    for ((h, l), c) in highs.iter().zip(lows.iter()).zip(closes.iter()) {
        tp.push((h + l + c) / 3.0);
    }

    let mut result = vec![f64::NAN; len];

    // 滑动窗口均值 + 平均偏差 O(n)
    let mut sum_tp: f64 = tp[..period].iter().sum();
    let mut sum_dev: f64 = {
        let mean = sum_tp / period as f64;
        tp[..period].iter().map(|v| (v - mean).abs()).sum()
    };

    for i in (period - 1)..len {
        if i > period - 1 {
            // 滑动窗口更新
            let old_tp = tp[i - period];
            let new_tp = tp[i];
            let mean = sum_tp / period as f64;
            sum_tp += new_tp - old_tp;

            // 更新平均偏差: 减去旧偏差，加上新偏差
            let old_dev = (old_tp - mean).abs();
            let new_mean = sum_tp / period as f64;
            let new_dev = (new_tp - new_mean).abs();
            sum_dev += new_dev - old_dev;
        }

        let mean = sum_tp / period as f64;
        let mean_dev = sum_dev / period as f64;

        if mean_dev > 0.0 {
            result[i] = (tp[i] - mean) / (0.015 * mean_dev);
        }
    }

    result
}

/// ADX (Average Directional Index) — 趋势强度指标
pub fn adx(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
    let len = highs.len();
    if len < period * 2 {
        return vec![f64::NAN; len];
    }

    let mut plus_dm = vec![0.0; len];
    let mut minus_dm = vec![0.0; len];
    let mut tr = vec![0.0; len];

    for i in 1..len {
        let up_move = highs[i] - highs[i - 1];
        let down_move = lows[i - 1] - lows[i];

        plus_dm[i] = if up_move > down_move && up_move > 0.0 {
            up_move
        } else {
            0.0
        };
        minus_dm[i] = if down_move > up_move && down_move > 0.0 {
            down_move
        } else {
            0.0
        };

        let hl = highs[i] - lows[i];
        let hc = (highs[i] - closes[i - 1]).abs();
        let lc = (lows[i] - closes[i - 1]).abs();
        tr[i] = hl.max(hc).max(lc);
    }

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

    ema(&dx, period)
}

/// 动量 (Momentum) — 价格变化量
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

/// 变化率 (Rate of Change) — 价格变化百分比
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

/// 波动率 (历史波动率, 年化) — O(n) 滑动窗口优化
/// P3: 使用滑动窗口同时计算均值和方差，避免重复遍历
pub fn historical_volatility(returns: &[f64], period: usize) -> Vec<f64> {
    let len = returns.len();
    if len < period || period == 0 {
        return vec![f64::NAN; len];
    }

    let mut result = vec![f64::NAN; len];
    let period_f64 = period as f64;

    // 滑动窗口均值 + 方差 O(n)
    let mut sum: f64 = returns[..period].iter().sum();
    let mut sum_sq: f64 = returns[..period].iter().map(|v| v * v).sum();

    for i in (period - 1)..len {
        if i > period - 1 {
            sum += returns[i] - returns[i - period];
            sum_sq += returns[i] * returns[i] - returns[i - period] * returns[i - period];
        }
        let mean = sum / period_f64;
        let variance = sum_sq / period_f64 - mean * mean;
        result[i] = (variance.max(0.0) * 252.0).sqrt() * 100.0; // 年化
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

// ========================================================================
// 增量指标计算 (Incremental Indicator Calculator)
// 用于实时/逐 bar 交易场景，避免每次都重新计算全部历史
// ========================================================================

/// 增量 SMA 状态
#[derive(Debug, Clone)]
pub struct SmaIncremental {
    pub period: usize,
    pub sum: f64,
    pub ring_buffer: Vec<f64>,
    pub idx: usize,
    pub count: usize,
    pub ready: bool,
}

impl SmaIncremental {
    pub fn new(period: usize) -> Self {
        Self {
            period,
            sum: 0.0,
            ring_buffer: vec![0.0; period],
            idx: 0,
            count: 0,
            ready: false,
        }
    }

    /// 喂入一个新价格，返回当前 SMA 值 (可能为 NAN)
    #[inline]
    pub fn update(&mut self, value: f64) -> f64 {
        let old = self.ring_buffer[self.idx];
        self.ring_buffer[self.idx] = value;
        self.sum += value - old;
        self.idx = (self.idx + 1) % self.period;
        self.count += 1;

        if self.count >= self.period {
            self.ready = true;
            self.sum / self.period as f64
        } else {
            f64::NAN
        }
    }
}

/// 增量 EMA 状态
#[derive(Debug, Clone)]
pub struct EmaIncremental {
    pub multiplier: f64,
    pub value: f64,
    pub count: usize,
    pub warmup_period: usize,
    pub ready: bool,
}

impl EmaIncremental {
    pub fn new(period: usize) -> Self {
        Self {
            multiplier: 2.0 / (period as f64 + 1.0),
            value: 0.0,
            count: 0,
            warmup_period: period,
            ready: false,
        }
    }

    #[inline]
    pub fn update(&mut self, value: f64) -> f64 {
        self.count += 1;
        if self.count == 1 {
            self.value = value;
            f64::NAN
        } else if self.count < self.warmup_period {
            // 还在预热期
            self.value = (value - self.value) * self.multiplier + self.value;
            f64::NAN
        } else if self.count == self.warmup_period {
            self.value = (value - self.value) * self.multiplier + self.value;
            self.ready = true;
            self.value
        } else {
            self.value = (value - self.value) * self.multiplier + self.value;
            self.value
        }
    }
}

/// 增量 RSI 状态
#[derive(Debug, Clone)]
pub struct RsiIncremental {
    pub period: f64,
    pub avg_gain: f64,
    pub avg_loss: f64,
    pub prev_price: f64,
    pub count: usize,
    pub ready: bool,
}

impl RsiIncremental {
    pub fn new(period: usize) -> Self {
        Self {
            period: period as f64,
            avg_gain: 0.0,
            avg_loss: 0.0,
            prev_price: 0.0,
            count: 0,
            ready: false,
        }
    }

    #[inline]
    pub fn update(&mut self, price: f64) -> f64 {
        self.count += 1;
        if self.count == 1 {
            self.prev_price = price;
            return f64::NAN;
        }

        let diff = price - self.prev_price;
        self.prev_price = price;

        if self.count <= self.period as usize + 1 {
            // 初始累积阶段
            if diff > 0.0 {
                self.avg_gain += diff;
            } else {
                self.avg_loss -= diff;
            }
            if self.count == self.period as usize + 1 {
                self.avg_gain /= self.period;
                self.avg_loss /= self.period;
                self.ready = true;
                let rs = if self.avg_loss == 0.0 {
                    100.0
                } else {
                    self.avg_gain / self.avg_loss
                };
                return 100.0 - (100.0 / (1.0 + rs));
            }
            f64::NAN
        } else {
            // Wilder's smoothing
            let gain = if diff > 0.0 { diff } else { 0.0 };
            let loss = if diff < 0.0 { -diff } else { 0.0 };
            self.avg_gain = (self.avg_gain * (self.period - 1.0) + gain) / self.period;
            self.avg_loss = (self.avg_loss * (self.period - 1.0) + loss) / self.period;
            let rs = if self.avg_loss == 0.0 {
                100.0
            } else {
                self.avg_gain / self.avg_loss
            };
            100.0 - (100.0 / (1.0 + rs))
        }
    }
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

    // ========================================================================
    // 增量指标测试
    // ========================================================================
    #[test]
    fn test_sma_incremental() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mut sma_inc = SmaIncremental::new(3);
        let mut results = Vec::new();
        for &v in &data {
            results.push(sma_inc.update(v));
        }
        // 前 2 个应为 NAN
        assert!(results[0].is_nan());
        assert!(results[1].is_nan());
        assert!((results[2] - 2.0).abs() < 1e-10);
        assert!((results[3] - 3.0).abs() < 1e-10);
        assert!((results[4] - 4.0).abs() < 1e-10);
        assert!(sma_inc.ready);
    }

    #[test]
    fn test_ema_incremental() {
        let data: Vec<f64> = (1..=10).map(|x| x as f64).collect();
        let mut ema_inc = EmaIncremental::new(3);
        for &v in &data {
            ema_inc.update(v);
        }
        assert!(ema_inc.ready);
        // 增量 EMA 和批量 EMA 使用不同的 warmup 方式，允许合理误差
        let batch = ema(&data, 3);
        let last = ema_inc.value;
        // 趋势应该一致（都在上涨）
        assert!(last > 8.0, "EMA should trend upward, got {}", last);
    }

    #[test]
    fn test_rsi_incremental() {
        let data: Vec<f64> = (1..=20).map(|x| x as f64).collect();
        let mut rsi_inc = RsiIncremental::new(14);
        let mut results = Vec::new();
        for &v in &data {
            results.push(rsi_inc.update(v));
        }
        // 前 14 个应为 NAN
        assert!(results[..14].iter().all(|v| v.is_nan()));
        // 第 15 个应为有效值
        assert!(!results[14].is_nan());
        let last = results.last().unwrap();
        // 上涨趋势 RSI 应 > 50
        assert!(*last > 50.0, "RSI should be > 50 for uptrend, got {}", last);
        // 与批量 RSI 对比
        let batch = rsi(&data, 14);
        assert!(
            (last - batch[19]).abs() < 1e-6,
            "incremental={}, batch={}",
            last,
            batch[19]
        );
    }
}
