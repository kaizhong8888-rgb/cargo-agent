/// 市场状态识别模块
/// 包括：ADX趋势/震荡过滤器、波动率regimes分类、多时间框架分析、市场状态机
use super::data::Candle;
use super::indicators;

use serde::{Deserialize, Serialize};

/// 市场状态枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketRegime {
    /// 强趋势 (上涨)
    StrongUptrend,
    /// 强趋势 (下跌)
    StrongDowntrend,
    /// 弱趋势 (上涨)
    WeakUptrend,
    /// 弱趋势 (下跌)
    WeakDowntrend,
    /// 震荡/横盘
    Ranging,
    /// 高波动 (突破可能)
    HighVolatilityBreakout,
    /// 低波动 (酝酿中)
    LowVolatilitySqueeze,
}

impl MarketRegime {
    /// 是否为趋势状态
    pub fn is_trending(&self) -> bool {
        matches!(
            self,
            MarketRegime::StrongUptrend
                | MarketRegime::StrongDowntrend
                | MarketRegime::WeakUptrend
                | MarketRegime::WeakDowntrend
        )
    }

    /// 是否为震荡状态
    pub fn is_ranging(&self) -> bool {
        matches!(self, MarketRegime::Ranging)
    }

    /// 是否适合趋势跟踪策略
    pub fn is_suitable_for_trend_following(&self) -> bool {
        matches!(
            self,
            MarketRegime::StrongUptrend | MarketRegime::StrongDowntrend
        )
    }

    /// 是否适合均值回归策略
    pub fn is_suitable_for_mean_reversion(&self) -> bool {
        matches!(
            self,
            MarketRegime::Ranging | MarketRegime::WeakUptrend | MarketRegime::WeakDowntrend
        )
    }

    /// 是否适合突破策略
    pub fn is_suitable_for_breakout(&self) -> bool {
        matches!(self, MarketRegime::LowVolatilitySqueeze)
    }

    /// 趋势方向
    pub fn trend_direction(&self) -> f64 {
        match self {
            MarketRegime::StrongUptrend | MarketRegime::WeakUptrend => 1.0,
            MarketRegime::StrongDowntrend | MarketRegime::WeakDowntrend => -1.0,
            _ => 0.0,
        }
    }
}

/// 波动率等级
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VolatilityRegime {
    /// 低波动 (后10%分位)
    Low,
    /// 中等波动
    Medium,
    /// 高波动 (前10%分位)
    High,
    /// 极端波动
    Extreme,
}

/// 市场状态检测器
pub struct MarketRegimeDetector {
    /// ADX 趋势强度阈值
    adx_strong_trend: f64,
    adx_weak_trend: f64,
    /// ATR 周期
    atr_period: usize,
    /// ADX 周期
    adx_period: usize,
    /// 布林带周期
    bb_period: usize,
}

impl Default for MarketRegimeDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl MarketRegimeDetector {
    pub fn new() -> Self {
        Self {
            adx_strong_trend: 30.0,
            adx_weak_trend: 20.0,
            atr_period: 14,
            adx_period: 14,
            bb_period: 20,
        }
    }

    /// 设置 ADX 阈值
    pub fn with_adx_thresholds(mut self, strong: f64, weak: f64) -> Self {
        self.adx_strong_trend = strong;
        self.adx_weak_trend = weak;
        self
    }

    /// 检测当前市场状态
    pub fn detect_regime(&self, candles: &[Candle]) -> MarketRegime {
        if candles.len() < self.adx_period * 3 {
            return MarketRegime::Ranging; // 数据不足
        }

        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        // 1. 计算 ADX
        let adx = Self::compute_adx(&highs, &lows, &closes, self.adx_period);
        let current_adx = adx.last().copied().unwrap_or(0.0);

        // 2. 计算 DI+ 和 DI-
        let (di_plus, di_minus) = Self::compute_di(&highs, &lows, &closes, self.adx_period);
        let current_di_plus = di_plus.last().copied().unwrap_or(0.0);
        let current_di_minus = di_minus.last().copied().unwrap_or(0.0);

        // 3. 计算波动率
        let atr_values = indicators::atr(&highs, &lows, &closes, self.atr_period);
        let current_atr = atr_values.last().copied().unwrap_or(0.0);
        let current_price = closes.last().copied().unwrap_or(1.0);
        let current_volatility = if current_price > 0.0 {
            current_atr / current_price
        } else {
            0.0
        };

        // 4. 波动率分位 (使用历史ATR序列)
        let vol_regime = Self::classify_volatility(&atr_values, current_volatility);

        // 5. 布林带挤压检测
        let is_squeeze = Self::detect_bollinger_squeeze(&closes, self.bb_period);

        // 6. 综合判断
        self.classify_regime(
            current_adx,
            current_di_plus,
            current_di_minus,
            vol_regime,
            is_squeeze,
        )
    }

    /// 计算 ADX (Average Directional Index)
    fn compute_adx(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<f64> {
        let len = highs.len();
        if len < period * 2 {
            return vec![f64::NAN; len];
        }

        let (di_plus, di_minus) = Self::compute_di(highs, lows, closes, period);

        // DX = |DI+ - DI-| / (DI+ + DI-) * 100
        let mut dx = vec![f64::NAN; len];
        for i in 0..len {
            if !di_plus[i].is_nan() && !di_minus[i].is_nan() {
                let sum = di_plus[i] + di_minus[i];
                if sum > 0.0 {
                    dx[i] = (di_plus[i] - di_minus[i]).abs() / sum * 100.0;
                }
            }
        }

        // ADX = DX 的 EMA
        indicators::ema(&dx, period)
    }

    /// 计算 DI+ 和 DI-
    fn compute_di(
        highs: &[f64],
        lows: &[f64],
        closes: &[f64],
        period: usize,
    ) -> (Vec<f64>, Vec<f64>) {
        let len = highs.len();
        if len < 2 {
            return (vec![f64::NAN; len], vec![f64::NAN; len]);
        }

        // 计算 +DM 和 -DM
        let mut plus_dm = vec![0.0; len];
        let mut minus_dm = vec![0.0; len];
        let mut tr_values = vec![0.0; len];

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

            // True Range
            let high_low = highs[i] - lows[i];
            let high_close = (highs[i] - closes[i - 1]).abs();
            let low_close = (lows[i] - closes[i - 1]).abs();
            tr_values[i] = high_low.max(high_close).max(low_close);
        }

        // 平滑
        let mut smooth_plus_dm = vec![f64::NAN; len];
        let mut smooth_minus_dm = vec![f64::NAN; len];
        let mut smooth_tr = vec![f64::NAN; len];

        // 初始值 (前 period 个的和)
        if len > period {
            let sum_plus: f64 = plus_dm[1..=period].iter().sum();
            let sum_minus: f64 = minus_dm[1..=period].iter().sum();
            let sum_tr: f64 = tr_values[1..=period].iter().sum();

            smooth_plus_dm[period] = sum_plus;
            smooth_minus_dm[period] = sum_minus;
            smooth_tr[period] = sum_tr;

            for i in (period + 1)..len {
                smooth_plus_dm[i] =
                    smooth_plus_dm[i - 1] - smooth_plus_dm[i - 1] / period as f64 + plus_dm[i];
                smooth_minus_dm[i] =
                    smooth_minus_dm[i - 1] - smooth_minus_dm[i - 1] / period as f64 + minus_dm[i];
                smooth_tr[i] = smooth_tr[i - 1] - smooth_tr[i - 1] / period as f64 + tr_values[i];
            }
        }

        // 计算 DI+ 和 DI-
        let mut di_plus = vec![f64::NAN; len];
        let mut di_minus = vec![f64::NAN; len];

        for i in period..len {
            if smooth_tr[i] > 0.0 {
                di_plus[i] = smooth_plus_dm[i] / smooth_tr[i] * 100.0;
                di_minus[i] = smooth_minus_dm[i] / smooth_tr[i] * 100.0;
            }
        }

        (di_plus, di_minus)
    }

    /// 波动率分类
    fn classify_volatility(atr_values: &[f64], current_vol: f64) -> VolatilityRegime {
        // 过滤掉 NaN
        let valid_atr: Vec<f64> = atr_values
            .iter()
            .filter(|&&v| !v.is_nan())
            .copied()
            .collect();

        if valid_atr.len() < 20 {
            return VolatilityRegime::Medium;
        }

        // 计算分位数
        let mut sorted = valid_atr.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let n = sorted.len();
        let low_threshold = sorted[(n as f64 * 0.25) as usize];
        let high_threshold = sorted[(n as f64 * 0.75) as usize];
        let extreme_threshold = sorted[(n as f64 * 0.90) as usize];

        if current_vol >= extreme_threshold {
            VolatilityRegime::Extreme
        } else if current_vol >= high_threshold {
            VolatilityRegime::High
        } else if current_vol >= low_threshold {
            VolatilityRegime::Medium
        } else {
            VolatilityRegime::Low
        }
    }

    /// 布林带挤压检测
    fn detect_bollinger_squeeze(closes: &[f64], period: usize) -> bool {
        if closes.len() < period * 2 {
            return false;
        }

        let bb = indicators::bollinger_bands(closes, period, 2.0);

        // 计算最近几个周期的带宽
        let recent_bandwidths: Vec<f64> = bb
            .upper
            .iter()
            .zip(bb.lower.iter())
            .zip(bb.middle.iter())
            .skip(closes.len().saturating_sub(period))
            .map(|((u, l), m)| {
                if *m > 0.0 && !u.is_nan() && !l.is_nan() {
                    (u - l) / m
                } else {
                    f64::NAN
                }
            })
            .filter(|&v| !v.is_nan())
            .collect();

        if recent_bandwidths.len() < 5 {
            return false;
        }

        // 历史带宽的中位数
        let mut all_bandwidths: Vec<f64> = bb
            .upper
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
            .filter(|&v| !v.is_nan())
            .collect();

        if all_bandwidths.is_empty() {
            return false;
        }

        all_bandwidths.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median_bandwidth = all_bandwidths[all_bandwidths.len() / 2];

        // 当前带宽低于历史中位数的 60% → 挤压
        let current_bandwidth = recent_bandwidths.last().copied().unwrap_or(f64::MAX);
        current_bandwidth < median_bandwidth * 0.6
    }

    /// 综合分类市场状态
    fn classify_regime(
        &self,
        adx: f64,
        di_plus: f64,
        di_minus: f64,
        vol_regime: VolatilityRegime,
        is_squeeze: bool,
    ) -> MarketRegime {
        // 挤压优先
        if is_squeeze && vol_regime == VolatilityRegime::Low {
            return MarketRegime::LowVolatilitySqueeze;
        }

        // 极端波动
        if vol_regime == VolatilityRegime::Extreme {
            return MarketRegime::HighVolatilityBreakout;
        }

        // 趋势判断 (基于 ADX)
        if adx >= self.adx_strong_trend {
            if di_plus > di_minus {
                return MarketRegime::StrongUptrend;
            } else {
                return MarketRegime::StrongDowntrend;
            }
        } else if adx >= self.adx_weak_trend {
            if di_plus > di_minus {
                return MarketRegime::WeakUptrend;
            } else {
                return MarketRegime::WeakDowntrend;
            }
        }

        // 默认震荡
        MarketRegime::Ranging
    }
}

/// 多时间框架分析器
pub struct MultiTimeframeAnalyzer {
    /// 长期趋势判断周期
    long_period: usize,
    /// 中期趋势判断周期
    mid_period: usize,
    /// 短期趋势判断周期
    short_period: usize,
}

impl MultiTimeframeAnalyzer {
    pub fn new(long: usize, mid: usize, short_: usize) -> Self {
        Self {
            long_period: long,
            mid_period: mid,
            short_period: short_,
        }
    }

    /// 多时间框架趋势分析
    /// 返回: (长期趋势, 中期趋势, 短期趋势, 综合信号强度)
    /// 趋势值: -1 (强空) ~ 1 (强多)
    pub fn analyze_trend(&self, closes: &[f64]) -> (f64, f64, f64, f64) {
        if closes.len() < self.long_period {
            return (0.0, 0.0, 0.0, 0.0);
        }

        let long_trend = Self::trend_strength(closes, self.long_period);
        let mid_trend = Self::trend_strength(closes, self.mid_period);
        let short_trend = Self::trend_strength(closes, self.short_period);

        // 综合信号: 加权平均 (长期权重更高)
        let combined = long_trend * 0.5 + mid_trend * 0.3 + short_trend * 0.2;

        (long_trend, mid_trend, short_trend, combined)
    }

    /// 计算某个周期的趋势强度
    /// 基于 EMA 斜率和价格相对位置
    fn trend_strength(closes: &[f64], period: usize) -> f64 {
        if closes.len() < period {
            return 0.0;
        }

        let ema_values = indicators::ema(closes, period);
        let last_ema = ema_values.last().copied().unwrap_or(0.0);
        let prev_ema = ema_values.iter().rev().nth(1).copied().unwrap_or(0.0);

        if last_ema == 0.0 {
            return 0.0;
        }

        // EMA 变化率
        let ema_change = (last_ema - prev_ema) / last_ema;

        // 价格相对 EMA 位置
        let last_price = closes.last().copied().unwrap_or(0.0);
        let price_vs_ema = if last_ema > 0.0 {
            (last_price - last_ema) / last_ema
        } else {
            0.0
        };

        // 综合: EMA 变化率 + 价格位置
        (ema_change * 100.0 + price_vs_ema).clamp(-1.0, 1.0)
    }
}

/// Hurst 指数计算器 (用于判断趋势持续性)
pub struct HurstExponent;

impl HurstExponent {
    /// 简化的 Hurst 指数计算
    /// H > 0.5: 趋势持续 (trend persistent)
    /// H = 0.5: 随机游走 (random walk)
    /// H < 0.5: 均值回归 (mean reverting)
    pub fn calculate(returns: &[f64], max_lag: usize) -> f64 {
        if returns.len() < max_lag * 2 || max_lag < 2 {
            return 0.5;
        }

        let mut lags = Vec::new();
        let mut rs_values = Vec::new();

        for lag in 2..=max_lag {
            if lag >= returns.len() {
                break;
            }

            // 计算累积偏差
            let cum_dev = vec![0.0; returns.len() - lag + 1];
            for i in 0..cum_dev.len() {
                let segment = &returns[i..i + lag];
                let mean: f64 = segment.iter().sum::<f64>() / segment.len() as f64;
                let mut sum = 0.0;
                let mut max_dev = f64::NEG_INFINITY;
                let mut min_dev = f64::INFINITY;

                for &r in segment {
                    sum += r - mean;
                    max_dev = max_dev.max(sum);
                    min_dev = min_dev.min(sum);
                }

                // R = max - min
                let r_range = max_dev - min_dev;

                // S = 标准差
                let variance: f64 =
                    segment.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / segment.len() as f64;
                let std = variance.sqrt();

                if std > 0.0 {
                    lags.push(lag as f64);
                    rs_values.push((r_range / std).ln());
                }
            }
        }

        if lags.len() < 3 {
            return 0.5;
        }

        // 线性回归: log(R/S) = H * log(lag) + C
        let log_lags: Vec<f64> = lags.iter().map(|&l| l.ln()).collect();
        let n = log_lags.len() as f64;

        let sum_x: f64 = log_lags.iter().sum();
        let sum_y: f64 = rs_values.iter().sum();
        let sum_xy: f64 = log_lags
            .iter()
            .zip(rs_values.iter())
            .map(|(x, y)| x * y)
            .sum();
        let sum_x2: f64 = log_lags.iter().map(|x| x * x).sum();

        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator.abs() < 1e-10 {
            return 0.5;
        }

        let h = (n * sum_xy - sum_x * sum_y) / denominator;
        h.clamp(0.0, 1.0)
    }
}

/// 策略推荐器 (根据市场状态推荐合适的策略类型)
pub struct StrategyRecommender;

impl StrategyRecommender {
    pub fn recommend(
        regime: MarketRegime,
        hurst: f64,
        _volatility: VolatilityRegime,
    ) -> Vec<&'static str> {
        let mut recommendations = Vec::new();

        match regime {
            MarketRegime::StrongUptrend | MarketRegime::StrongDowntrend => {
                recommendations.push("Triple EMA Trend");
                recommendations.push("MACD Crossover");
                recommendations.push("SuperTrend");
                recommendations.push("Turtle Trading");
                recommendations.push("ADX + DI Trend Strength");
            }
            MarketRegime::WeakUptrend | MarketRegime::WeakDowntrend => {
                recommendations.push("SMA Crossover + RSI Filter");
                recommendations.push("Bollinger Bands MeanRev");
                recommendations.push("MACD + Histogram");
            }
            MarketRegime::Ranging => {
                recommendations.push("RSI Mean Reversion");
                recommendations.push("Bollinger Bands MeanRev");
                recommendations.push("Stochastic + RSI Dual Oscillator");
                recommendations.push("VWAP + RSI Reversion");
                recommendations.push("Williams %R Momentum");
            }
            MarketRegime::LowVolatilitySqueeze => {
                recommendations.push("Bollinger Bands + Squeeze");
                recommendations.push("Keltner Breakout");
                recommendations.push("Turtle Trading");
            }
            MarketRegime::HighVolatilityBreakout => {
                recommendations.push("ATR Trailing Stop");
                recommendations.push("Parabolic SAR");
                recommendations.push("Keltner Breakout");
            }
        }

        // Hurst 指数补充
        if hurst > 0.6 {
            recommendations.push("Pairs Trading (Mean Rev)");
        } else if hurst < 0.4 {
            recommendations.push("Multi-Factor Momentum");
        }

        recommendations
    }
}

#[cfg(test)]
mod tests {
    use super::super::data::DataSource;
    use super::*;

    #[test]
    fn test_detect_regime() {
        let detector = MarketRegimeDetector::new();
        let candles = DataSource::generate_mock(200, 100.0);
        let regime = detector.detect_regime(&candles);
        // 模拟数据可能产生任何状态，但不应 panic
        println!("Detected regime: {:?}", regime);
    }

    #[test]
    fn test_multi_timeframe_analysis() {
        let analyzer = MultiTimeframeAnalyzer::new(50, 20, 10);
        let closes: Vec<f64> = (1..=100).map(|x| x as f64).collect(); // 上涨趋势
        let (long, mid, short, combined) = analyzer.analyze_trend(&closes);

        // 上涨趋势应该产生正值
        assert!(
            combined > 0.0,
            "Combined trend should be positive for uptrend"
        );
        println!(
            "Long: {:.3}, Mid: {:.3}, Short: {:.3}, Combined: {:.3}",
            long, mid, short, combined
        );
    }

    #[test]
    fn test_hurst_exponent() {
        // 上涨序列应该产生 H > 0.5
        let returns: Vec<f64> = (1..=100).map(|x| (x as f64 + 1.0).ln()).collect();
        let h = HurstExponent::calculate(&returns, 20);
        println!("Hurst exponent for uptrend: {:.3}", h);
    }

    #[test]
    fn test_strategy_recommendation() {
        let recs =
            StrategyRecommender::recommend(MarketRegime::Ranging, 0.5, VolatilityRegime::Medium);
        assert!(!recs.is_empty());
        println!("Recommended strategies for ranging market: {:?}", recs);
    }
}
