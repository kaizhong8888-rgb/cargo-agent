use crate::trading::data::Candle;
use crate::trading::indicators;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};

/// 交易方向
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Signal {
    /// 买入
    Buy,
    /// 卖出 (做空)
    Sell,
    /// 持仓不动
    Hold,
}

/// 策略 trait：所有策略必须实现此接口
pub trait Strategy {
    /// 策略名称
    fn name(&self) -> &str;

    /// 在给定 K 线数据上生成交易信号序列
    /// 返回与 candles 等长的信号向量
    fn generate(&self, candles: &[Candle]) -> Vec<Signal>;

    /// P1: 使用预分配的 OHLCV 数组生成信号（避免重复分配）
    /// 默认实现委托给 generate()，子类可覆盖以优化性能
    fn generate_with_data(
        &self,
        closes: &[f64],
        highs: &[f64],
        lows: &[f64],
        volumes: &[f64],
        _candles: &[Candle],
    ) -> Vec<Signal> {
        let _ = (highs, lows, volumes); // 默认不使用
        self.generate_from_closes(closes)
    }

    /// 便捷方法：仅使用收盘价生成信号
    fn generate_from_closes(&self, closes: &[f64]) -> Vec<Signal> {
        // 默认实现需要完整 Candle 数据，返回空
        vec![Signal::Hold; closes.len()]
    }
}

/// P1: 预分配的 OHLCV 数据容器（避免策略重复分配）
#[derive(Clone)]
pub struct OhlcvData {
    pub closes: Vec<f64>,
    pub highs: Vec<f64>,
    pub lows: Vec<f64>,
    pub volumes: Vec<f64>,
}

impl OhlcvData {
    /// 从 Candle 数组预分配所有 OHLCV 数组
    pub fn from_candles(candles: &[Candle]) -> Self {
        let n = candles.len();
        let mut closes = Vec::with_capacity(n);
        let mut highs = Vec::with_capacity(n);
        let mut lows = Vec::with_capacity(n);
        let mut volumes = Vec::with_capacity(n);

        for c in candles {
            closes.push(c.close);
            highs.push(c.high);
            lows.push(c.low);
            volumes.push(c.volume);
        }

        Self {
            closes,
            highs,
            lows,
            volumes,
        }
    }
}

// ========================================================================
// 1️⃣ 双均线交叉策略 (SMA Crossover)
// ========================================================================
#[derive(Debug)]
pub struct SmaCrossover {
    fast_period: usize,
    slow_period: usize,
}

impl SmaCrossover {
    pub fn new(fast_period: usize, slow_period: usize) -> Self {
        Self::try_new(fast_period, slow_period).expect("快周期必须小于慢周期")
    }

    pub fn try_new(fast_period: usize, slow_period: usize) -> anyhow::Result<Self> {
        if fast_period >= slow_period {
            return Err(anyhow!(
                "快周期必须小于慢周期 (fast={}, slow={})",
                fast_period,
                slow_period
            ));
        }
        Ok(Self {
            fast_period,
            slow_period,
        })
    }
}

impl Strategy for SmaCrossover {
    fn name(&self) -> &str {
        "SMA Crossover"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        self.generate_from_closes(&closes)
    }

    fn generate_from_closes(&self, closes: &[f64]) -> Vec<Signal> {
        let fast_sma = indicators::sma(closes, self.fast_period);
        let slow_sma = indicators::sma(closes, self.slow_period);
        let n = closes.len();
        let mut signals = vec![Signal::Hold; n];

        for i in 1..n {
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
#[derive(Debug)]
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
        Self::try_new(
            fast_period,
            slow_period,
            rsi_period,
            rsi_oversold,
            rsi_overbought,
        )
        .expect("invalid SmaCrossoverWithRsi params")
    }

    pub fn try_new(
        fast_period: usize,
        slow_period: usize,
        rsi_period: usize,
        rsi_oversold: f64,
        rsi_overbought: f64,
    ) -> anyhow::Result<Self> {
        if fast_period >= slow_period {
            return Err(anyhow!(
                "快周期必须小于慢周期 (fast={}, slow={})",
                fast_period,
                slow_period
            ));
        }
        if rsi_oversold >= rsi_overbought {
            return Err(anyhow!(
                "超卖阈值必须小于超买阈值 (oversold={}, overbought={})",
                rsi_oversold,
                rsi_overbought
            ));
        }
        Ok(Self {
            fast_period,
            slow_period,
            rsi_period,
            rsi_oversold,
            rsi_overbought,
        })
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
            else if prev_fast >= prev_slow
                && curr_fast < curr_slow
                && curr_rsi > self.rsi_oversold
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
pub struct RsiMeanReversion {
    rsi_period: usize,
    oversold: f64,
    overbought: f64,
}

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
// 4️⃣ MACD 金叉死叉 + 柱状图确认 + 背离策略
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

#[derive(Debug)]
pub struct MacdStrategy {
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
    mode: MacdMode,
    divergence_lookback: usize,
}

impl MacdStrategy {
    pub fn new(
        fast_period: usize,
        slow_period: usize,
        signal_period: usize,
        mode: MacdMode,
    ) -> Self {
        Self::try_new(fast_period, slow_period, signal_period, mode)
            .expect("invalid MacdStrategy params")
    }

    pub fn try_new(
        fast_period: usize,
        slow_period: usize,
        signal_period: usize,
        mode: MacdMode,
    ) -> anyhow::Result<Self> {
        if fast_period >= slow_period {
            return Err(anyhow!(
                "MACD 快周期必须小于慢周期 (fast={}, slow={})",
                fast_period,
                slow_period
            ));
        }
        Ok(Self {
            fast_period,
            slow_period,
            signal_period,
            mode,
            divergence_lookback: 20,
        })
    }

    fn detect_bullish_divergence(&self, closes: &[f64], macd_line: &[f64], i: usize) -> bool {
        let lookback = self.divergence_lookback.min(i);
        if lookback < 5 {
            return false;
        }
        let start = i - lookback;

        // 手动扫描价格最低点和 MACD 最低点，避免 iterator 分配
        let mut price_low_idx = start;
        let mut macd_low_idx = start;
        let mut price_min = closes[start];
        let mut macd_min = macd_line[start];

        for j in (start + 1)..=i {
            if closes[j] < price_min {
                price_min = closes[j];
                price_low_idx = j;
            }
            if macd_line[j] < macd_min {
                macd_min = macd_line[j];
                macd_low_idx = j;
            }
        }

        if price_low_idx > 0 && macd_low_idx > 0 {
            let prev_price_low = closes[price_low_idx - 1].min(closes[price_low_idx]);
            let prev_macd_low = macd_line[macd_low_idx - 1].min(macd_line[macd_low_idx]);

            closes[price_low_idx] < prev_price_low && macd_line[macd_low_idx] > prev_macd_low
        } else {
            false
        }
    }

    fn detect_bearish_divergence(&self, closes: &[f64], macd_line: &[f64], i: usize) -> bool {
        let lookback = self.divergence_lookback.min(i);
        if lookback < 5 {
            return false;
        }
        let start = i - lookback;

        // 手动扫描价格最高点和 MACD 最高点
        let mut price_high_idx = start;
        let mut macd_high_idx = start;
        let mut price_max = closes[start];
        let mut macd_max = macd_line[start];

        for j in (start + 1)..=i {
            if closes[j] > price_max {
                price_max = closes[j];
                price_high_idx = j;
            }
            if macd_line[j] > macd_max {
                macd_max = macd_line[j];
                macd_high_idx = j;
            }
        }

        if price_high_idx > 0 && macd_high_idx > 0 {
            let prev_price_high = closes[price_high_idx - 1].max(closes[price_high_idx]);
            let prev_macd_high = macd_line[macd_high_idx - 1].max(macd_line[macd_high_idx]);

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
        let macd_out = indicators::macd(
            &closes,
            self.fast_period,
            self.slow_period,
            self.signal_period,
        );
        let macd_line = &macd_out.macd_line;
        let signal_line = &macd_out.signal_line;
        let histogram = &macd_out.histogram;

        let mut signals = vec![Signal::Hold; candles.len()];
        let warmup = self.slow_period + self.signal_period;

        for i in 1..candles.len() {
            if i < warmup {
                continue;
            }

            let prev_macd = macd_line[i - 1];
            let curr_macd = macd_line[i];
            let prev_signal = signal_line[i - 1];
            let curr_signal = signal_line[i];
            let curr_hist = histogram[i];
            let prev_hist = histogram[i - 1];

            if prev_macd.is_nan()
                || curr_macd.is_nan()
                || prev_signal.is_nan()
                || curr_signal.is_nan()
            {
                continue;
            }

            let gold_cross = prev_macd <= prev_signal && curr_macd > curr_signal;
            let death_cross = prev_macd >= prev_signal && curr_macd < curr_signal;

            match self.mode {
                MacdMode::Crossover => {
                    if gold_cross {
                        signals[i] = Signal::Buy;
                    } else if death_cross {
                        signals[i] = Signal::Sell;
                    }
                }
                MacdMode::CrossoverWithHistogram => {
                    let hist_buy = prev_hist <= 0.0 && curr_hist > 0.0;
                    let hist_sell = prev_hist >= 0.0 && curr_hist < 0.0;
                    if gold_cross && hist_buy {
                        signals[i] = Signal::Buy;
                    } else if death_cross && hist_sell {
                        signals[i] = Signal::Sell;
                    }
                }
                MacdMode::CrossoverWithDivergence => {
                    let bullish_div = self.detect_bullish_divergence(&closes, macd_line, i);
                    let bearish_div = self.detect_bearish_divergence(&closes, macd_line, i);

                    if gold_cross || bullish_div {
                        signals[i] = Signal::Buy;
                    } else if death_cross || bearish_div {
                        signals[i] = Signal::Sell;
                    }
                }
            }
        }

        signals
    }
}

// ========================================================================
// 5️⃣ 海龟交易法则 (Turtle Trading - Donchian Breakout)
// ========================================================================
#[derive(Debug)]
pub struct TurtleTradingStrategy {
    entry_period: usize,
    exit_period: usize,
    atr_period: usize,
    stop_loss_atr: f64,
}

impl TurtleTradingStrategy {
    pub fn new(
        entry_period: usize,
        exit_period: usize,
        atr_period: usize,
        stop_loss_atr: f64,
    ) -> Self {
        Self::try_new(entry_period, exit_period, atr_period, stop_loss_atr)
            .expect("invalid TurtleTradingStrategy params")
    }

    pub fn try_new(
        entry_period: usize,
        exit_period: usize,
        atr_period: usize,
        stop_loss_atr: f64,
    ) -> anyhow::Result<Self> {
        if entry_period == 0 || exit_period == 0 || atr_period == 0 {
            return Err(anyhow!(
                "海龟交易参数不能为零 (entry={}, exit={}, atr={})",
                entry_period,
                exit_period,
                atr_period
            ));
        }
        Ok(Self {
            entry_period,
            exit_period,
            atr_period,
            stop_loss_atr,
        })
    }

    /// P1: 使用单调队列（Deque）实现 O(n) 滑动窗口最大值
    fn donchian_channel_high(prices: &[f64], period: usize) -> Vec<f64> {
        let len = prices.len();
        if len < period || period == 0 {
            return vec![f64::NAN; len];
        }
        let mut result = Vec::with_capacity(len);
        // 单调递减双端队列：存储索引
        let mut dq: Vec<usize> = Vec::with_capacity(period);

        for i in 0..len {
            // 移除超出窗口的元素
            if !dq.is_empty() && dq[0] + period <= i {
                dq.remove(0);
            }
            // 维护单调递减：移除尾部所有小于等于当前值的索引
            while !dq.is_empty() && prices[*dq.last().unwrap()] <= prices[i] {
                dq.pop();
            }
            dq.push(i);
            // 窗口头部即为最大值
            if i >= period - 1 {
                result.push(prices[dq[0]]);
            } else {
                result.push(f64::NAN);
            }
        }
        result
    }

    /// P1: 使用单调队列（Deque）实现 O(n) 滑动窗口最小值
    fn donchian_channel_low(prices: &[f64], period: usize) -> Vec<f64> {
        let len = prices.len();
        if len < period || period == 0 {
            return vec![f64::NAN; len];
        }
        let mut result = Vec::with_capacity(len);
        // 单调递增双端队列：存储索引
        let mut dq: Vec<usize> = Vec::with_capacity(period);

        for i in 0..len {
            // 移除超出窗口的元素
            if !dq.is_empty() && dq[0] + period <= i {
                dq.remove(0);
            }
            // 维护单调递增：移除尾部所有大于等于当前值的索引
            while !dq.is_empty() && prices[*dq.last().unwrap()] >= prices[i] {
                dq.pop();
            }
            dq.push(i);
            // 窗口头部即为最小值
            if i >= period - 1 {
                result.push(prices[dq[0]]);
            } else {
                result.push(f64::NAN);
            }
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
            let exit_low_val = exit_low[i];
            let curr_close = closes[i];
            let prev_close = closes[i - 1];
            let atr = atr_values[i];

            if current_high.is_nan() || current_low.is_nan() || atr.is_nan() {
                continue;
            }

            if !in_position {
                if prev_close <= current_high && curr_close > current_high {
                    signals[i] = Signal::Buy;
                    in_position = true;
                    entry_price = curr_close;
                }
            } else {
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
// 6️⃣ 布林带均值回归 + 挤压突破策略
// ========================================================================
pub struct BollingerBandsStrategy {
    period: usize,
    std_dev: f64,
    buy_threshold: f64,
    sell_threshold: f64,
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

    #[inline]
    fn compute_bandwidth(bb: &indicators::BollingerBands) -> Vec<f64> {
        let n = bb.middle.len();
        let mut bw = Vec::with_capacity(n);
        for ((u, l), m) in bb.upper.iter().zip(bb.lower.iter()).zip(bb.middle.iter()) {
            bw.push(if *m > 0.0 && !u.is_nan() && !l.is_nan() {
                (u - l) / m
            } else {
                f64::NAN
            });
        }
        bw
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

            let range = upper - lower;
            let position = if range > 0.0 {
                (close - lower) / range
            } else {
                0.5
            };

            if self.enable_squeeze && i > self.period + 1 {
                let curr_bandwidth = bandwidth[i];
                let prev_bandwidth = bandwidth[i - 1];
                let prev_prev_bandwidth = bandwidth[i - 2];

                if !curr_bandwidth.is_nan()
                    && !prev_bandwidth.is_nan()
                    && !prev_prev_bandwidth.is_nan()
                {
                    let is_squeeze = prev_prev_bandwidth > prev_bandwidth
                        && prev_bandwidth < curr_bandwidth
                        && curr_bandwidth > prev_bandwidth * 1.1;

                    if is_squeeze {
                        if close > upper {
                            signals[i] = Signal::Buy;
                            continue;
                        } else if close < lower {
                            signals[i] = Signal::Sell;
                            continue;
                        }
                    }
                }
            }

            if position <= (1.0 - self.buy_threshold) {
                signals[i] = Signal::Buy;
            } else if position >= self.sell_threshold {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 7️⃣ 三均线趋势跟踪策略 (Triple EMA)
// ========================================================================
#[derive(Debug)]
pub struct TripleEmaStrategy {
    fast_period: usize,
    mid_period: usize,
    slow_period: usize,
}

impl TripleEmaStrategy {
    pub fn new(fast_period: usize, mid_period: usize, slow_period: usize) -> Self {
        Self::try_new(fast_period, mid_period, slow_period)
            .expect("invalid TripleEmaStrategy params")
    }

    pub fn try_new(
        fast_period: usize,
        mid_period: usize,
        slow_period: usize,
    ) -> anyhow::Result<Self> {
        if fast_period >= mid_period || mid_period >= slow_period {
            return Err(anyhow!(
                "EMA 周期必须 fast < mid < slow (fast={}, mid={}, slow={})",
                fast_period,
                mid_period,
                slow_period
            ));
        }
        Ok(Self {
            fast_period,
            mid_period,
            slow_period,
        })
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

            let prev_aligned = pf > pm && pm > ps;
            let curr_aligned = f > m && m > s;
            let prev_bearish = pf < pm && pm < ps;
            let curr_bearish = f < m && m < s;

            if !prev_aligned && curr_aligned {
                signals[i] = Signal::Buy;
            } else if !prev_bearish && curr_bearish {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 8️⃣ VWAP + RSI 均值回归策略
// ========================================================================
pub struct VwapRsiStrategy {
    rsi_period: usize,
    oversold: f64,
    overbought: f64,
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

    #[inline]
    fn compute_vwap(candles: &[Candle]) -> Vec<f64> {
        let n = candles.len();
        let mut vwap = Vec::with_capacity(n);
        let mut cum_pv = 0.0;
        let mut cum_vol = 0.0;

        for c in candles.iter() {
            let typical_price = (c.high + c.low + c.close) / 3.0;
            cum_pv += typical_price * c.volume;
            cum_vol += c.volume;

            vwap.push(if cum_vol > 0.0 {
                cum_pv / cum_vol
            } else {
                f64::NAN
            });
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

            let deviation = (close - vwap_val) / vwap_val * 100.0;

            if deviation < -self.vwap_deviation_pct && rsi_val < self.oversold {
                signals[i] = Signal::Buy;
            } else if deviation > self.vwap_deviation_pct && rsi_val > self.overbought {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 9️⃣ SuperTrend 趋势跟踪策略
// ========================================================================
pub struct SuperTrendStrategy {
    atr_period: usize,
    multiplier: f64,
}

impl SuperTrendStrategy {
    pub fn new(atr_period: usize, multiplier: f64) -> Self {
        Self {
            atr_period,
            multiplier,
        }
    }
}

impl Strategy for SuperTrendStrategy {
    fn name(&self) -> &str {
        "SuperTrend"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        let st = indicators::supertrend(&highs, &lows, &closes, self.atr_period, self.multiplier);
        let direction = st.direction;

        let mut signals = vec![Signal::Hold; candles.len()];

        for i in 1..candles.len() {
            if i >= direction.len() || direction[i] == 0 || direction[i - 1] == 0 {
                continue;
            }

            let prev_dir = direction[i - 1];
            let curr_dir = direction[i];

            // 方向从空头变为多头 → 买入
            if prev_dir == -1 && curr_dir == 1 {
                signals[i] = Signal::Buy;
            }
            // 方向从多头变为空头 → 卖出
            else if prev_dir == 1 && curr_dir == -1 {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 🔟 Keltner Channels 突破策略
// ========================================================================
pub struct KeltnerChannelsStrategy {
    ema_period: usize,
    atr_period: usize,
    multiplier: f64,
    mode: KeltnerMode,
}

/// Keltner 策略模式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeltnerMode {
    /// 价格突破上轨做多，跌破下轨做空
    Breakout,
    /// 价格回到通道内时反向操作（均值回归）
    Reversion,
}

impl KeltnerChannelsStrategy {
    pub fn new(ema_period: usize, atr_period: usize, multiplier: f64, mode: KeltnerMode) -> Self {
        Self {
            ema_period,
            atr_period,
            multiplier,
            mode,
        }
    }
}

impl Strategy for KeltnerChannelsStrategy {
    fn name(&self) -> &str {
        match self.mode {
            KeltnerMode::Breakout => "Keltner Breakout",
            KeltnerMode::Reversion => "Keltner Reversion",
        }
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        let kc = indicators::keltner_channels(
            &highs,
            &lows,
            &closes,
            self.ema_period,
            self.atr_period,
            self.multiplier,
        );
        let upper = kc.upper;
        let lower = kc.lower;

        let mut signals = vec![Signal::Hold; candles.len()];

        match self.mode {
            KeltnerMode::Breakout => {
                for i in 1..candles.len() {
                    if upper[i].is_nan() || lower[i].is_nan() {
                        continue;
                    }
                    let prev_close = closes[i - 1];
                    let curr_close = closes[i];

                    // 突破上轨 → 买入（上涨突破）
                    if prev_close <= upper[i] && curr_close > upper[i] {
                        signals[i] = Signal::Buy;
                    }
                    // 跌破下轨 → 卖出（下跌突破）
                    else if prev_close >= lower[i] && curr_close < lower[i] {
                        signals[i] = Signal::Sell;
                    }
                }
            }
            KeltnerMode::Reversion => {
                for i in 1..candles.len() {
                    if upper[i].is_nan() || lower[i].is_nan() {
                        continue;
                    }
                    let curr_close = closes[i];
                    let prev_close = closes[i - 1];
                    let prev_upper = upper[i - 1];
                    let prev_lower = lower[i - 1];

                    // 从通道外回到通道内 → 反向交易
                    if prev_close >= prev_upper && curr_close < upper[i] {
                        // 从上轨上方回落 → 卖出
                        signals[i] = Signal::Sell;
                    } else if prev_close <= prev_lower && curr_close > lower[i] {
                        // 从下轨下方回升 → 买入
                        signals[i] = Signal::Buy;
                    }
                }
            }
        }

        signals
    }
}

// ========================================================================
// 1️⃣1️⃣ Parabolic SAR 抛物线转向策略
// ========================================================================
pub struct ParabolicSarStrategy {
    acceleration: f64,
    max_acceleration: f64,
}

impl ParabolicSarStrategy {
    pub fn new(acceleration: f64, max_acceleration: f64) -> Self {
        Self {
            acceleration,
            max_acceleration,
        }
    }
}

impl Strategy for ParabolicSarStrategy {
    fn name(&self) -> &str {
        "Parabolic SAR"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        let sar =
            indicators::parabolic_sar(&highs, &lows, self.acceleration, self.max_acceleration);

        let mut signals = vec![Signal::Hold; candles.len()];

        for i in 1..candles.len() {
            if sar[i].is_nan() || sar[i - 1].is_nan() {
                continue;
            }

            let prev_close = closes[i - 1];
            let prev_sar = sar[i - 1];
            let curr_close = closes[i];
            let curr_sar = sar[i];

            // SAR 从价格上方转到下方 → 买入（趋势转多）
            if prev_sar > prev_close && curr_sar < curr_close {
                signals[i] = Signal::Buy;
            }
            // SAR 从价格下方转到上方 → 卖出（趋势转空）
            else if prev_sar < prev_close && curr_sar > curr_close {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 1️⃣2️⃣ 组合策略 (Ensemble Voting)
// ========================================================================
pub struct EnsembleStrategy {
    strategies: Vec<Box<dyn Strategy>>,
    buy_threshold: usize,
    sell_threshold: usize,
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
        let name = format!(
            "Ensemble ({} strategies, {}/{})",
            strategies.len(),
            buy_threshold,
            sell_threshold
        );
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
        let n_candles = candles.len();
        let mut buy_votes = vec![0usize; n_candles];
        let mut sell_votes = vec![0usize; n_candles];

        for strat in &self.strategies {
            let signals = strat.generate(candles);
            let limit = n_candles.min(signals.len());
            for i in 0..limit {
                match signals[i] {
                    Signal::Buy => buy_votes[i] += 1,
                    Signal::Sell => sell_votes[i] += 1,
                    Signal::Hold => {}
                }
            }
        }

        let mut result = vec![Signal::Hold; n_candles];
        for i in 0..n_candles {
            if buy_votes[i] >= self.buy_threshold && buy_votes[i] > sell_votes[i] {
                result[i] = Signal::Buy;
            } else if sell_votes[i] >= self.sell_threshold && sell_votes[i] > buy_votes[i] {
                result[i] = Signal::Sell;
            }
        }
        result
    }
}

// ========================================================================
// 1️⃣3️⃣ 一目均衡表 (Ichimoku Cloud) 趋势跟踪策略
// ========================================================================
pub struct IchimokuStrategy {
    /// TK交叉确认窗口 (默认1根)
    confirm_period: usize,
}

impl IchimokuStrategy {
    pub fn new(confirm_period: usize) -> Self {
        Self { confirm_period }
    }
}

impl Strategy for IchimokuStrategy {
    fn name(&self) -> &str {
        "Ichimoku Cloud"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        let ichi = indicators::ichimoku(&highs, &lows, &closes);
        let n = closes.len();
        let mut signals = vec![Signal::Hold; n];

        // 需要足够的数据 (52周期 + 26前移)
        let warmup = 78;
        if n < warmup + self.confirm_period {
            return signals;
        }

        for i in warmup..(n - self.confirm_period) {
            let tenkan = ichi.tenkan_sen[i];
            let kijun = ichi.kijun_sen[i];
            let span_a = ichi.senkou_span_a[i];
            let span_b = ichi.senkou_span_b[i];
            let price = closes[i];
            let chikou = ichi.chikou_span[i];

            if tenkan.is_nan() || kijun.is_nan() || span_a.is_nan() || span_b.is_nan() {
                continue;
            }

            // 多头条件: TK金叉 + 价格在云层上方 + 迟行带确认
            let tk_bullish = tenkan > kijun;
            let above_cloud = price > span_a.max(span_b);
            let chikou_confirmed = chikou.is_nan()
                || chikou
                    > closes
                        .get(i.saturating_add(26))
                        .copied()
                        .unwrap_or(f64::MAX);

            if tk_bullish && above_cloud && chikou_confirmed {
                // 确认: TK交叉后confirm_period根K线保持
                let mut confirmed = true;
                for j in 1..=self.confirm_period {
                    if i + j < n {
                        let ft = ichi.tenkan_sen[i + j];
                        let fk = ichi.kijun_sen[i + j];
                        if !(ft.is_nan() || fk.is_nan()) && ft <= fk {
                            confirmed = false;
                            break;
                        }
                    }
                }
                if confirmed {
                    signals[i + self.confirm_period] = Signal::Buy;
                }
            }

            // 空头条件: TK死叉 + 价格在云层下方
            let tk_bearish = tenkan < kijun;
            let below_cloud = price < span_a.min(span_b);

            if tk_bearish && below_cloud {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 1️⃣4️⃣ ADX + DI 趋势强度过滤策略
// ========================================================================
pub struct AdxDiStrategy {
    adx_period: usize,
    /// ADX 阈值，超过表示趋势较强 (默认25)
    adx_threshold: f64,
}

impl AdxDiStrategy {
    pub fn new(adx_period: usize, adx_threshold: f64) -> Self {
        Self {
            adx_period,
            adx_threshold,
        }
    }
}

impl Strategy for AdxDiStrategy {
    fn name(&self) -> &str {
        "ADX + DI Trend Strength"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        let adx_out = indicators::adx_di(&highs, &lows, &closes, self.adx_period);
        let n = closes.len();
        let mut signals = vec![Signal::Hold; n];

        let warmup = self.adx_period * 2 + 1;
        if n <= warmup {
            return signals;
        }

        for i in warmup..n {
            let adx = adx_out.adx[i];
            let plus_di = adx_out.plus_di[i];
            let minus_di = adx_out.minus_di[i];
            let prev_plus = adx_out.plus_di[i - 1];
            let prev_minus = adx_out.minus_di[i - 1];

            if adx.is_nan() || plus_di.is_nan() || minus_di.is_nan() {
                continue;
            }

            // 买入: 趋势强(ADX>阈值) + +DI > -DI + +DI上穿-DI
            if adx > self.adx_threshold && plus_di > minus_di && prev_plus <= prev_minus {
                signals[i] = Signal::Buy;
            }
            // 卖出: 趋势强(ADX>阈值) + -DI > +DI + -DI上穿+DI
            else if adx > self.adx_threshold && minus_di > plus_di && prev_minus <= prev_plus {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 1️⃣5️⃣ ATR 追踪止损策略
// ========================================================================
pub struct AtrTrailingStopStrategy {
    atr_period: usize,
    multiplier: f64,
}

impl AtrTrailingStopStrategy {
    pub fn new(atr_period: usize, multiplier: f64) -> Self {
        Self {
            atr_period,
            multiplier,
        }
    }
}

impl Strategy for AtrTrailingStopStrategy {
    fn name(&self) -> &str {
        "ATR Trailing Stop"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        let ats =
            indicators::atr_trailing_stop(&highs, &lows, &closes, self.atr_period, self.multiplier);
        let n = closes.len();
        let mut signals = vec![Signal::Hold; n];

        let warmup = self.atr_period + 1;
        if n <= warmup {
            return signals;
        }

        for i in warmup..n {
            let curr_dir = ats.direction[i];
            let prev_dir = ats.direction[i - 1];

            if prev_dir == 0 || curr_dir == 0 {
                continue;
            }

            // 从空头止损翻多头止损 → 买入
            if prev_dir == -1 && curr_dir == 1 {
                signals[i] = Signal::Buy;
            }
            // 从多头止损翻空头止损 → 卖出
            else if prev_dir == 1 && curr_dir == -1 {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 1️⃣6️⃣ 随机指标 + RSI 双振荡器策略
// ========================================================================
pub struct StochasticRsiStrategy {
    stoch_k_period: usize,
    stoch_d_period: usize,
    rsi_period: usize,
    oversold: f64,
    overbought: f64,
}

impl StochasticRsiStrategy {
    pub fn new(
        stoch_k_period: usize,
        stoch_d_period: usize,
        rsi_period: usize,
        oversold: f64,
        overbought: f64,
    ) -> Self {
        Self {
            stoch_k_period,
            stoch_d_period,
            rsi_period,
            oversold,
            overbought,
        }
    }
}

impl Strategy for StochasticRsiStrategy {
    fn name(&self) -> &str {
        "Stochastic + RSI Dual Oscillator"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        let stoch = indicators::stochastic(
            &highs,
            &lows,
            &closes,
            self.stoch_k_period,
            self.stoch_d_period,
        );
        let rsi_vals = indicators::rsi(&closes, self.rsi_period);
        let n = closes.len();
        let mut signals = vec![Signal::Hold; n];

        let warmup = self.stoch_k_period.max(self.rsi_period) + self.stoch_d_period * 2;
        if n <= warmup {
            return signals;
        }

        for i in warmup..n {
            let k = stoch.k[i];
            let d = stoch.d[i];
            let prev_k = stoch.k[i - 1];
            let prev_d = stoch.d[i - 1];
            let rsi_val = rsi_vals[i];
            let prev_rsi = rsi_vals[i - 1];

            if k.is_nan() || d.is_nan() || rsi_val.is_nan() {
                continue;
            }

            // 买入: K从下方上穿D(金叉) + RSI在超卖区域回升
            if prev_k <= prev_d && k > d && rsi_val < self.oversold && prev_rsi <= rsi_val {
                signals[i] = Signal::Buy;
            }
            // 卖出: K从上方下穿D(死叉) + RSI在超买区域回落
            else if prev_k >= prev_d && k < d && rsi_val > self.overbought && prev_rsi >= rsi_val
            {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 1️⃣7️⃣ Williams %R 动量策略
// ========================================================================
pub struct WilliamsRStrategy {
    period: usize,
    oversold: f64,   // 默认 -80
    overbought: f64, // 默认 -20
}

impl WilliamsRStrategy {
    pub fn new(period: usize, oversold: f64, overbought: f64) -> Self {
        Self {
            period,
            oversold,
            overbought,
        }
    }
}

impl Strategy for WilliamsRStrategy {
    fn name(&self) -> &str {
        "Williams %R Momentum"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        let wr = indicators::williams_r(&highs, &lows, &closes, self.period);
        let n = closes.len();
        let mut signals = vec![Signal::Hold; n];

        if n <= self.period {
            return signals;
        }

        for i in self.period..n {
            let curr = wr[i];
            let prev = wr[i - 1];
            if curr.is_nan() {
                continue;
            }

            // 买入: %R 从超卖区(-100附近)回升
            if prev <= self.oversold && curr > self.oversold {
                signals[i] = Signal::Buy;
            }
            // 卖出: %R 从超买区(0附近)回落
            else if prev >= self.overbought && curr < self.overbought {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 1️⃣8️⃣ OBV 量价确认策略
// ========================================================================
pub struct ObvMomentumStrategy {
    sma_period: usize,
}

impl ObvMomentumStrategy {
    pub fn new(sma_period: usize) -> Self {
        Self { sma_period }
    }
}

impl Strategy for ObvMomentumStrategy {
    fn name(&self) -> &str {
        "OBV Volume Confirmation"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let volumes: Vec<f64> = candles.iter().map(|c| c.volume).collect();

        let obv_vals = indicators::obv(&closes, &volumes);
        let obv_sma = indicators::sma(&obv_vals, self.sma_period);
        let n = closes.len();
        let mut signals = vec![Signal::Hold; n];

        let warmup = self.sma_period + 1;
        if n <= warmup {
            return signals;
        }

        for i in warmup..n {
            let obv = obv_vals[i];
            let sma = obv_sma[i];
            let prev_obv = obv_vals[i - 1];
            let prev_sma = obv_sma[i - 1];

            if obv.is_nan() || sma.is_nan() {
                continue;
            }

            // 买入: OBV 上穿其SMA (资金流入确认)
            if prev_obv <= prev_sma && obv > sma {
                signals[i] = Signal::Buy;
            }
            // 卖出: OBV 下穿其SMA (资金流出确认)
            else if prev_obv >= prev_sma && obv < sma {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 1️⃣9️⃣ 多因子动量策略 (Multi-Factor Momentum)
// ========================================================================
pub struct MultiFactorMomentum {
    fast_ema: usize,
    slow_ema: usize,
    rsi_period: usize,
    rsi_oversold: f64,
    rsi_overbought: f64,
    volume_sma_period: usize,
}

impl MultiFactorMomentum {
    pub fn new(
        fast_ema: usize,
        slow_ema: usize,
        rsi_period: usize,
        rsi_oversold: f64,
        rsi_overbought: f64,
        volume_sma_period: usize,
    ) -> Self {
        Self {
            fast_ema,
            slow_ema,
            rsi_period,
            rsi_oversold,
            rsi_overbought,
            volume_sma_period,
        }
    }
}

impl Strategy for MultiFactorMomentum {
    fn name(&self) -> &str {
        "Multi-Factor Momentum"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let volumes: Vec<f64> = candles.iter().map(|c| c.volume).collect();

        let fast = indicators::ema(&closes, self.fast_ema);
        let slow = indicators::ema(&closes, self.slow_ema);
        let rsi_vals = indicators::rsi(&closes, self.rsi_period);
        let vol_sma = indicators::sma(&volumes, self.volume_sma_period);

        let n = closes.len();
        let mut signals = vec![Signal::Hold; n];

        let warmup = self
            .slow_ema
            .max(self.rsi_period)
            .max(self.volume_sma_period);
        if n <= warmup + 1 {
            return signals;
        }

        for i in (warmup + 1)..n {
            let f = fast[i];
            let s = slow[i];
            let pf = fast[i - 1];
            let ps = slow[i - 1];
            let rsi = rsi_vals[i];
            let vol = volumes[i];
            let avg_vol = vol_sma[i];

            if f.is_nan() || s.is_nan() || rsi.is_nan() || avg_vol.is_nan() {
                continue;
            }

            // 因子1: EMA金叉/死叉
            let trend_bullish = pf <= ps && f > s;
            let trend_bearish = pf >= ps && f < s;
            let _in_uptrend = f > s;
            let in_downtrend = f < s;

            // 因子2: RSI 不在极端区域
            let rsi_ok_buy = rsi < self.rsi_overbought;
            let rsi_ok_sell = rsi > self.rsi_oversold;

            // 因子3: 成交量放大确认
            let vol_confirm = vol > avg_vol * 1.2; // 超过均值20%

            // 买入: 金叉 + RSI不超买 + 放量
            if trend_bullish && rsi_ok_buy && vol_confirm {
                signals[i] = Signal::Buy;
            }
            // 卖出: 死叉 + RSI不超卖 + 放量 (或处于下跌趋势中)
            else if (trend_bearish || in_downtrend) && rsi_ok_sell && vol_confirm {
                signals[i] = Signal::Sell;
            }
        }

        signals
    }
}

// ========================================================================
// 2️⃣0️⃣ 配对交易策略 (Pairs Trading - 单资产简化版: 均值回归到历史中位)
// ========================================================================
pub struct PairsTradingStrategy {
    lookback: usize,
    entry_z: f64, // 入场 Z-Score 阈值 (默认 2.0)
    exit_z: f64,  // 出场 Z-Score 阈值 (默认 0.5)
}

impl PairsTradingStrategy {
    pub fn new(lookback: usize, entry_z: f64, exit_z: f64) -> Self {
        Self {
            lookback,
            entry_z,
            exit_z,
        }
    }
}

impl Strategy for PairsTradingStrategy {
    fn name(&self) -> &str {
        "Pairs Trading (Mean Rev)"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let n = closes.len();
        let mut signals = vec![Signal::Hold; n];

        if n < self.lookback + 1 {
            return signals;
        }

        let mut in_position = false;

        for i in self.lookback..n {
            let window = &closes[i - self.lookback..i];
            let mean: f64 = window.iter().sum::<f64>() / self.lookback as f64;
            let variance: f64 =
                window.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / self.lookback as f64;
            let std = variance.sqrt();

            if std < f64::EPSILON {
                continue;
            }

            let z_score = (closes[i] - mean) / std;

            if !in_position {
                // Z-Score > +entry → 价格过高，做空
                if z_score > self.entry_z {
                    signals[i] = Signal::Sell;
                    in_position = true;
                }
                // Z-Score < -entry → 价格过低，做多
                else if z_score < -self.entry_z {
                    signals[i] = Signal::Buy;
                    in_position = true;
                }
            } else {
                // Z-Score 回归到 exit 以内 → 平仓
                if z_score.abs() < self.exit_z {
                    // 之前做空 → 现在买入平仓
                    // 之前做多 → 现在卖出平仓
                    // 简化处理: 根据z_score方向决定
                    if z_score > 0.0 {
                        signals[i] = Signal::Buy; // 之前做空, 现在买入
                    } else {
                        signals[i] = Signal::Sell; // 之前做多, 现在卖出
                    }
                    in_position = false;
                }
            }
        }

        signals
    }
}

// ========================================================================
// 创建所有内置策略列表
// ========================================================================
pub fn create_default_strategies() -> Vec<Box<dyn Strategy>> {
    vec![
        Box::new(SmaCrossover::new(5, 20)),
        Box::new(SmaCrossoverWithRsi::new(5, 20, 14, 30.0, 70.0)),
        Box::new(RsiMeanReversion::new(14, 30.0, 70.0)),
        Box::new(MacdStrategy::new(12, 26, 9, MacdMode::Crossover)),
        Box::new(MacdStrategy::new(
            12,
            26,
            9,
            MacdMode::CrossoverWithHistogram,
        )),
        Box::new(MacdStrategy::new(
            12,
            26,
            9,
            MacdMode::CrossoverWithDivergence,
        )),
        Box::new(TurtleTradingStrategy::new(20, 10, 20, 2.0)),
        Box::new(TripleEmaStrategy::new(5, 13, 34)),
        Box::new(BollingerBandsStrategy::new(20, 2.0, 0.95, 0.95, false)),
        Box::new(BollingerBandsStrategy::new(20, 2.0, 0.95, 0.95, true)),
        Box::new(VwapRsiStrategy::new(14, 30.0, 70.0, 1.0)),
        Box::new(SuperTrendStrategy::new(10, 3.0)),
        Box::new(KeltnerChannelsStrategy::new(
            20,
            14,
            2.0,
            KeltnerMode::Breakout,
        )),
        Box::new(KeltnerChannelsStrategy::new(
            20,
            14,
            2.0,
            KeltnerMode::Reversion,
        )),
        Box::new(ParabolicSarStrategy::new(0.02, 0.2)),
        // 新增策略
        Box::new(IchimokuStrategy::new(1)),
        Box::new(AdxDiStrategy::new(14, 25.0)),
        Box::new(AtrTrailingStopStrategy::new(10, 3.0)),
        Box::new(StochasticRsiStrategy::new(14, 3, 14, 30.0, 70.0)),
        Box::new(WilliamsRStrategy::new(14, -80.0, -20.0)),
        Box::new(ObvMomentumStrategy::new(20)),
        Box::new(MultiFactorMomentum::new(12, 26, 14, 30.0, 70.0, 20)),
        Box::new(PairsTradingStrategy::new(20, 2.0, 0.5)),
    ]
}

pub fn create_ensemble_strategy() -> Box<dyn Strategy> {
    Box::new(EnsembleStrategy::new(vec![
        Box::new(MacdStrategy::new(12, 26, 9, MacdMode::Crossover)),
        Box::new(TripleEmaStrategy::new(5, 13, 34)),
        Box::new(VwapRsiStrategy::new(14, 30.0, 70.0, 1.0)),
        Box::new(SuperTrendStrategy::new(10, 3.0)),
        Box::new(ParabolicSarStrategy::new(0.02, 0.2)),
    ]))
}

// ========================================================================
// 测试
// ========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading::data::DataSource;

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
    fn test_macd_strategy_all_modes() {
        let candles = create_test_candles();
        for mode in &[
            MacdMode::Crossover,
            MacdMode::CrossoverWithHistogram,
            MacdMode::CrossoverWithDivergence,
        ] {
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
    fn test_supertrend_strategy() {
        // 使用更多数据确保SuperTrend有足够的数据点计算
        let candles = DataSource::generate_mock(500, 100.0);
        let strategy = SuperTrendStrategy::new(10, 3.0);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
        // SuperTrend可能不产生信号（取决于价格波动）
        // 只要能正确运行即可
        let _non_hold = signals.iter().filter(|s| **s != Signal::Hold).count();
    }

    #[test]
    fn test_keltner_channels_all_modes() {
        let candles = create_test_candles();
        for mode in &[KeltnerMode::Breakout, KeltnerMode::Reversion] {
            let strategy = KeltnerChannelsStrategy::new(20, 14, 2.0, *mode);
            let signals = strategy.generate(&candles);
            assert_eq!(signals.len(), candles.len());
        }
    }

    #[test]
    fn test_parabolic_sar_strategy() {
        let candles = create_test_candles();
        let strategy = ParabolicSarStrategy::new(0.02, 0.2);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_ensemble_strategy() {
        let candles = create_test_candles();
        let ensemble = create_ensemble_strategy();
        let signals = ensemble.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_all_strategies() {
        let candles = create_test_candles();
        let strategies = create_default_strategies();
        for strat in &strategies {
            let signals = strat.generate(&candles);
            assert_eq!(signals.len(), candles.len());
        }
    }

    #[test]
    fn test_try_new_sma_crossover_valid() {
        let result = SmaCrossover::try_new(5, 20);
        assert!(result.is_ok());
    }

    #[test]
    fn test_try_new_sma_crossover_invalid() {
        let result = SmaCrossover::try_new(20, 5);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("快周期"));
    }

    #[test]
    fn test_try_new_rsi_invalid_thresholds() {
        let result = SmaCrossoverWithRsi::try_new(5, 20, 14, 70.0, 30.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("超卖阈值"));
    }

    #[test]
    fn test_try_new_macd_invalid_periods() {
        let result = MacdStrategy::try_new(26, 12, 9, MacdMode::Crossover);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("快周期"));
    }

    #[test]
    fn test_try_new_turtle_zero_periods() {
        let result = TurtleTradingStrategy::try_new(0, 10, 20, 2.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("不能为零"));
    }

    #[test]
    fn test_try_new_triple_ema_invalid_order() {
        let result = TripleEmaStrategy::try_new(20, 5, 13);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("fast < mid < slow"));
    }
}
