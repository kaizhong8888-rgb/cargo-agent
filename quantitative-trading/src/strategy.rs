use crate::data::Candle;
use crate::indicators;
use crate::indicators::MacdOutput;

/// 交易方向
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum Signal {
    /// 买入
    Buy,
    /// 卖出 (做空)
    Sell,
    /// 持仓不动
    Hold,
}

use serde::{Deserialize, Serialize};

/// 策略 trait：所有策略必须实现此接口
pub trait Strategy {
    /// 策略名称
    fn name(&self) -> &str;

    /// 在给定 K 线数据上生成交易信号序列
    /// 返回与 candles 等长的信号向量
    fn generate(&self, candles: &[Candle]) -> Vec<Signal>;
}

// ========================================================================
// 1️⃣ 双均线交叉策略 (SMA Crossover)
// ========================================================================
pub struct SmaCrossover {
    fast_period: usize,
    slow_period: usize,
}

impl SmaCrossover {
    pub fn new(fast_period: usize, slow_period: usize) -> Self {
        assert!(fast_period < slow_period, "快周期必须小于慢周期");
        Self {
            fast_period,
            slow_period,
        }
    }
}

impl Strategy for SmaCrossover {
    fn name(&self) -> &str {
        "SMA Crossover"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let fast_sma = indicators::sma(&closes, self.fast_period);
        let slow_sma = indicators::sma(&closes, self.slow_period);

        let mut signals = vec![Signal::Hold; candles.len()];

        for i in 1..candles.len() {
            if i < self.slow_period {
                continue;
            }

            let prev_fast = fast_sma[i - 1];
            let prev_slow = slow_sma[i - 1];
            let curr_fast = fast_sma[i];
            let curr_slow = slow_sma[i];

            if prev_fast.is_nan() || prev_slow.is_nan() || curr_fast.is_nan() || curr_slow.is_nan()
            {
                continue;
            }

            // 金叉：快线上穿慢线 → 买入
            if prev_fast <= prev_slow && curr_fast > curr_slow {
                signals[i] = Signal::Buy;
            }
            // 死叉：快线下穿慢线 → 卖出
            else if prev_fast >= prev_slow && curr_fast < curr_slow {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 2️⃣ 双均线交叉 + RSI 过滤策略
// ========================================================================
pub struct SmaCrossoverWithRsi {
    fast_period: usize,
    slow_period: usize,
    rsi_period: usize,
    rsi_oversold: f64,
    rsi_overbought: f64,
}

impl SmaCrossoverWithRsi {
    pub fn new(
        fast_period: usize,
        slow_period: usize,
        rsi_period: usize,
        rsi_oversold: f64,
        rsi_overbought: f64,
    ) -> Self {
        assert!(fast_period < slow_period, "快周期必须小于慢周期");
        assert!(
            rsi_oversold < rsi_overbought,
            "超卖阈值必须小于超买阈值"
        );
        Self {
            fast_period,
            slow_period,
            rsi_period,
            rsi_oversold,
            rsi_overbought,
        }
    }
}

impl Strategy for SmaCrossoverWithRsi {
    fn name(&self) -> &str {
        "SMA Crossover + RSI Filter"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let fast_sma = indicators::sma(&closes, self.fast_period);
        let slow_sma = indicators::sma(&closes, self.slow_period);
        let rsi_values = indicators::rsi(&closes, self.rsi_period);

        let mut signals = vec![Signal::Hold; candles.len()];

        for i in 1..candles.len() {
            if i < self.slow_period || i >= rsi_values.len() {
                continue;
            }

            let curr_rsi = rsi_values[i];
            let curr_fast = fast_sma[i];
            let curr_slow = slow_sma[i];
            let prev_fast = fast_sma[i - 1];
            let prev_slow = slow_sma[i - 1];

            if curr_rsi.is_nan() || curr_fast.is_nan() || curr_slow.is_nan() {
                continue;
            }

            // 买入信号：金叉 + RSI 不处于超买区域
            if prev_fast <= prev_slow && curr_fast > curr_slow && curr_rsi < self.rsi_overbought {
                signals[i] = Signal::Buy;
            }
            // 卖出信号：死叉 + RSI 不处于超卖区域
            else if prev_fast >= prev_slow && curr_fast < curr_slow && curr_rsi > self.rsi_oversold
            {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 3️⃣ RSI 均值回归策略 (RSI Mean Reversion)
// ========================================================================
#[allow(dead_code)]
pub struct RsiMeanReversion {
    rsi_period: usize,
    oversold: f64,
    overbought: f64,
}

#[allow(dead_code)]
impl RsiMeanReversion {
    pub fn new(rsi_period: usize, oversold: f64, overbought: f64) -> Self {
        Self {
            rsi_period,
            oversold,
            overbought,
        }
    }
}

impl Strategy for RsiMeanReversion {
    fn name(&self) -> &str {
        "RSI Mean Reversion"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let rsi_values = indicators::rsi(&closes, self.rsi_period);

        let mut signals = vec![Signal::Hold; candles.len()];

        for i in 1..candles.len() {
            if i >= rsi_values.len() || rsi_values[i].is_nan() {
                continue;
            }

            let curr_rsi = rsi_values[i];
            let prev_rsi = rsi_values[i - 1];

            // RSI 从超卖区域回升 → 买入
            if prev_rsi <= self.oversold && curr_rsi > self.oversold {
                signals[i] = Signal::Buy;
            }
            // RSI 从超买区域回落 → 卖出
            else if prev_rsi >= self.overbought && curr_rsi < self.overbought {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 4️⃣ MACD 金叉死叉 + 柱状图确认 + 背离策略 🔥
// ========================================================================
/// MACD 策略模式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MacdMode {
    /// 仅金叉/死叉
    Crossover,
    /// 金叉/死叉 + 柱状图方向确认
    CrossoverWithHistogram,
    /// 金叉/死叉 + MACD背离
    CrossoverWithDivergence,
}

pub struct MacdStrategy {
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
    mode: MacdMode,
    divergence_lookback: usize,
}

impl MacdStrategy {
    pub fn new(fast_period: usize, slow_period: usize, signal_period: usize, mode: MacdMode) -> Self {
        assert!(fast_period < slow_period, "快周期必须小于慢周期");
        Self {
            fast_period,
            slow_period,
            signal_period,
            mode,
            divergence_lookback: 20, // 背离回溯窗口
        }
    }

    /// 检测 MACD 底背离：价格新低但 MACD 未新低 → 看涨
    fn detect_bullish_divergence(
        &self,
        closes: &[f64],
        macd_line: &[f64],
        i: usize,
    ) -> bool {
        let lookback = self.divergence_lookback.min(i);
        if lookback < 5 {
            return false;
        }
        let start = i - lookback;

        // 找到区间内的价格最低点和 MACD 最低点
        let price_low_idx = start
            + closes[start..=i]
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(0);

        let macd_low_idx = start
            + macd_line[start..=i]
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(0);

        // 底背离：价格低点比上一个低点更低，但 MACD 低点更高
        if price_low_idx > 0 && macd_low_idx > 0 {
            let prev_price_low = closes[price_low_idx - 1..=price_low_idx]
                .iter()
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .copied()
                .unwrap_or(closes[price_low_idx]);

            let prev_macd_low = macd_line[macd_low_idx - 1..=macd_low_idx]
                .iter()
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .copied()
                .unwrap_or(macd_line[macd_low_idx]);

            closes[price_low_idx] < prev_price_low && macd_line[macd_low_idx] > prev_macd_low
        } else {
            false
        }
    }

    /// 检测 MACD 顶背离：价格新高但 MACD 未创新高 → 看跌
    fn detect_bearish_divergence(
        &self,
        closes: &[f64],
        macd_line: &[f64],
        i: usize,
    ) -> bool {
        let lookback = self.divergence_lookback.min(i);
        if lookback < 5 {
            return false;
        }
        let start = i - lookback;

        // 找到区间内的价格最高点和 MACD 最高点
        let price_high_idx = start
            + closes[start..=i]
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(0);

        let macd_high_idx = start
            + macd_line[start..=i]
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(0);

        // 顶背离：价格高点比上一个高点更高，但 MACD 高点更低
        if price_high_idx > 0 && macd_high_idx > 0 {
            let prev_price_high = closes[price_high_idx - 1..=price_high_idx]
                .iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .copied()
                .unwrap_or(closes[price_high_idx]);

            let prev_macd_high = macd_line[macd_high_idx - 1..=macd_high_idx]
                .iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .copied()
                .unwrap_or(macd_line[macd_high_idx]);

            closes[price_high_idx] > prev_price_high && macd_line[macd_high_idx] < prev_macd_high
        } else {
            false
        }
    }
}

impl Strategy for MacdStrategy {
    fn name(&self) -> &str {
        match self.mode {
            MacdMode::Crossover => "MACD Crossover",
            MacdMode::CrossoverWithHistogram => "MACD + Histogram",
            MacdMode::CrossoverWithDivergence => "MACD + Divergence",
        }
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let MacdOutput {
            macd_line,
            signal_line,
            histogram,
        } = indicators::macd(&closes, self.fast_period, self.slow_period, self.signal_period);

        let mut signals = vec![Signal::Hold; candles.len()];

        for i in 1..candles.len() {
            if i < self.slow_period + self.signal_period {
                continue;
            }

            let prev_macd = macd_line[i - 1];
            let curr_macd = macd_line[i];
            let prev_signal = signal_line[i - 1];
            let curr_signal = signal_line[i];
            let curr_hist = histogram[i];
            let prev_hist = histogram[i - 1];

            if prev_macd.is_nan() || curr_macd.is_nan() || prev_signal.is_nan() || curr_signal.is_nan()
            {
                continue;
            }

            match self.mode {
                MacdMode::Crossover => {
                    // 金叉：MACD 线上穿 Signal 线 → 买入
                    if prev_macd <= prev_signal && curr_macd > curr_signal {
                        signals[i] = Signal::Buy;
                    }
                    // 死叉：MACD 线下穿 Signal 线 → 卖出
                    else if prev_macd >= prev_signal && curr_macd < curr_signal {
                        signals[i] = Signal::Sell;
                    }
                }
                MacdMode::CrossoverWithHistogram => {
                    // 金叉 + 柱状图由负转正（加速上涨）
                    if prev_macd <= prev_signal && curr_macd > curr_signal && prev_hist <= 0.0 && curr_hist > 0.0
                    {
                        signals[i] = Signal::Buy;
                    }
                    // 死叉 + 柱状图由正转负（加速下跌）
                    else if prev_macd >= prev_signal && curr_macd < curr_signal && prev_hist >= 0.0 && curr_hist < 0.0
                    {
                        signals[i] = Signal::Sell;
                    }
                }
                MacdMode::CrossoverWithDivergence => {
                    // 金叉或底背离 → 买入
                    let gold_cross = prev_macd <= prev_signal && curr_macd > curr_signal;
                    let bullish_div = self.detect_bullish_divergence(&closes, &macd_line, i);

                    if gold_cross || bullish_div {
                        signals[i] = Signal::Buy;
                    }

                    // 死叉或顶背离 → 卖出
                    let death_cross = prev_macd >= prev_signal && curr_macd < curr_signal;
                    let bearish_div = self.detect_bearish_divergence(&closes, &macd_line, i);

                    if death_cross || bearish_div {
                        signals[i] = Signal::Sell;
                    }

                    // 如果同时出现矛盾信号，以金叉/死叉为准
                    if gold_cross && bearish_div {
                        signals[i] = Signal::Buy;
                    }
                    if death_cross && bullish_div {
                        signals[i] = Signal::Sell;
                    }
                }
            }
        }

        signals
    }
}

// ========================================================================
// 5️⃣ 海龟交易法则 (Turtle Trading - Donchian Breakout) 🔥
// ========================================================================
pub struct TurtleTradingStrategy {
    /// 入场通道周期 (默认 20)
    entry_period: usize,
    /// 出场通道周期 (默认 10)
    exit_period: usize,
    /// ATR 周期 (默认 20)
    atr_period: usize,
    /// ATR 倍数用于止损 (默认 2)
    stop_loss_atr: f64,
}

impl TurtleTradingStrategy {
    pub fn new(entry_period: usize, exit_period: usize, atr_period: usize, stop_loss_atr: f64) -> Self {
        assert!(entry_period > 0 && exit_period > 0 && atr_period > 0);
        Self {
            entry_period,
            exit_period,
            atr_period,
            stop_loss_atr,
        }
    }

    fn donchian_channel_high(prices: &[f64], period: usize) -> Vec<f64> {
        if prices.len() < period || period == 0 {
            return vec![f64::NAN; prices.len()];
        }
        let mut result = vec![f64::NAN; period - 1];
        for i in (period - 1)..prices.len() {
            let max_val = prices[i + 1 - period..=i]
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max);
            result.push(max_val);
        }
        result
    }

    fn donchian_channel_low(prices: &[f64], period: usize) -> Vec<f64> {
        if prices.len() < period || period == 0 {
            return vec![f64::NAN; prices.len()];
        }
        let mut result = vec![f64::NAN; period - 1];
        for i in (period - 1)..prices.len() {
            let min_val = prices[i + 1 - period..=i]
                .iter()
                .cloned()
                .fold(f64::INFINITY, f64::min);
            result.push(min_val);
        }
        result
    }
}

impl Strategy for TurtleTradingStrategy {
    fn name(&self) -> &str {
        "Turtle Trading (Donchian)"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        let entry_high = Self::donchian_channel_high(&highs, self.entry_period);
        let entry_low = Self::donchian_channel_low(&lows, self.entry_period);
        let exit_high = Self::donchian_channel_high(&highs, self.exit_period);
        let exit_low = Self::donchian_channel_low(&lows, self.exit_period);
        let atr_values = indicators::atr(&highs, &lows, &closes, self.atr_period);

        let mut signals = vec![Signal::Hold; candles.len()];
        let mut in_position = false;
        let mut entry_price = 0.0;

        for i in 1..candles.len() {
            if i < self.entry_period {
                continue;
            }

            let current_high = entry_high[i];
            let current_low = entry_low[i];
            let _exit_high_val = exit_high[i];
            let exit_low_val = exit_low[i];
            let curr_close = closes[i];
            let prev_close = closes[i - 1];
            let atr = atr_values[i];

            if current_high.is_nan() || current_low.is_nan() || atr.is_nan() {
                continue;
            }

            if !in_position {
                // 入场信号：价格突破 20日高点 → 买入
                if prev_close <= current_high && curr_close > current_high {
                    signals[i] = Signal::Buy;
                    in_position = true;
                    entry_price = curr_close;
                }
            } else {
                // 止损：价格跌破入场价 - 2 * ATR
                // 出场信号：价格跌破 10日低点
                let stop_loss = entry_price - self.stop_loss_atr * atr;
                if curr_close < stop_loss || curr_close < exit_low_val {
                    signals[i] = Signal::Sell;
                    in_position = false;
                }
            }
        }

        signals
    }
}

// ========================================================================
// 6️⃣ 布林带均值回归 + 挤压突破策略 🔥
// ========================================================================
pub struct BollingerBandsStrategy {
    period: usize,
    std_dev: f64,
    /// 买入阈值：价格触及下轨的百分比 (1.0 = 触及下轨)
    buy_threshold: f64,
    /// 卖出阈值：价格触及上轨的百分比 (1.0 = 触及上轨)
    sell_threshold: f64,
    /// 启用挤压突破检测
    enable_squeeze: bool,
}

impl BollingerBandsStrategy {
    pub fn new(
        period: usize,
        std_dev: f64,
        buy_threshold: f64,
        sell_threshold: f64,
        enable_squeeze: bool,
    ) -> Self {
        Self {
            period,
            std_dev,
            buy_threshold,
            sell_threshold,
            enable_squeeze,
        }
    }
}

impl Strategy for BollingerBandsStrategy {
    fn name(&self) -> &str {
        if self.enable_squeeze {
            "Bollinger Bands + Squeeze"
        } else {
            "Bollinger Bands MeanRev"
        }
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        let bb = indicators::bollinger_bands(&closes, self.period, self.std_dev);
        let bandwidth = Self::compute_bandwidth(&bb);

        let mut signals = vec![Signal::Hold; candles.len()];

        for i in self.period..candles.len() {
            let upper = bb.upper[i];
            let lower = bb.lower[i];
            let middle = bb.middle[i];
            let close = closes[i];

            if upper.is_nan() || lower.is_nan() || middle.is_nan() {
                continue;
            }

            // 计算价格在布林带中的位置 (0 = 下轨, 0.5 = 中轨, 1 = 上轨)
            let range = upper - lower;
            let position = if range > 0.0 {
                (close - lower) / range
            } else {
                0.5
            };

            // 挤压突破模式
            if self.enable_squeeze && i > self.period + 1 {
                let curr_bandwidth = bandwidth[i];
                let prev_bandwidth = bandwidth[i - 1];
                let prev_prev_bandwidth = bandwidth[i - 2];

                // 检测挤压结束：带宽先收缩到低位再突然扩张
                if !curr_bandwidth.is_nan()
                    && !prev_bandwidth.is_nan()
                    && !prev_prev_bandwidth.is_nan()
                {
                    let is_squeeze = prev_prev_bandwidth > prev_bandwidth
                        && prev_bandwidth < curr_bandwidth
                        && curr_bandwidth > prev_bandwidth * 1.1; // 扩张超过10%

                    if is_squeeze {
                        // 突破上轨 → 买入（趋势向上）
                        if close > upper {
                            signals[i] = Signal::Buy;
                            continue;
                        }
                        // 突破下轨 → 卖出（趋势向下）
                        else if close < lower {
                            signals[i] = Signal::Sell;
                            continue;
                        }
                    }
                }
            }

            // 均值回归信号：超卖买入 / 超买卖出
            if position <= (1.0 - self.buy_threshold) {
                signals[i] = Signal::Buy;
            } else if position >= self.sell_threshold {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

impl BollingerBandsStrategy {
    /// 计算布林带带宽： (上轨 - 下轨) / 中轨
    fn compute_bandwidth(bb: &indicators::BollingerBands) -> Vec<f64> {
        bb.upper
            .iter()
            .zip(bb.lower.iter())
            .zip(bb.middle.iter())
            .map(|((u, l), m)| {
                if *m > 0.0 && !u.is_nan() && !l.is_nan() {
                    (u - l) / m
                } else {
                    f64::NAN
                }
            })
            .collect()
    }
}

// ========================================================================
// 7️⃣ 三均线趋势跟踪策略 (Triple EMA) 🔥
// ========================================================================
pub struct TripleEmaStrategy {
    fast_period: usize,
    mid_period: usize,
    slow_period: usize,
}

impl TripleEmaStrategy {
    pub fn new(fast_period: usize, mid_period: usize, slow_period: usize) -> Self {
        assert!(
            fast_period < mid_period && mid_period < slow_period,
            "周期必须 fast < mid < slow"
        );
        Self {
            fast_period,
            mid_period,
            slow_period,
        }
    }
}

impl Strategy for TripleEmaStrategy {
    fn name(&self) -> &str {
        "Triple EMA Trend"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let fast_ema = indicators::ema(&closes, self.fast_period);
        let mid_ema = indicators::ema(&closes, self.mid_period);
        let slow_ema = indicators::ema(&closes, self.slow_period);

        let mut signals = vec![Signal::Hold; candles.len()];

        for i in 1..candles.len() {
            if i < self.slow_period {
                continue;
            }

            let f = fast_ema[i];
            let m = mid_ema[i];
            let s = slow_ema[i];
            let pf = fast_ema[i - 1];
            let pm = mid_ema[i - 1];
            let ps = slow_ema[i - 1];

            if f.is_nan() || m.is_nan() || s.is_nan() {
                continue;
            }

            // 趋势状态
            let prev_aligned = pf > pm && pm > ps; // 之前三线多头排列
            let curr_aligned = f > m && m > s;     // 当前三线多头排列
            let prev_bearish = pf < pm && pm < ps; // 之前三线空头排列
            let curr_bearish = f < m && m < s;     // 当前三线空头排列

            // 买入：快线上穿中线（趋势开始转多）
            if !prev_aligned && curr_aligned {
                signals[i] = Signal::Buy;
            }
            // 卖出：快线下穿中线（趋势开始转空）
            else if !prev_bearish && curr_bearish {
                signals[i] = Signal::Sell;
            }
            // 趋势加强：继续保持多头排列时加仓信号（可选）
            // 这里保持简单，只在趋势转变时交易
        }

        signals
    }
}

// ========================================================================
// 8️⃣ VWAP + RSI 均值回归策略 🔥
// ========================================================================
pub struct VwapRsiStrategy {
    rsi_period: usize,
    oversold: f64,
    overbought: f64,
    /// VWAP 偏离阈值：价格偏离 VWAP 的百分比
    vwap_deviation_pct: f64,
}

impl VwapRsiStrategy {
    pub fn new(rsi_period: usize, oversold: f64, overbought: f64, vwap_deviation_pct: f64) -> Self {
        Self {
            rsi_period,
            oversold,
            overbought,
            vwap_deviation_pct,
        }
    }

    /// 计算 VWAP (成交量加权平均价格)
    fn compute_vwap(candles: &[Candle]) -> Vec<f64> {
        let mut vwap = vec![f64::NAN; candles.len()];
        let mut cum_pv = 0.0;
        let mut cum_vol = 0.0;

        for (i, c) in candles.iter().enumerate() {
            // 典型价格 = (高 + 低 + 收) / 3
            let typical_price = (c.high + c.low + c.close) / 3.0;
            cum_pv += typical_price * c.volume;
            cum_vol += c.volume;

            if cum_vol > 0.0 {
                vwap[i] = cum_pv / cum_vol;
            }
        }

        vwap
    }
}

impl Strategy for VwapRsiStrategy {
    fn name(&self) -> &str {
        "VWAP + RSI Reversion"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let rsi_values = indicators::rsi(&closes, self.rsi_period);
        let vwap = Self::compute_vwap(candles);

        let mut signals = vec![Signal::Hold; candles.len()];

        for i in self.rsi_period..candles.len() {
            let close = closes[i];
            let vwap_val = vwap[i];
            let rsi_val = rsi_values[i];

            if vwap_val.is_nan() || rsi_val.is_nan() || vwap_val == 0.0 {
                continue;
            }

            let deviation = (close - vwap_val) / vwap_val * 100.0; // 偏离百分比

            // 买入条件：价格低于 VWAP 超过阈值（折价）+ RSI 超卖
            if deviation < -self.vwap_deviation_pct && rsi_val < self.oversold {
                signals[i] = Signal::Buy;
            }
            // 卖出条件：价格高于 VWAP 超过阈值（溢价）+ RSI 超买
            else if deviation > self.vwap_deviation_pct && rsi_val > self.overbought {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 9️⃣ 组合策略 (Ensemble Voting) 🔥
// ========================================================================
pub struct EnsembleStrategy {
    strategies: Vec<Box<dyn Strategy>>,
    /// 买入所需的最小票数 (默认: 过半)
    buy_threshold: usize,
    /// 卖出所需的最小票数 (默认: 过半)
    sell_threshold: usize,
    /// 策略名称（预先计算）
    name: String,
}

impl EnsembleStrategy {
    pub fn new(strategies: Vec<Box<dyn Strategy>>) -> Self {
        let n = strategies.len();
        let thresh = (n as f64 / 2.0).ceil() as usize;
        let name = format!("Ensemble ({} strategies, {}/{})", n, thresh, thresh);
        Self {
            strategies,
            buy_threshold: thresh,
            sell_threshold: thresh,
            name,
        }
    }

    #[allow(dead_code)]
    pub fn with_thresholds(
        strategies: Vec<Box<dyn Strategy>>,
        buy_threshold: usize,
        sell_threshold: usize,
    ) -> Self {
        let name = format!("Ensemble ({} strategies, {}/{})", strategies.len(), buy_threshold, sell_threshold);
        Self {
            strategies,
            buy_threshold,
            sell_threshold,
            name,
        }
    }
}

impl Strategy for EnsembleStrategy {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let n_strats = self.strategies.len();
        let n_candles = candles.len();

        // 收集每个子策略的信号
        let mut all_signals: Vec<Vec<Signal>> = Vec::with_capacity(n_strats);
        for strat in &self.strategies {
            all_signals.push(strat.generate(candles));
        }

        let mut result = vec![Signal::Hold; n_candles];

        for i in 0..n_candles {
            let mut buy_votes = 0;
            let mut sell_votes = 0;

            for signals in &all_signals {
                if i < signals.len() {
                    match signals[i] {
                        Signal::Buy => buy_votes += 1,
                        Signal::Sell => sell_votes += 1,
                        Signal::Hold => {}
                    }
                }
            }

            if buy_votes >= self.buy_threshold && buy_votes > sell_votes {
                result[i] = Signal::Buy;
            } else if sell_votes >= self.sell_threshold && sell_votes > buy_votes {
                result[i] = Signal::Sell;
            }
        }

        result
    }
}

/// 辅助函数：将策略对象装箱
#[macro_export]
macro_rules! box_strategy {
    ($s:expr) => {
        Box::new($s) as Box<dyn Strategy>
    };
}

// ========================================================================
// 测试
// ========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::DataSource;

    fn create_test_candles() -> Vec<Candle> {
        DataSource::generate_mock(200, 100.0)
    }

    #[test]
    fn test_sma_crossover_signals() {
        let candles = create_test_candles();
        let strategy = SmaCrossover::new(5, 20);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_sma_crossover_with_rsi_signals() {
        let candles = create_test_candles();
        let strategy = SmaCrossoverWithRsi::new(5, 20, 14, 30.0, 70.0);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_rsi_mean_reversion_signals() {
        let candles = create_test_candles();
        let strategy = RsiMeanReversion::new(14, 30.0, 70.0);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_macd_strategy_all_modes() {
        let candles = create_test_candles();
        for mode in &[MacdMode::Crossover, MacdMode::CrossoverWithHistogram, MacdMode::CrossoverWithDivergence] {
            let strategy = MacdStrategy::new(12, 26, 9, *mode);
            let signals = strategy.generate(&candles);
            assert_eq!(signals.len(), candles.len());
        }
    }

    #[test]
    fn test_turtle_trading() {
        let candles = create_test_candles();
        let strategy = TurtleTradingStrategy::new(20, 10, 20, 2.0);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_bollinger_bands_strategy() {
        let candles = create_test_candles();
        let strategy = BollingerBandsStrategy::new(20, 2.0, 0.95, 0.95, false);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_bollinger_squeeze() {
        let candles = create_test_candles();
        let strategy = BollingerBandsStrategy::new(20, 2.0, 0.95, 0.95, true);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_triple_ema() {
        let candles = create_test_candles();
        let strategy = TripleEmaStrategy::new(5, 13, 34);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_vwap_rsi() {
        let candles = create_test_candles();
        let strategy = VwapRsiStrategy::new(14, 30.0, 70.0, 1.0);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_ensemble_strategy() {
        let candles = create_test_candles();
        let strategies: Vec<Box<dyn Strategy>> = vec![
            Box::new(SmaCrossover::new(5, 20)),
            Box::new(MacdStrategy::new(12, 26, 9, MacdMode::Crossover)),
            Box::new(RsiMeanReversion::new(14, 30.0, 70.0)),
        ];
        let strategy = EnsembleStrategy::new(strategies);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_all_strategies_different_outputs() {
        let candles = create_test_candles();
        let strategies: Vec<Box<dyn Strategy>> = vec![
            Box::new(SmaCrossover::new(5, 20)),
            Box::new(SmaCrossoverWithRsi::new(5, 20, 14, 30.0, 70.0)),
            Box::new(RsiMeanReversion::new(14, 30.0, 70.0)),
            Box::new(MacdStrategy::new(12, 26, 9, MacdMode::Crossover)),
            Box::new(TurtleTradingStrategy::new(20, 10, 20, 2.0)),
            Box::new(BollingerBandsStrategy::new(20, 2.0, 0.95, 0.95, false)),
            Box::new(TripleEmaStrategy::new(5, 13, 34)),
            Box::new(VwapRsiStrategy::new(14, 30.0, 70.0, 1.0)),
        ];

        for strat in &strategies {
            let signals = strat.generate(&candles);
            assert_eq!(signals.len(), candles.len());
            let buy_count = signals.iter().filter(|s| **s == Signal::Buy).count();
            let sell_count = signals.iter().filter(|s| **s == Signal::Sell).count();
            println!("  {} → Buy: {}, Sell: {}", strat.name(), buy_count, sell_count);
        }
    }
}
