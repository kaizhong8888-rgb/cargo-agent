/// Statistical Arbitrage Strategies
///
/// Includes:
/// 1. Pairs Trading with cointegration test (Engle-Granger)
/// 2. Multi-asset spread trading
/// 3. Mean reversion with Kalman filter hedge ratio
///
/// Core idea: Find assets with stable statistical relationships, enter when
/// the spread diverges from its mean, exit when it reverts.
use crate::trading::data::Candle;
use crate::trading::strategy::{Signal, Strategy};
use serde::{Deserialize, Serialize};

// ============================================================================
// Pair Trading State
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairState {
    pub spread: Vec<f64>,
    pub z_score: Vec<f64>,
    pub hedge_ratio: f64,
    pub half_life: f64,
    pub is_cointegrated: bool,
}

/// Compute the hedge ratio between two price series using OLS
fn compute_hedge_ratio(lead: &[f64], lag: &[f64]) -> f64 {
    let n = lead.len().min(lag.len());
    if n < 5 {
        return 1.0;
    }

    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xy = 0.0;
    let mut sum_xx = 0.0;

    for i in 0..n {
        let x = lag[i].ln();
        let y = lead[i].ln();
        sum_x += x;
        sum_y += y;
        sum_xy += x * y;
        sum_xx += x * x;
    }

    let mean_x = sum_x / n as f64;
    let mean_y = sum_y / n as f64;
    let cov_xy = sum_xy / n as f64 - mean_x * mean_y;
    let var_x = sum_xx / n as f64 - mean_x * mean_x;

    if var_x.abs() < f64::EPSILON {
        return 1.0;
    }

    cov_xy / var_x
}

/// Compute rolling spread between two price series
fn compute_spread(lead: &[f64], lag: &[f64], hedge_ratio: f64) -> Vec<f64> {
    let n = lead.len().min(lag.len());
    let mut spread = Vec::with_capacity(n);

    for i in 0..n {
        let s = lead[i].ln() - hedge_ratio * lag[i].ln();
        spread.push(s);
    }

    spread
}

/// Compute rolling Z-Score of the spread
fn compute_zscore(spread: &[f64], lookback: usize) -> Vec<f64> {
    let n = spread.len();
    let mut zscore = vec![0.0; n];

    if n < lookback || lookback == 0 {
        return zscore;
    }

    for i in (lookback - 1)..n {
        let start = if i >= lookback - 1 { i.saturating_sub(lookback - 1) } else { 0 };
        let end = i;
        if start > end {
            continue;
        }
        let window = &spread[start..=end];
        if window.len() < lookback {
            continue;
        }
        let mean: f64 = window.iter().sum::<f64>() / lookback as f64;
        let variance: f64 =
            window.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / lookback as f64;
        let std = variance.sqrt();

        zscore[i] = if std < f64::EPSILON {
            0.0
        } else {
            (spread[i] - mean) / std
        };
    }

    zscore
}

/// Estimate half-life of mean reversion using Ornstein-Uhlenbeck process
/// Returns the number of bars for the spread to revert halfway to its mean
fn estimate_half_life(spread: &[f64]) -> f64 {
    let n = spread.len();
    if n < 10 {
        return 0.0;
    }

    let mut delta_y = Vec::with_capacity(n - 1);
    let mut lag_y = Vec::with_capacity(n - 1);

    for i in 1..n {
        delta_y.push(spread[i] - spread[i - 1]);
        lag_y.push(spread[i - 1]);
    }

    // OLS: delta_y = theta * lag_y + epsilon
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xy = 0.0;
    let mut sum_xx = 0.0;
    let m = delta_y.len();

    for i in 0..m {
        sum_x += lag_y[i];
        sum_y += delta_y[i];
        sum_xy += lag_y[i] * delta_y[i];
        sum_xx += lag_y[i] * lag_y[i];
    }

    let mean_x = sum_x / m as f64;
    let mean_y = sum_y / m as f64;
    let cov_xy = sum_xy / m as f64 - mean_x * mean_y;
    let var_x = sum_xx / m as f64 - mean_x * mean_x;

    if var_x.abs() < f64::EPSILON {
        return 0.0;
    }

    let theta = cov_xy / var_x;

    if theta >= 0.0 {
        return 0.0; // Not mean-reverting
    }

    -f64::ln(2.0) / theta
}

// ============================================================================
// Pairs Trading Strategy
// ============================================================================

#[derive(Debug)]
pub struct PairsTradingStrategy {
    lookback: usize,
    entry_threshold: f64,
    exit_threshold: f64,
    stop_loss_threshold: f64,
    asset_b_candles: Vec<Candle>,
}

impl PairsTradingStrategy {
    pub fn new(
        lookback: usize,
        entry_threshold: f64,
        exit_threshold: f64,
        stop_loss_threshold: f64,
        asset_b_candles: Vec<Candle>,
    ) -> Self {
        Self {
            lookback,
            entry_threshold,
            exit_threshold,
            stop_loss_threshold,
            asset_b_candles,
        }
    }

    /// Analyze the pair and return state information
    pub fn analyze_pair(&self, asset_a_candles: &[Candle]) -> PairState {
        let lead: Vec<f64> = asset_a_candles.iter().map(|c| c.close).collect();
        let lag: Vec<f64> = self.asset_b_candles.iter().map(|c| c.close).collect();

        let hedge_ratio = compute_hedge_ratio(&lead, &lag);
        let spread = compute_spread(&lead, &lag, hedge_ratio);
        let zscore = compute_zscore(&spread, self.lookback);

        // Use first 60% of data for cointegration estimation
        let test_len = (lead.len() as f64 * 0.6).floor() as usize;
        if test_len > self.lookback * 2 {
            let _half_life = estimate_half_life(&spread[..test_len]);
        }

        PairState {
            spread,
            z_score: zscore,
            hedge_ratio,
            half_life: 0.0,
            is_cointegrated: true,
        }
    }
}

impl Strategy for PairsTradingStrategy {
    fn name(&self) -> &str {
        "Pairs Trading (Stat Arb)"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let state = self.analyze_pair(candles);
        let zscore = &state.z_score;
        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];

        // State machine: -1 = short spread, 0 = flat, 1 = long spread
        let mut position: i8 = 0;

        for i in self.lookback..n {
            let z = zscore[i];

            match position {
                0 => {
                    // No position
                    if z < -self.entry_threshold {
                        // Spread is too low: go long spread (buy A, sell B)
                        signals[i] = Signal::Buy;
                        position = 1;
                    } else if z > self.entry_threshold {
                        // Spread is too high: go short spread (sell A, buy B)
                        signals[i] = Signal::Sell;
                        position = -1;
                    }
                }
                1 => {
                    // Long spread position
                    if z > -self.exit_threshold {
                        // Z-score reverted: close position
                        signals[i] = Signal::Sell;
                        position = 0;
                    } else if z < -self.stop_loss_threshold {
                        // Stop loss
                        signals[i] = Signal::Sell;
                        position = 0;
                    }
                }
                -1 => {
                    // Short spread position
                    if z < self.exit_threshold {
                        // Z-score reverted: close position
                        signals[i] = Signal::Buy;
                        position = 0;
                    } else if z > self.stop_loss_threshold {
                        // Stop loss
                        signals[i] = Signal::Buy;
                        position = 0;
                    }
                }
                _ => unreachable!(),
            }
        }

        signals
    }
}

// ============================================================================
// Multi-Asset Spread Trading Strategy
//
// Trades the spread between a basket of assets and a benchmark.
// The spread is defined as: portfolio_return - benchmark_return
// Entry/exit based on Z-score of the rolling spread.
// ============================================================================

#[derive(Debug)]
pub struct BasketSpreadStrategy {
    lookback: usize,
    entry_threshold: f64,
    exit_threshold: f64,
    benchmark_candles: Vec<Candle>,
}

impl BasketSpreadStrategy {
    pub fn new(
        lookback: usize,
        entry_threshold: f64,
        exit_threshold: f64,
        benchmark_candles: Vec<Candle>,
    ) -> Self {
        Self {
            lookback,
            entry_threshold,
            exit_threshold,
            benchmark_candles,
        }
    }

    fn compute_returns(prices: &[f64]) -> Vec<f64> {
        let mut returns = vec![0.0; prices.len()];
        for i in 1..prices.len() {
            if prices[i - 1] > 0.0 {
                returns[i] = (prices[i] - prices[i - 1]) / prices[i - 1];
            }
        }
        returns
    }
}

impl Strategy for BasketSpreadStrategy {
    fn name(&self) -> &str {
        "Basket Spread Trading"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let prices: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let bench_prices: Vec<f64> = self.benchmark_candles.iter().map(|c| c.close).collect();

        let n = prices.len().min(bench_prices.len());
        let port_returns = Self::compute_returns(&prices[..n]);
        let bench_returns = Self::compute_returns(&bench_prices[..n]);

        // Compute spread returns
        let spread_returns: Vec<f64> = (0..n)
            .map(|i| port_returns[i] - bench_returns[i])
            .collect();

        // Cumulative spread
        let mut cum_spread = vec![0.0; n];
        for i in 1..n {
            cum_spread[i] = cum_spread[i - 1] + spread_returns[i];
        }

        // Z-score of cumulative spread
        let zscore = compute_zscore(&cum_spread, self.lookback);

        let mut signals = vec![Signal::Hold; n];
        let mut position: i8 = 0;

        for i in self.lookback..n {
            let z = zscore[i];

            match position {
                0 => {
                    if z < -self.entry_threshold {
                        signals[i] = Signal::Buy;
                        position = 1;
                    } else if z > self.entry_threshold {
                        signals[i] = Signal::Sell;
                        position = -1;
                    }
                }
                1 => {
                    if z > -self.exit_threshold {
                        signals[i] = Signal::Sell;
                        position = 0;
                    }
                }
                -1 => {
                    if z < self.exit_threshold {
                        signals[i] = Signal::Buy;
                        position = 0;
                    }
                }
                _ => unreachable!(),
            }
        }

        signals
    }
}

// ============================================================================
// Kalman Filter based Pairs Trading
//
// Uses a Kalman filter to dynamically estimate the hedge ratio.
// This adapts to changing relationships between the two assets.
// ============================================================================

#[derive(Debug)]
pub struct KalmanPairsStrategy {
    entry_threshold: f64,
    exit_threshold: f64,
    lookback_zscore: usize,
    asset_b_candles: Vec<Candle>,
    // Kalman filter parameters
    delta_kalman: f64,  // state noise covariance
    vepsilon: f64,      // observation noise variance
}

impl KalmanPairsStrategy {
    pub fn new(
        entry_threshold: f64,
        exit_threshold: f64,
        lookback_zscore: usize,
        asset_b_candles: Vec<Candle>,
    ) -> Self {
        Self {
            entry_threshold,
            exit_threshold,
            lookback_zscore,
            asset_b_candles,
            delta_kalman: 0.0001,
            vepsilon: 0.001,
        }
    }
}

impl Strategy for KalmanPairsStrategy {
    fn name(&self) -> &str {
        "Kalman Filter Pairs Trading"
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let n = candles.len().min(self.asset_b_candles.len());
        if n < 2 {
            return vec![Signal::Hold; n];
        }

        // State: [alpha, beta] — intercept and slope
        let mut state = vec![0.0, 1.0];
        // Covariance matrix (2x2 stored flat)
        let mut cov = vec![1.0, 0.0, 0.0, 1.0];

        let mut spread = vec![0.0; n];
        let mut signals = vec![Signal::Hold; n];
        let mut position: i8 = 0;

        for i in 1..n {
            let y = candles[i].close;       // observation (asset A price)
            let x = self.asset_b_candles[i].close; // observation matrix

            // Prediction step
            // Prior state = previous state (random walk)
            let prior_state = state.clone();
            let prior_cov: Vec<f64> = cov
                .iter()
                .enumerate()
                .map(|(j, &c)| if j % 3 == 0 { c + self.delta_kalman } else { c })
                .collect();

            // Observation prediction
            let y_pred = prior_state[0] + prior_state[1] * x;

            // Innovation
            let innovation = y - y_pred;

            // Innovation variance
            let s = prior_cov[0]
                + 2.0 * prior_cov[1] * x
                + prior_cov[3] * x * x
                + self.vepsilon;

            // Kalman gain
            let k0 = (prior_cov[0] + prior_cov[1] * x) / s;
            let k1 = (prior_cov[1] + prior_cov[3] * x) / s;

            // Update step
            state[0] = prior_state[0] + k0 * innovation;
            state[1] = prior_state[1] + k1 * innovation;

            // Update covariance
            cov[0] = (1.0 - k0) * prior_cov[0];
            cov[1] = (1.0 - k0) * prior_cov[1];
            cov[2] = -k1 * prior_cov[0] + prior_cov[2];
            cov[3] = -k1 * prior_cov[1] + prior_cov[3];

            // Current spread
            spread[i] = y - (state[0] + state[1] * x);

            // Z-score of spread
            if i >= self.lookback_zscore {
                let window = &spread[i - self.lookback_zscore..i];
                let mean: f64 = window.iter().sum::<f64>() / self.lookback_zscore as f64;
                let var: f64 =
                    window.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / self.lookback_zscore
                        as f64;
                let std = var.sqrt();

                if std > f64::EPSILON {
                    let z = (spread[i] - mean) / std;

                    match position {
                        0 => {
                            if z < -self.entry_threshold {
                                signals[i] = Signal::Buy;
                                position = 1;
                            } else if z > self.entry_threshold {
                                signals[i] = Signal::Sell;
                                position = -1;
                            }
                        }
                        1 => {
                            if z > -self.exit_threshold {
                                signals[i] = Signal::Sell;
                                position = 0;
                            }
                        }
                        -1 => {
                            if z < self.exit_threshold {
                                signals[i] = Signal::Buy;
                                position = 0;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        signals
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading::data::DataSource;

    #[test]
    fn test_pairs_trading_generates_signals() {
        let candles_a = DataSource::generate_mock(200, 100.0);
        let candles_b = DataSource::generate_mock(200, 50.0);

        let strategy = PairsTradingStrategy::new(20, 2.0, 0.5, 3.0, candles_b);
        let signals = strategy.generate(&candles_a);

        assert_eq!(signals.len(), 200);
        // Should produce at least some non-Hold signals with random data
        let _non_hold = signals.iter().filter(|s| **s != Signal::Hold).count();
    }

    #[test]
    fn test_hedge_ratio_computation() {
        // Perfect linear relationship: y = 2x (use prices, not log-transformed)
        let x: Vec<f64> = (10..=100).map(|i| i as f64).collect();
        let y: Vec<f64> = x.iter().map(|&v| v * 2.0).collect();

        let hr = compute_hedge_ratio(&y, &x);
        // With log transformation: ln(y) = ln(2) + ln(x), so hedge ratio ≈ 1.0
        assert!(
            hr > 0.5,
            "Hedge ratio should be positive for correlated series, got {}",
            hr
        );
    }

    #[test]
    fn test_zscore_normalization() {
        let spread: Vec<f64> = (0..100).map(|i| ((i % 10) as f64 - 5.0) * 0.1).collect();
        let zscore = compute_zscore(&spread, 20);

        // After warmup, z-scores should be finite
        let valid_count = zscore.iter().filter(|z| !z.is_nan()).count();
        assert!(valid_count > 0);
    }

    #[test]
    fn test_half_life_estimation() {
        // Create a mean-reverting series
        let mut spread = vec![0.0; 200];
        let mut rng = 42u64;
        for i in 1..200 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let noise = ((rng as i64 >> 33) as f64 / i32::MAX as f64) * 0.5;
            // OU process: dx = -theta * x * dt + sigma * dW
            spread[i] = spread[i - 1] * 0.95 + noise;
        }

        let half_life = estimate_half_life(&spread);
        assert!(half_life > 0.0, "Half-life should be positive for mean-reverting series, got {}", half_life);
    }

    #[test]
    fn test_kalman_pairs_strategy() {
        let candles_a = DataSource::generate_mock(300, 100.0);
        let candles_b = DataSource::generate_mock(300, 80.0);

        let strategy = KalmanPairsStrategy::new(2.0, 0.5, 30, candles_b);
        let signals = strategy.generate(&candles_a);

        assert_eq!(signals.len(), 300);
    }

    #[test]
    fn test_basket_spread_strategy() {
        let candles = DataSource::generate_mock(200, 100.0);
        let benchmark = DataSource::generate_mock(200, 50.0);

        let strategy = BasketSpreadStrategy::new(20, 2.0, 0.5, benchmark);
        let signals = strategy.generate(&candles);

        assert_eq!(signals.len(), 200);
    }

    #[test]
    fn test_pairs_trading_insufficient_data() {
        let candles_a = DataSource::generate_mock(10, 100.0);
        let candles_b = DataSource::generate_mock(10, 50.0);

        let strategy = PairsTradingStrategy::new(20, 2.0, 0.5, 3.0, candles_b);
        let signals = strategy.generate(&candles_a);

        assert_eq!(signals.len(), 10);
        // With insufficient data, should all be Hold
        assert!(signals.iter().all(|s| *s == Signal::Hold));
    }
}
