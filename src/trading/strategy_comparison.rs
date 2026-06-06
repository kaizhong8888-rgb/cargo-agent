/// 策略对比与排名模块
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::backtest::{BacktestEngine, PositionSizing, StopLoss};
use super::data::Candle;
use super::report::BacktestResult;
use super::strategy::Strategy;
use super::walk_forward::{WalkForwardAnalyzer, WalkForwardConfig};

/// 单个策略的回测排名信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyRankEntry {
    pub rank: usize,
    pub strategy_name: String,
    pub total_return_pct: f64,
    pub sharpe_ratio: f64,
    pub calmar_ratio: f64,
    pub max_drawdown_pct: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub total_trades: usize,
    pub final_equity: f64,
    pub sortino_ratio: f64,
    pub recovery_factor: f64,
    pub annualized_return: f64,
    pub composite_score: f64,
    pub wf_oos_sharpe: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyRanking {
    pub entries: Vec<StrategyRankEntry>,
    pub total_strategies: usize,
    pub ranking_method: String,
    pub benchmark: Option<BenchmarkResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub total_return_pct: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown_pct: f64,
    pub final_equity: f64,
}

pub struct StrategyComparator;

impl StrategyComparator {
    pub fn compare_and_rank(
        strategies: &[Box<dyn Strategy>],
        candles: &[Candle],
        initial_capital: f64,
        commission_rate: f64,
        slippage: f64,
    ) -> StrategyRanking {
        let mut entries = Vec::with_capacity(strategies.len());
        for strategy in strategies {
            let result = Self::run_single_backtest(
                strategy.as_ref(),
                candles,
                initial_capital,
                commission_rate,
                slippage,
            );
            let total_days = candles.len();
            let years = total_days as f64 / 252.0;
            let total_return_decimal = result.engine.total_return_pct / 100.0;
            let annualized_return = if years > 0.0 && (1.0 + total_return_decimal) > 0.0 {
                (1.0 + total_return_decimal).powf(1.0 / years) - 1.0
            } else {
                0.0
            };
            entries.push(StrategyRankEntry {
                rank: 0,
                strategy_name: strategy.name().to_string(),
                total_return_pct: result.engine.total_return_pct,
                sharpe_ratio: result.engine.sharpe_ratio,
                calmar_ratio: result.engine.calmar_ratio,
                max_drawdown_pct: result.engine.max_drawdown_pct,
                win_rate: result.engine.win_rate,
                profit_factor: result.engine.profit_factor,
                total_trades: result.engine.total_trades,
                final_equity: result.engine.final_equity,
                sortino_ratio: result.engine.sortino_ratio,
                recovery_factor: result.engine.recovery_factor,
                annualized_return,
                composite_score: 0.0,
                wf_oos_sharpe: None,
            });
        }
        Self::compute_composite_scores(&mut entries);
        entries.sort_by(|a, b| {
            b.composite_score
                .partial_cmp(&a.composite_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (i, entry) in entries.iter_mut().enumerate() {
            entry.rank = i + 1;
        }
        let benchmark = Self::compute_buy_hold_benchmark(candles, initial_capital);
        StrategyRanking {
            entries,
            total_strategies: strategies.len(),
            ranking_method: "composite_score".to_string(),
            benchmark,
        }
    }

    pub fn rank_by_sharpe(
        strategies: &[Box<dyn Strategy>],
        candles: &[Candle],
        initial_capital: f64,
        commission_rate: f64,
        slippage: f64,
    ) -> StrategyRanking {
        let mut ranking = Self::compare_and_rank(
            strategies,
            candles,
            initial_capital,
            commission_rate,
            slippage,
        );
        ranking.entries.sort_by(|a, b| {
            b.sharpe_ratio
                .partial_cmp(&a.sharpe_ratio)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (i, entry) in ranking.entries.iter_mut().enumerate() {
            entry.rank = i + 1;
        }
        ranking.ranking_method = "sharpe_ratio".to_string();
        ranking
    }

    pub fn rank_by_calmar(
        strategies: &[Box<dyn Strategy>],
        candles: &[Candle],
        initial_capital: f64,
        commission_rate: f64,
        slippage: f64,
    ) -> StrategyRanking {
        let mut ranking = Self::compare_and_rank(
            strategies,
            candles,
            initial_capital,
            commission_rate,
            slippage,
        );
        ranking.entries.sort_by(|a, b| {
            b.calmar_ratio
                .partial_cmp(&a.calmar_ratio)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (i, entry) in ranking.entries.iter_mut().enumerate() {
            entry.rank = i + 1;
        }
        ranking.ranking_method = "calmar_ratio".to_string();
        ranking
    }

    pub fn generate_comparison_report(ranking: &StrategyRanking) -> String {
        let mut report = String::new();
        report.push_str("# Strategy Comparison Report\n\n");
        if let Some(bm) = &ranking.benchmark {
            report
                .push_str("## Benchmark (Buy & Hold)\n\n| Metric | Value |\n|--------|-------|\n");
            report.push_str(&format!("| Total Return | {:.2}% |\n| Sharpe Ratio | {:.4} |\n| Max Drawdown | {:.2}% |\n| Final Equity | ${:.2} |\n\n", bm.total_return_pct, bm.sharpe_ratio, bm.max_drawdown_pct, bm.final_equity));
        }
        report.push_str(&format!("## Strategy Ranking (Method: {})\n\n| Rank | Strategy | Score | Return | Sharpe | Calmar | MaxDD | Win% | Trades | OOS |\n|------|----------|-------|--------|--------|--------|-------|------|--------|-----|\n", ranking.ranking_method));
        for entry in &ranking.entries {
            let oos = entry
                .wf_oos_sharpe
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "N/A".to_string());
            report.push_str(&format!(
                "| #{} | {} | {:.2} | {:.2}% | {:.3} | {:.3} | {:.2}% | {:.1}% | {} | {} |\n",
                entry.rank,
                entry.strategy_name,
                entry.composite_score,
                entry.total_return_pct,
                entry.sharpe_ratio,
                entry.calmar_ratio,
                entry.max_drawdown_pct,
                entry.win_rate,
                entry.total_trades,
                oos
            ));
        }
        report.push_str("\n## Detailed Metrics\n\n| Rank | Strategy | AnnReturn | Sortino | Recovery | ProfitFactor | FinalEquity |\n|------|----------|-----------|---------|----------|--------------|-------------|\n");
        for entry in &ranking.entries {
            report.push_str(&format!(
                "| #{} | {} | {:.2}% | {:.3} | {:.3} | {:.2} | ${:.2} |\n",
                entry.rank,
                entry.strategy_name,
                entry.annualized_return * 100.0,
                entry.sortino_ratio,
                entry.recovery_factor,
                entry.profit_factor,
                entry.final_equity
            ));
        }
        report
    }

    fn run_single_backtest(
        strategy: &dyn Strategy,
        candles: &[Candle],
        initial_capital: f64,
        commission_rate: f64,
        slippage: f64,
    ) -> BacktestResult {
        let mut engine = BacktestEngine::new(initial_capital, commission_rate, slippage)
            .with_position_sizing(PositionSizing::FixedFractional(0.5))
            .with_stop_loss(StopLoss::AtrTrailing(3.0))
            .with_max_drawdown_stop(25.0);
        match engine.run(candles, strategy) {
            Ok(trades) => BacktestResult::new(&engine, candles, &trades),
            Err(_) => BacktestResult::from_trades(&[], initial_capital),
        }
    }

    fn compute_composite_scores(entries: &mut [StrategyRankEntry]) {
        if entries.is_empty() {
            return;
        }
        let find_min_max = |selector: fn(&StrategyRankEntry) -> f64| -> (f64, f64) {
            let mut min = f64::MAX;
            let mut max = f64::MIN;
            for e in entries.iter() {
                let v = selector(e);
                if v.is_finite() {
                    min = min.min(v);
                    max = max.max(v);
                }
            }
            (min, max)
        };
        let normalize = |val: f64, lo: f64, hi: f64| -> f64 {
            let range = hi - lo;
            if range < 1e-10 {
                0.5
            } else {
                (val - lo) / range
            }
        };
        let (smin, smax) = find_min_max(|e| e.sharpe_ratio);
        let (cmin, cmax) = find_min_max(|e| e.calmar_ratio);
        let (rmin, rmax) = find_min_max(|e| e.total_return_pct);
        let (dmin, dmax) = find_min_max(|e| e.max_drawdown_pct);
        let (wmin, wmax) = find_min_max(|e| e.win_rate);
        for e in entries.iter_mut() {
            e.composite_score = normalize(e.sharpe_ratio, smin, smax) * 0.40
                + normalize(e.calmar_ratio, cmin, cmax) * 0.25
                + normalize(e.total_return_pct, rmin, rmax) * 0.15
                + (1.0 - normalize(e.max_drawdown_pct, dmin, dmax)) * 0.10
                + normalize(e.win_rate, wmin, wmax) * 0.10;
        }
    }

    fn compute_buy_hold_benchmark(
        candles: &[Candle],
        initial_capital: f64,
    ) -> Option<BenchmarkResult> {
        if candles.len() < 2 {
            return None;
        }
        let first = candles[0].close;
        let last = candles.last().unwrap().close;
        let total_return_pct = (last - first) / first * 100.0;
        let final_equity = initial_capital * (1.0 + total_return_pct / 100.0);
        let mut sum_r = 0.0;
        let mut count_r = 0usize;
        for w in candles.windows(2) {
            let r = (w[1].close - w[0].close) / w[0].close;
            if r.is_finite() {
                sum_r += r;
                count_r += 1;
            }
        }
        let sharpe_ratio = if count_r > 1 {
            let mean = sum_r / count_r as f64;
            let var: f64 = candles
                .windows(2)
                .map(|w| (w[1].close - w[0].close) / w[0].close)
                .filter(|r| r.is_finite())
                .map(|r| (r - mean).powi(2))
                .sum::<f64>()
                / count_r as f64;
            let std = var.sqrt();
            if std > 1e-10 {
                mean / std * (252.0_f64).sqrt()
            } else {
                0.0
            }
        } else {
            0.0
        };
        let mut peak = first;
        let mut max_dd = 0.0;
        for c in candles {
            if c.close > peak {
                peak = c.close;
            }
            let dd = (peak - c.close) / peak * 100.0;
            if dd > max_dd {
                max_dd = dd;
            }
        }
        Some(BenchmarkResult {
            total_return_pct,
            sharpe_ratio,
            max_drawdown_pct: max_dd,
            final_equity,
        })
    }
}

// ============================================================================
// 双资产配对交易回测
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairsBacktestResult {
    pub total_return_pct: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown_pct: f64,
    pub win_rate: f64,
    pub total_trades: usize,
    pub final_equity: f64,
    pub trades: Vec<PairTrade>,
    pub spread_zscore_mean: f64,
    pub spread_zscore_std: f64,
    pub avg_trade_duration: f64,
    pub profit_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairTrade {
    pub entry_idx: usize,
    pub exit_idx: usize,
    pub entry_spread_z: f64,
    pub exit_spread_z: f64,
    pub direction: String,
    pub pnl: f64,
    pub pnl_pct: f64,
}

pub struct PairsBacktestEngine {
    pub initial_capital: f64,
    pub commission_rate: f64,
    pub slippage: f64,
    pub entry_threshold: f64,
    pub exit_threshold: f64,
    pub stop_loss_threshold: f64,
    pub lookback: usize,
}

impl PairsBacktestEngine {
    pub fn new(initial_capital: f64) -> Self {
        Self {
            initial_capital,
            commission_rate: 0.001,
            slippage: 0.001,
            entry_threshold: 2.0,
            exit_threshold: 0.5,
            stop_loss_threshold: 3.5,
            lookback: 20,
        }
    }

    pub fn run(&self, asset_a: &[Candle], asset_b: &[Candle]) -> PairsBacktestResult {
        let n = asset_a.len().min(asset_b.len());
        if n < self.lookback + 5 {
            return PairsBacktestResult {
                total_return_pct: 0.0,
                sharpe_ratio: 0.0,
                max_drawdown_pct: 0.0,
                win_rate: 0.0,
                total_trades: 0,
                final_equity: self.initial_capital,
                trades: vec![],
                spread_zscore_mean: 0.0,
                spread_zscore_std: 0.0,
                avg_trade_duration: 0.0,
                profit_factor: 0.0,
            };
        }
        let pa: Vec<f64> = asset_a[..n].iter().map(|c| c.close).collect();
        let pb: Vec<f64> = asset_b[..n].iter().map(|c| c.close).collect();
        let hr = Self::compute_hr(&pa, &pb);
        let spread: Vec<f64> = (0..n).map(|i| pa[i].ln() - hr * pb[i].ln()).collect();
        let zscore = Self::compute_zs(&spread, self.lookback);
        let mut equity = self.initial_capital;
        let mut peak = self.initial_capital;
        let mut max_dd = 0.0;
        let mut trades: Vec<PairTrade> = Vec::new();
        let mut equity_curve = vec![self.initial_capital];
        let mut pos: i8 = 0;
        let mut eidx: usize = 0;
        let mut eequity: f64 = 0.0;
        for i in self.lookback..n {
            let z = zscore[i];
            match pos {
                0 => {
                    if z < -self.entry_threshold {
                        pos = 1;
                        eidx = i;
                        eequity = equity;
                    } else if z > self.entry_threshold {
                        pos = -1;
                        eidx = i;
                        eequity = equity;
                    }
                }
                1 => {
                    if z > -self.exit_threshold || z < -self.stop_loss_threshold {
                        let pp = (spread[i] - spread[eidx]) / spread[eidx].abs().max(0.01);
                        let p = eequity * pp * (1.0 - self.commission_rate * 2.0);
                        equity += p;
                        trades.push(PairTrade {
                            entry_idx: eidx,
                            exit_idx: i,
                            entry_spread_z: zscore[eidx],
                            exit_spread_z: z,
                            direction: "long".to_string(),
                            pnl: p,
                            pnl_pct: pp * 100.0,
                        });
                        pos = 0;
                    }
                }
                -1 => {
                    if z < self.exit_threshold || z > self.stop_loss_threshold {
                        let pp = (spread[eidx] - spread[i]) / spread[eidx].abs().max(0.01);
                        let p = eequity * pp * (1.0 - self.commission_rate * 2.0);
                        equity += p;
                        trades.push(PairTrade {
                            entry_idx: eidx,
                            exit_idx: i,
                            entry_spread_z: zscore[eidx],
                            exit_spread_z: z,
                            direction: "short".to_string(),
                            pnl: p,
                            pnl_pct: pp * 100.0,
                        });
                        pos = 0;
                    }
                }
                _ => {}
            }
            equity_curve.push(equity);
            if equity > peak {
                peak = equity;
            }
            let dd = (peak - equity) / peak * 100.0;
            if dd > max_dd {
                max_dd = dd;
            }
        }
        if pos != 0 && n > self.lookback {
            let i = n - 1;
            let z = zscore[i];
            let pp = if pos == 1 {
                (spread[i] - spread[eidx]) / spread[eidx].abs().max(0.01)
            } else {
                (spread[eidx] - spread[i]) / spread[eidx].abs().max(0.01)
            };
            let p = eequity * pp * (1.0 - self.commission_rate * 2.0);
            equity += p;
            trades.push(PairTrade {
                entry_idx: eidx,
                exit_idx: i,
                entry_spread_z: zscore[eidx],
                exit_spread_z: z,
                direction: if pos == 1 { "long" } else { "short" }.to_string(),
                pnl: p,
                pnl_pct: pp * 100.0,
            });
        }
        let tr = (equity - self.initial_capital) / self.initial_capital * 100.0;
        let sr = if equity_curve.len() > 1 {
            let rets: Vec<f64> = equity_curve
                .windows(2)
                .map(|w| (w[1] - w[0]) / w[0])
                .collect();
            let m: f64 = rets.iter().sum::<f64>() / rets.len() as f64;
            let v: f64 = rets.iter().map(|r| (r - m).powi(2)).sum::<f64>() / rets.len() as f64;
            let s = v.sqrt();
            if s > 1e-10 {
                m / s * (252.0_f64).sqrt()
            } else {
                0.0
            }
        } else {
            0.0
        };
        let wins = trades.iter().filter(|t| t.pnl > 0.0).count();
        let wr = if trades.is_empty() {
            0.0
        } else {
            wins as f64 / trades.len() as f64 * 100.0
        };
        let gp: f64 = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum();
        let gl: f64 = trades
            .iter()
            .filter(|t| t.pnl < 0.0)
            .map(|t| t.pnl.abs())
            .sum();
        let pf = if gl > 0.0 {
            gp / gl
        } else if gp > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };
        let ad = if trades.is_empty() {
            0.0
        } else {
            trades
                .iter()
                .map(|t| t.exit_idx - t.entry_idx)
                .sum::<usize>() as f64
                / trades.len() as f64
        };
        let vz: Vec<f64> = zscore[self.lookback..]
            .iter()
            .filter(|z| z.is_finite())
            .copied()
            .collect();
        let zm = if vz.is_empty() {
            0.0
        } else {
            vz.iter().sum::<f64>() / vz.len() as f64
        };
        let zs = if vz.len() > 1 {
            let var = vz.iter().map(|z| (z - zm).powi(2)).sum::<f64>() / (vz.len() - 1) as f64;
            var.sqrt()
        } else {
            0.0
        };
        PairsBacktestResult {
            total_return_pct: tr,
            sharpe_ratio: sr,
            max_drawdown_pct: max_dd,
            win_rate: wr,
            total_trades: trades.len(),
            final_equity: equity,
            trades,
            spread_zscore_mean: zm,
            spread_zscore_std: zs,
            avg_trade_duration: ad,
            profit_factor: pf,
        }
    }

    fn compute_hr(lead: &[f64], lag: &[f64]) -> f64 {
        let n = lead.len().min(lag.len());
        if n < 5 {
            return 1.0;
        }
        let mut sx = 0.0;
        let mut sy = 0.0;
        let mut sxy = 0.0;
        let mut sxx = 0.0;
        for i in 0..n {
            let x = lag[i].ln();
            let y = lead[i].ln();
            sx += x;
            sy += y;
            sxy += x * y;
            sxx += x * x;
        }
        let mx = sx / n as f64;
        let my = sy / n as f64;
        let cov = sxy / n as f64 - mx * my;
        let var = sxx / n as f64 - mx * mx;
        if var.abs() < f64::EPSILON {
            return 1.0;
        }
        cov / var
    }

    fn compute_zs(data: &[f64], lb: usize) -> Vec<f64> {
        let n = data.len();
        let mut zs = vec![0.0; n];
        if n < lb || lb == 0 {
            return zs;
        }
        for i in (lb - 1)..n {
            let w = &data[i.saturating_sub(lb - 1)..=i];
            if w.len() < lb {
                continue;
            }
            let m: f64 = w.iter().sum::<f64>() / lb as f64;
            let v: f64 = w.iter().map(|x| (x - m).powi(2)).sum::<f64>() / lb as f64;
            let s = v.sqrt();
            zs[i] = if s < f64::EPSILON {
                0.0
            } else {
                (data[i] - m) / s
            };
        }
        zs
    }

    pub fn generate_report(&self, r: &PairsBacktestResult) -> String {
        let mut s = String::new();
        s.push_str("# Pairs Trading Backtest Report\n\n## Parameters\n\n");
        s.push_str(&format!(
            "- Entry: {:.1} sigma | Exit: {:.1} sigma | Stop: {:.1} sigma | Lookback: {} bars\n",
            self.entry_threshold, self.exit_threshold, self.stop_loss_threshold, self.lookback
        ));
        s.push_str("\n## Performance\n\n| Metric | Value |\n|--------|-------|\n");
        s.push_str(&format!("| Total Return | {:.2}% |\n| Sharpe | {:.4} |\n| Max Drawdown | {:.2}% |\n| Win Rate | {:.1}% |\n| Profit Factor | {:.2} |\n| Trades | {} |\n| Final Equity | ${:.2} |\n| Spread Z Mean | {:.4} |\n| Spread Z Std | {:.4} |\n| Avg Duration | {:.1} bars |\n", r.total_return_pct, r.sharpe_ratio, r.max_drawdown_pct, r.win_rate, r.profit_factor, r.total_trades, r.final_equity, r.spread_zscore_mean, r.spread_zscore_std, r.avg_trade_duration));
        if !r.trades.is_empty() {
            s.push_str("\n## Trade Details\n\n| # | Dir | EntryZ | ExitZ | PnL% |\n|---|-----|--------|-------|------|\n");
            for (i, t) in r.trades.iter().enumerate() {
                s.push_str(&format!(
                    "| {} | {} | {:.2} | {:.2} | {:.2}% |\n",
                    i + 1,
                    t.direction,
                    t.entry_spread_z,
                    t.exit_spread_z,
                    t.pnl_pct
                ));
            }
        }
        s
    }
}

// ============================================================================
// Walk-Forward Parameter Optimizer
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfOptimizationResult {
    pub best_params: HashMap<String, String>,
    pub best_oos_sharpe: f64,
    pub best_oos_return: f64,
    pub all_candidates: Vec<ParamCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamCandidate {
    pub params: HashMap<String, String>,
    pub oos_sharpe: f64,
    pub oos_return: f64,
    pub train_sharpe: f64,
    pub overfitting_ratio: f64,
}

pub struct WfOptimizer;

impl WfOptimizer {
    pub fn grid_search<F>(
        param_grid: Vec<HashMap<String, f64>>,
        config: &WalkForwardConfig,
        candles: &[Candle],
        backtest_fn: F,
    ) -> WfOptimizationResult
    where
        F: Fn(&[Candle], &HashMap<String, f64>) -> BacktestResult,
    {
        let mut all = Vec::new();
        for params in &param_grid {
            let wf = WalkForwardAnalyzer::analyze(config, candles, |c| backtest_fn(c, params));
            let oos_s = wf.summary.avg_test_sharpe;
            let oos_r = wf.summary.avg_test_return;
            let train_s = {
                let v: Vec<_> = wf
                    .periods
                    .iter()
                    .filter(|p| p.train_result.is_some())
                    .collect();
                if v.is_empty() {
                    0.0
                } else {
                    v.iter()
                        .map(|p| p.train_result.as_ref().unwrap().sharpe_ratio)
                        .sum::<f64>()
                        / v.len() as f64
                }
            };
            let ofr = if train_s.abs() > 1e-10 {
                (train_s - oos_s) / train_s.abs()
            } else {
                0.0
            };
            let mut ps = HashMap::new();
            for (k, v) in params {
                ps.insert(k.clone(), format!("{:.4}", v));
            }
            all.push(ParamCandidate {
                params: ps,
                oos_sharpe: oos_s,
                oos_return: oos_r,
                train_sharpe: train_s,
                overfitting_ratio: ofr,
            });
        }
        all.sort_by(|a, b| {
            b.oos_sharpe
                .partial_cmp(&a.oos_sharpe)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let best = all.first().cloned().unwrap_or(ParamCandidate {
            params: HashMap::new(),
            oos_sharpe: 0.0,
            oos_return: 0.0,
            train_sharpe: 0.0,
            overfitting_ratio: 0.0,
        });
        WfOptimizationResult {
            best_params: best.params.clone(),
            best_oos_sharpe: best.oos_sharpe,
            best_oos_return: best.oos_return,
            all_candidates: all,
        }
    }

    pub fn generate_report(r: &WfOptimizationResult) -> String {
        let mut s = String::new();
        s.push_str("# Walk-Forward Parameter Optimization Report\n\n## Best Parameters\n\n| Parameter | Value |\n|-----------|-------|\n");
        for (k, v) in &r.best_params {
            s.push_str(&format!("| {} | {} |\n", k, v));
        }
        s.push_str(&format!(
            "\n- OOS Sharpe: {:.4}\n- OOS Return: {:.2}%\n",
            r.best_oos_sharpe,
            r.best_oos_return * 100.0
        ));
        s.push_str("\n## All Candidates (sorted by OOS Sharpe)\n\n| # | OOS Sharpe | OOS Return | Train Sharpe | Overfit | Params |\n|---|------------|------------|--------------|---------|--------|\n");
        for (i, c) in r.all_candidates.iter().enumerate() {
            let ps = c
                .params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            s.push_str(&format!(
                "| {} | {:.4} | {:.2}% | {:.4} | {:.2} | {} |\n",
                i + 1,
                c.oos_sharpe,
                c.oos_return * 100.0,
                c.train_sharpe,
                c.overfitting_ratio,
                ps
            ));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading::data::DataSource;
    use crate::trading::strategy::{BollingerBandsStrategy, RsiMeanReversion, SmaCrossover};

    fn test_strats() -> Vec<Box<dyn Strategy>> {
        vec![
            Box::new(SmaCrossover::new(5, 20)),
            Box::new(SmaCrossover::new(10, 30)),
            Box::new(RsiMeanReversion::new(14, 30.0, 70.0)),
            Box::new(BollingerBandsStrategy::new(20, 2.0, 1.0, 1.0, false)),
        ]
    }

    #[test]
    fn test_compare_and_rank() {
        let candles = DataSource::generate_mock(200, 100.0);
        let strats = test_strats();
        let r = StrategyComparator::compare_and_rank(&strats, &candles, 10000.0, 0.001, 0.001);
        assert_eq!(r.entries.len(), 4);
        assert_eq!(r.entries[0].rank, 1);
        assert!(r.entries[0].composite_score >= r.entries[1].composite_score);
        assert!(r.benchmark.is_some());
    }

    #[test]
    fn test_rank_by_sharpe() {
        let candles = DataSource::generate_mock(200, 100.0);
        let r = StrategyComparator::rank_by_sharpe(&test_strats(), &candles, 10000.0, 0.001, 0.001);
        assert_eq!(r.ranking_method, "sharpe_ratio");
        assert!(r.entries[0].sharpe_ratio >= r.entries[1].sharpe_ratio);
    }

    #[test]
    fn test_rank_by_calmar() {
        let candles = DataSource::generate_mock(200, 100.0);
        let r = StrategyComparator::rank_by_calmar(&test_strats(), &candles, 10000.0, 0.001, 0.001);
        assert_eq!(r.ranking_method, "calmar_ratio");
    }

    #[test]
    fn test_comparison_report() {
        let candles = DataSource::generate_mock(200, 100.0);
        let r =
            StrategyComparator::compare_and_rank(&test_strats(), &candles, 10000.0, 0.001, 0.001);
        let report = StrategyComparator::generate_comparison_report(&r);
        assert!(report.contains("Strategy Comparison Report"));
    }

    #[test]
    fn test_pairs_backtest() {
        let a = DataSource::generate_mock(300, 100.0);
        let b = DataSource::generate_mock(300, 80.0);
        let eng = PairsBacktestEngine::new(10000.0);
        let r = eng.run(&a, &b);
        assert!(r.final_equity > 0.0);
        // Report should be non-empty when trades exist, or just verify equity is valid
        assert!(r.final_equity > 0.0);
    }

    #[test]
    fn test_pairs_insufficient_data() {
        let a = DataSource::generate_mock(10, 100.0);
        let b = DataSource::generate_mock(10, 80.0);
        let eng = PairsBacktestEngine::new(10000.0);
        let r = eng.run(&a, &b);
        assert_eq!(r.total_trades, 0);
        assert_eq!(r.final_equity, 10000.0);
    }

    #[test]
    fn test_pairs_report() {
        let a = DataSource::generate_mock(300, 100.0);
        let b = DataSource::generate_mock(300, 80.0);
        let eng = PairsBacktestEngine::new(10000.0);
        let r = eng.run(&a, &b);
        let report = eng.generate_report(&r);
        assert!(report.contains("Pairs Trading"));
    }
}
