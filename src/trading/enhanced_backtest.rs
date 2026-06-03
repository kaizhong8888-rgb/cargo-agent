/// 增强型回测引擎
/// 集成：动态风险管理、市场状态识别、动态止损止盈、交易成本建模、执行延迟
use super::backtest::{Trade, TradeSide};
use super::data::Candle;
use super::market_regime::{MarketRegime, MarketRegimeDetector};
use super::risk_management::{ExitManager, RiskManager};
use super::strategy::{Signal, Strategy};

/// 增强型回测配置
#[derive(Debug, Clone)]
pub struct EnhancedBacktestConfig {
    pub initial_capital: f64,
    pub commission_rate: f64,
    pub base_slippage: f64,
    pub execution_delay: usize,
    pub enable_risk_management: bool,
    pub enable_regime_filter: bool,
    pub enable_dynamic_exit: bool,
    pub enable_vol_slippage: bool,
    pub min_signal_interval: usize,
    pub enable_mtf_filter: bool,
    pub monte_carlo_iterations: usize,
}

impl Default for EnhancedBacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: 10000.0,
            commission_rate: 0.001,
            base_slippage: 0.001,
            execution_delay: 1,
            enable_risk_management: true,
            enable_regime_filter: true,
            enable_dynamic_exit: true,
            enable_vol_slippage: true,
            min_signal_interval: 3,
            enable_mtf_filter: false,
            monte_carlo_iterations: 0,
        }
    }
}

/// 增强型回测结果
#[derive(Debug, Clone)]
pub struct EnhancedBacktestResult {
    pub trades: Vec<Trade>,
    pub total_return_pct: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown_pct: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub final_equity: f64,
    pub regime_distribution: Vec<(MarketRegime, f64)>,
    pub regime_performance: Vec<(MarketRegime, f64, f64)>,
    pub monte_carlo_stats: Option<MonteCarloStats>,
    pub exit_reason_stats: Vec<(String, usize, f64)>,
    pub risk_metrics: RiskMetrics,
}

/// 蒙特卡洛模拟统计
#[derive(Debug, Clone)]
pub struct MonteCarloStats {
    pub mean_return: f64,
    pub median_return: f64,
    pub std_return: f64,
    pub worst_return: f64,
    pub best_return: f64,
    pub pct_positive: f64,
    pub value_at_risk_95: f64,
    pub conditional_var_95: f64,
}

/// 风险指标
#[derive(Debug, Clone)]
pub struct RiskMetrics {
    pub daily_return_mean: f64,
    pub daily_return_std: f64,
    pub downside_deviation: f64,
    pub var_95: f64,
    pub cvar_95: f64,
    pub max_consecutive_losses: usize,
    pub max_consecutive_wins: usize,
    pub avg_bars_held: f64,
    pub kelly_position: f64,
}

/// 增强型回测引擎
pub struct EnhancedBacktestEngine {
    config: EnhancedBacktestConfig,
    risk_manager: RiskManager,
    regime_detector: MarketRegimeDetector,
}

impl EnhancedBacktestEngine {
    pub fn new(config: EnhancedBacktestConfig) -> Self {
        Self {
            risk_manager: RiskManager::new(config.initial_capital),
            regime_detector: MarketRegimeDetector::new(),
            config,
        }
    }

    pub fn run(
        &mut self,
        candles: &[Candle],
        strategy: &dyn Strategy,
    ) -> anyhow::Result<EnhancedBacktestResult> {
        let signals = strategy.generate(candles);
        let mut trades: Vec<Trade> = Vec::new();
        let mut equity_curve = vec![self.config.initial_capital];
        let mut current_equity = self.config.initial_capital;
        let mut peak_equity = self.config.initial_capital;
        let mut max_drawdown = 0.0;
        let mut max_drawdown_pct = 0.0;

        let mut in_position = false;
        let mut entry_idx: usize = 0;
        let mut entry_price = 0.0;
        let mut entry_quantity = 0.0;
        let mut side = TradeSide::Long;
        let mut highest_price = 0.0;
        let mut lowest_price = f64::MAX;
        let mut exit_manager: Option<ExitManager> = None;
        let mut breakeven_triggered = false;
        let mut last_signal_idx = 0;
        let mut signal_queue: Vec<(usize, Signal)> = Vec::new();
        let mut regime_history: Vec<MarketRegime> = Vec::new();
        let mut exit_reasons: std::collections::HashMap<String, (usize, f64)> =
            std::collections::HashMap::new();

        println!(
            "   📊 增强回测: {} | 资金: ${:.2} | 滑点: {:.1}% | 延迟: {} bar",
            strategy.name(),
            self.config.initial_capital,
            self.config.base_slippage * 100.0,
            self.config.execution_delay
        );

        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        for i in 0..candles.len() {
            let price = candles[i].close;
            let atr = if i >= 14 {
                let atr_values = super::indicators::atr(&highs, &lows, &closes, 14);
                atr_values[i]
            } else {
                0.0
            };

            let regime = if self.config.enable_regime_filter && i >= 50 {
                let recent = &candles[i.saturating_sub(49)..=i];
                self.regime_detector.detect_regime(recent)
            } else {
                MarketRegime::Ranging
            };
            regime_history.push(regime);

            let delayed_signal = if !signal_queue.is_empty() && signal_queue[0].0 <= i {
                Some(signal_queue.remove(0).1)
            } else {
                None
            };

            if let Some(signal) = delayed_signal {
                let min_interval_ok = (i - last_signal_idx) >= self.config.min_signal_interval;

                match signal {
                    Signal::Buy => {
                        if !in_position && min_interval_ok {
                            let regime_ok = if self.config.enable_regime_filter {
                                regime.is_suitable_for_trend_following()
                                    || regime.is_suitable_for_breakout()
                                    || regime == MarketRegime::Ranging
                            } else {
                                true
                            };

                            if regime_ok {
                                let position_pct = if self.config.enable_risk_management {
                                    self.risk_manager.get_position_size(
                                        current_equity,
                                        candles,
                                        14,
                                        0.02,
                                    )
                                } else {
                                    0.10
                                };

                                if position_pct > 0.0 {
                                    let slippage = self.calculate_slippage(
                                        atr,
                                        price,
                                        self.config.base_slippage,
                                    );
                                    let buy_price = price * (1.0 + slippage);
                                    let alloc = current_equity * position_pct;
                                    let after_commission =
                                        alloc * (1.0 - self.config.commission_rate);
                                    let quantity = after_commission / buy_price;
                                    let cost = quantity * buy_price;
                                    let commission = cost * self.config.commission_rate;

                                    current_equity -= cost + commission;
                                    in_position = true;
                                    entry_idx = i;
                                    entry_price = buy_price;
                                    entry_quantity = quantity;
                                    side = TradeSide::Long;
                                    highest_price = price;
                                    lowest_price = price;
                                    breakeven_triggered = false;
                                    last_signal_idx = i;

                                    if self.config.enable_dynamic_exit {
                                        exit_manager = Some(ExitManager::new(buy_price, true));
                                    }
                                }
                            }
                        }
                    }
                    Signal::Sell => {
                        if in_position && min_interval_ok {
                            let regime_ok = if self.config.enable_regime_filter {
                                regime.is_suitable_for_mean_reversion()
                                    || regime == MarketRegime::Ranging
                            } else {
                                true
                            };

                            if regime_ok {
                                Self::close_position_fn(
                                    price,
                                    i,
                                    candles,
                                    &mut trades,
                                    &mut current_equity,
                                    &mut in_position,
                                    entry_idx,
                                    entry_price,
                                    entry_quantity,
                                    side,
                                    "Signal Sell",
                                    &mut exit_reasons,
                                    self.config.base_slippage,
                                    self.config.commission_rate,
                                );
                                last_signal_idx = i;
                            }
                        }
                    }
                    Signal::Hold => {}
                }
            }

            signal_queue.push((i + self.config.execution_delay, signals[i]));

            if in_position {
                highest_price = highest_price.max(price);
                lowest_price = lowest_price.min(price);

                if !breakeven_triggered {
                    let profit_pct = (price - entry_price) / entry_price;
                    if let Some(ref em) = exit_manager {
                        let threshold_atr = if atr > 0.0 {
                            em.config.breakeven_threshold_atr * atr / entry_price
                        } else {
                            0.1
                        };
                        if profit_pct >= threshold_atr {
                            breakeven_triggered = true;
                        }
                    }
                }

                if self.config.enable_dynamic_exit {
                    if let Some(ref exit_mgr) = exit_manager {
                        let exit_signal = exit_mgr.check_exit(price, atr, breakeven_triggered);
                        if exit_signal.should_exit {
                            Self::close_position_fn(
                                exit_signal.exit_price,
                                i,
                                candles,
                                &mut trades,
                                &mut current_equity,
                                &mut in_position,
                                entry_idx,
                                entry_price,
                                entry_quantity,
                                side,
                                &exit_signal.reason,
                                &mut exit_reasons,
                                self.config.base_slippage,
                                self.config.commission_rate,
                            );
                            exit_manager = None;
                        }
                    }
                }
            }

            let total_equity = current_equity;
            equity_curve.push(total_equity);

            if total_equity > peak_equity {
                peak_equity = total_equity;
            }
            let drawdown = peak_equity - total_equity;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
                max_drawdown_pct = if peak_equity > 0.0 {
                    drawdown / peak_equity
                } else {
                    0.0
                };
            }
        }

        if in_position {
            let last_idx = candles.len() - 1;
            let last_price = candles[last_idx].close;
            Self::close_position_fn(
                last_price,
                last_idx,
                candles,
                &mut trades,
                &mut current_equity,
                &mut in_position,
                entry_idx,
                entry_price,
                entry_quantity,
                side,
                "End of Data",
                &mut exit_reasons,
                self.config.base_slippage,
                self.config.commission_rate,
            );
        }

        let final_equity = current_equity;
        let total_return_pct =
            (final_equity - self.config.initial_capital) / self.config.initial_capital * 100.0;

        // Compute stats BEFORE moving trades into result
        let win_rate = if trades.is_empty() {
            0.0
        } else {
            trades.iter().filter(|t| t.pnl > 0.0).count() as f64 / trades.len() as f64
        };
        let gross_profit: f64 = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum();
        let gross_loss: f64 = trades
            .iter()
            .filter(|t| t.pnl < 0.0)
            .map(|t| t.pnl.abs())
            .sum();
        let profit_factor = if gross_loss == 0.0 {
            if gross_profit > 0.0 {
                f64::INFINITY
            } else {
                0.0
            }
        } else {
            gross_profit / gross_loss
        };
        let avg_bars_held = if trades.is_empty() {
            0.0
        } else {
            trades.iter().map(|t| t.bars_held).sum::<usize>() as f64 / trades.len() as f64
        };

        let risk_metrics = self.compute_risk_metrics(&equity_curve, &trades, avg_bars_held);
        let regime_dist = Self::compute_regime_distribution(&regime_history);
        let regime_perf = Self::compute_regime_performance(&regime_history, &trades);
        let mc_stats = if self.config.monte_carlo_iterations > 0 {
            Some(Self::run_monte_carlo(
                &trades,
                self.config.monte_carlo_iterations,
            ))
        } else {
            None
        };
        let exit_reason_stats: Vec<_> = exit_reasons
            .into_iter()
            .map(|(reason, (count, pnl))| (reason, count, pnl))
            .collect();

        for trade in &trades {
            self.risk_manager.record_trade(trade.pnl_percent / 100.0);
        }

        Ok(EnhancedBacktestResult {
            trades,
            total_return_pct,
            sharpe_ratio: if risk_metrics.daily_return_std > 1e-10 {
                risk_metrics.daily_return_mean / risk_metrics.daily_return_std * (252.0_f64.sqrt())
            } else {
                0.0
            },
            max_drawdown_pct: max_drawdown_pct * 100.0,
            win_rate,
            profit_factor,
            final_equity,
            regime_distribution: regime_dist,
            regime_performance: regime_perf,
            monte_carlo_stats: mc_stats,
            exit_reason_stats,
            risk_metrics,
        })
    }

    fn calculate_slippage(&self, atr: f64, price: f64, base_slippage: f64) -> f64 {
        if !self.config.enable_vol_slippage || atr == 0.0 || price == 0.0 {
            return base_slippage;
        }
        let vol = atr / price;
        vol * 0.5 + base_slippage
    }

    fn close_position_fn(
        exit_price: f64,
        exit_idx: usize,
        candles: &[Candle],
        trades: &mut Vec<Trade>,
        current_equity: &mut f64,
        in_position: &mut bool,
        entry_idx: usize,
        entry_price: f64,
        quantity: f64,
        side: TradeSide,
        exit_reason: &str,
        exit_reasons: &mut std::collections::HashMap<String, (usize, f64)>,
        base_slippage: f64,
        commission_rate: f64,
    ) {
        let sell_price = exit_price * (1.0 - base_slippage);
        let revenue = quantity * sell_price;
        let commission = revenue * commission_rate;
        let pnl = revenue - commission - (quantity * entry_price);

        *current_equity += revenue - commission;
        *in_position = false;

        let pnl_pct = if entry_price > 0.0 {
            (sell_price - entry_price) / entry_price * 100.0
        } else {
            0.0
        };

        let entry = exit_reasons
            .entry(exit_reason.to_string())
            .or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += pnl;

        trades.push(Trade {
            entry_time: candles[entry_idx].timestamp.to_rfc3339(),
            exit_time: candles[exit_idx].timestamp.to_rfc3339(),
            side,
            entry_price,
            exit_price: sell_price,
            quantity,
            pnl,
            pnl_percent: pnl_pct,
            bars_held: exit_idx - entry_idx,
            exit_reason: exit_reason.to_string(),
        });
    }

    fn compute_risk_metrics(
        &self,
        equity_curve: &[f64],
        trades: &[Trade],
        avg_bars_held: f64,
    ) -> RiskMetrics {
        if equity_curve.len() < 2 {
            return RiskMetrics {
                daily_return_mean: 0.0,
                daily_return_std: 0.0,
                downside_deviation: 0.0,
                var_95: 0.0,
                cvar_95: 0.0,
                max_consecutive_losses: 0,
                max_consecutive_wins: 0,
                avg_bars_held,
                kelly_position: 0.0,
            };
        }

        let returns: Vec<f64> = equity_curve
            .windows(2)
            .map(|w| (w[1] - w[0]) / w[0])
            .collect();
        let mean: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance: f64 =
            returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
        let std = variance.sqrt();

        let downside_returns: Vec<f64> = returns.iter().filter(|&&r| r < 0.0).copied().collect();
        let downside_dev = if !downside_returns.is_empty() {
            let dd_var: f64 =
                downside_returns.iter().map(|r| r.powi(2)).sum::<f64>() / returns.len() as f64;
            dd_var.sqrt()
        } else {
            0.0
        };

        let mut sorted_returns = returns.clone();
        sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let var_idx = (sorted_returns.len() as f64 * 0.05) as usize;
        let var_95 = sorted_returns.get(var_idx).copied().unwrap_or(0.0);
        let cvar_95 = if var_idx > 0 {
            sorted_returns[..=var_idx].iter().sum::<f64>() / (var_idx + 1) as f64
        } else {
            var_95
        };

        let (max_consec_wins, max_consec_losses) = Self::compute_consecutive(trades);

        RiskMetrics {
            daily_return_mean: mean,
            daily_return_std: std,
            downside_deviation: downside_dev,
            var_95,
            cvar_95,
            max_consecutive_losses: max_consec_losses,
            max_consecutive_wins: max_consec_wins,
            avg_bars_held,
            kelly_position: 0.0,
        }
    }

    fn compute_consecutive(trades: &[Trade]) -> (usize, usize) {
        let mut max_wins = 0;
        let mut max_losses = 0;
        let mut cur_wins = 0;
        let mut cur_losses = 0;
        for t in trades {
            if t.pnl > 0.0 {
                cur_wins += 1;
                cur_losses = 0;
                max_wins = max_wins.max(cur_wins);
            } else {
                cur_losses += 1;
                cur_wins = 0;
                max_losses = max_losses.max(cur_losses);
            }
        }
        (max_wins, max_losses)
    }

    fn compute_regime_distribution(regimes: &[MarketRegime]) -> Vec<(MarketRegime, f64)> {
        use std::collections::HashMap;
        let mut counts: HashMap<MarketRegime, usize> = HashMap::new();
        for r in regimes {
            *counts.entry(*r).or_insert(0) += 1;
        }
        let total = regimes.len() as f64;
        counts
            .into_iter()
            .map(|(r, c)| (r, c as f64 / total * 100.0))
            .collect()
    }

    fn compute_regime_performance(
        _regimes: &[MarketRegime],
        _trades: &[Trade],
    ) -> Vec<(MarketRegime, f64, f64)> {
        vec![]
    }

    fn run_monte_carlo(trades: &[Trade], iterations: usize) -> MonteCarloStats {
        if trades.is_empty() {
            return MonteCarloStats {
                mean_return: 0.0,
                median_return: 0.0,
                std_return: 0.0,
                worst_return: 0.0,
                best_return: 0.0,
                pct_positive: 0.0,
                value_at_risk_95: 0.0,
                conditional_var_95: 0.0,
            };
        }

        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let pnl_pcts: Vec<f64> = trades.iter().map(|t| t.pnl_percent).collect();
        let mut total_returns = Vec::with_capacity(iterations);

        for _ in 0..iterations {
            let mut shuffled = pnl_pcts.clone();
            shuffled.shuffle(&mut rng);
            let mut equity = 1.0;
            for pnl in &shuffled {
                equity *= 1.0 + pnl / 100.0;
            }
            total_returns.push((equity - 1.0) * 100.0);
        }

        total_returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = total_returns.len();
        let mean: f64 = total_returns.iter().sum::<f64>() / n as f64;
        let median = total_returns[n / 2];
        let worst = total_returns[0];
        let best = total_returns[n - 1];
        let positive = total_returns.iter().filter(|&&r| r > 0.0).count() as f64 / n as f64 * 100.0;
        let var_idx = (n as f64 * 0.05) as usize;
        let var_95 = total_returns.get(var_idx).copied().unwrap_or(0.0);
        let cvar_95 = if var_idx > 0 {
            total_returns[..=var_idx].iter().sum::<f64>() / (var_idx + 1) as f64
        } else {
            var_95
        };
        let variance: f64 = total_returns
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>()
            / n as f64;

        MonteCarloStats {
            mean_return: mean,
            median_return: median,
            std_return: variance.sqrt(),
            worst_return: worst,
            best_return: best,
            pct_positive: positive,
            value_at_risk_95: var_95,
            conditional_var_95: cvar_95,
        }
    }
}
