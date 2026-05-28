use crate::trading::backtest::{BacktestEngine, Trade, TradeSide};
use crate::trading::data::Candle;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 回测结果报告
pub struct BacktestResult {
    pub engine: BacktestResultData,
    pub trades: Vec<Trade>,
    pub total_bars: usize,
    /// 月度收益分析
    pub monthly_returns: HashMap<String, f64>,
    /// 交易盈亏分布
    pub trade_pnl_distribution: TradePnlDistribution,
}

/// 可序列化的回测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResultData {
    // 基础指标
    pub initial_capital: f64,
    pub final_equity: f64,
    pub total_return_pct: f64,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub max_drawdown: f64,
    pub max_drawdown_pct: f64,
    pub total_pnl: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub profit_factor: f64,
    pub sharpe_ratio: f64,
    pub avg_bars_held: f64,

    // 新增高级指标
    /// Calmar 比率 = 年化收益率 / 最大回撤率
    pub calmar_ratio: f64,
    /// Sortino 比率 (仅考虑下行波动)
    pub sortino_ratio: f64,
    /// 恢复因子 = 总收益 / 最大回撤
    pub recovery_factor: f64,
    /// 期望值 = 平均每笔交易收益
    pub expectancy: f64,
    /// 最大连续亏损次数
    pub max_consecutive_losses: usize,
    /// 最大连续盈利次数
    pub max_consecutive_wins: usize,
    /// 最长持仓周期(bar数)
    pub max_bars_held: usize,
    /// 最短持仓周期(bar数)
    pub min_bars_held: usize,
    /// 多头交易数量
    pub long_trades: usize,
    /// 空头交易数量
    pub short_trades: usize,
    /// 多头胜率
    pub long_win_rate: f64,
    /// 空头胜率
    pub short_win_rate: f64,
    /// 收益标准差
    pub return_std: f64,
    /// 下行标准差 (Sortino)
    pub downside_std: f64,
}

/// 交易盈亏分布
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradePnlDistribution {
    /// 盈利交易数量
    pub wins: usize,
    /// 亏损交易数量
    pub losses: usize,
    /// 最大单笔盈利
    pub max_win: f64,
    /// 最大单笔亏损
    pub max_loss: f64,
    /// 平均盈利
    pub avg_win: f64,
    /// 平均亏损
    pub avg_loss: f64,
    /// 盈亏比 (avg_win / |avg_loss|)
    pub win_loss_ratio: f64,
    /// 盈利交易盈亏分布 (分桶)
    pub win_buckets: Vec<f64>,
    /// 亏损交易盈亏分布 (分桶)
    pub loss_buckets: Vec<f64>,
}

impl BacktestResult {
    pub fn new(engine: &BacktestEngine, candles: &[Candle], trades: &[Trade]) -> Self {
        let total_trades = trades.len();
        let winning_trades = engine.winning_trades;
        let losing_trades = engine.losing_trades;
        let win_rate = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64 * 100.0
        } else {
            0.0
        };

        let total_pnl = engine.total_equity() - engine.initial_capital;

        let (avg_win, avg_loss, profit_factor) = if total_trades > 0 {
            let wins: Vec<f64> = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).collect();
            let losses: Vec<f64> = trades.iter().filter(|t| t.pnl <= 0.0).map(|t| t.pnl).collect();
            let avg_w = if !wins.is_empty() {
                wins.iter().sum::<f64>() / wins.len() as f64
            } else {
                0.0
            };
            let avg_l = if !losses.is_empty() {
                losses.iter().sum::<f64>() / losses.len() as f64
            } else {
                0.0
            };
            let pf = if avg_l.abs() > 0.0 {
                (avg_w * wins.len() as f64) / (avg_l.abs() * losses.len() as f64)
            } else if !wins.is_empty() {
                f64::INFINITY
            } else {
                0.0
            };
            (avg_w, avg_l, pf)
        } else {
            (0.0, 0.0, 0.0)
        };

        // === Sharpe Ratio ===
        let (sharpe_ratio, return_std, downside_std) = if engine.equity_curve.len() > 1 {
            let returns: Vec<f64> = engine
                .equity_curve
                .windows(2)
                .map(|w| (w[1] - w[0]) / w[0])
                .filter(|r| r.is_finite())
                .collect();
            if !returns.is_empty() {
                let mean = returns.iter().sum::<f64>() / returns.len() as f64;
                let variance = returns
                    .iter()
                    .map(|r| (r - mean).powi(2))
                    .sum::<f64>()
                    / returns.len() as f64;
                let std = variance.sqrt();

                // 下行标准差 (仅负收益)
                let downside_variance = returns
                    .iter()
                    .filter(|r| **r < 0.0)
                    .map(|r| (r - mean).powi(2))
                    .sum::<f64>()
                    / returns.len() as f64;
                let d_std = downside_variance.sqrt();

                let sharpe = if std > 0.0 {
                    mean / std * (252.0_f64).sqrt()
                } else {
                    0.0
                };

                let _sortino = if d_std > 0.0 {
                    mean / d_std * (252.0_f64).sqrt()
                } else {
                    0.0
                };

                (sharpe, std, d_std)
            } else {
                (0.0, 0.0, 0.0)
            }
        } else {
            (0.0, 0.0, 0.0)
        };

        let avg_bars_held = if !trades.is_empty() {
            trades.iter().map(|t| t.bars_held as f64).sum::<f64>() / trades.len() as f64
        } else {
            0.0
        };

        // === Calmar Ratio = 年化收益率 / 最大回撤 ===
        let annualized_return = if engine.initial_capital > 0.0 && candles.len() > 1 {
            // 简单年化: 假设日线数据252个交易日
            let periods_per_year = 252.0;
            let total_return = engine.total_return() / 100.0; // 转小数
            let n_periods = candles.len() as f64;
            if n_periods > 0.0 && total_return > -1.0 {
                ((1.0 + total_return).powf(periods_per_year / n_periods) - 1.0) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        let calmar_ratio = if engine.max_drawdown_pct > 0.0 {
            annualized_return / engine.max_drawdown_pct
        } else {
            0.0
        };

        // === Recovery Factor ===
        let recovery_factor = if engine.max_drawdown > 0.0 {
            total_pnl / engine.max_drawdown
        } else if total_pnl > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };

        // === 期望值 ===
        let expectancy = if total_trades > 0 {
            trades.iter().map(|t| t.pnl).sum::<f64>() / total_trades as f64
        } else {
            0.0
        };

        // === 连续盈亏 ===
        let (max_consecutive_wins, max_consecutive_losses) = {
            let mut cur_wins = 0;
            let mut cur_losses = 0;
            let mut max_w = 0;
            let mut max_l = 0;
            for t in trades {
                if t.pnl > 0.0 {
                    cur_wins += 1;
                    cur_losses = 0;
                    max_w = max_w.max(cur_wins);
                } else {
                    cur_losses += 1;
                    cur_wins = 0;
                    max_l = max_l.max(cur_losses);
                }
            }
            (max_w, max_l)
        };

        // === 持仓周期 ===
        let max_bars_held = trades.iter().map(|t| t.bars_held).max().unwrap_or(0);
        let min_bars_held = trades.iter().map(|t| t.bars_held).min().unwrap_or(0);

        // === 多空统计 ===
        let long_trades = trades.iter().filter(|t| matches!(t.side, TradeSide::Long)).count();
        let short_trades = trades.iter().filter(|t| matches!(t.side, TradeSide::Short)).count();
        let long_wins = trades.iter().filter(|t| matches!(t.side, TradeSide::Long) && t.pnl > 0.0).count();
        let short_wins = trades.iter().filter(|t| matches!(t.side, TradeSide::Short) && t.pnl > 0.0).count();

        let long_win_rate = if long_trades > 0 {
            long_wins as f64 / long_trades as f64 * 100.0
        } else {
            0.0
        };
        let short_win_rate = if short_trades > 0 {
            short_wins as f64 / short_trades as f64 * 100.0
        } else {
            0.0
        };

        // === 月度收益分析 ===
        let monthly_returns = compute_monthly_returns(&engine.equity_curve, candles);

        // === 交易盈亏分布 ===
        let trade_pnl_distribution = compute_trade_distribution(trades);

        Self {
            engine: BacktestResultData {
                initial_capital: engine.initial_capital,
                final_equity: engine.total_equity(),
                total_return_pct: engine.total_return(),
                total_trades,
                winning_trades,
                losing_trades,
                win_rate,
                max_drawdown: engine.max_drawdown,
                max_drawdown_pct: engine.max_drawdown_pct,
                total_pnl,
                avg_win,
                avg_loss,
                profit_factor,
                sharpe_ratio,
                avg_bars_held,
                calmar_ratio,
                sortino_ratio: sharpe_ratio * (return_std / downside_std.max(1e-10)),
                recovery_factor,
                expectancy,
                max_consecutive_losses,
                max_consecutive_wins,
                max_bars_held,
                min_bars_held,
                long_trades,
                short_trades,
                long_win_rate,
                short_win_rate,
                return_std,
                downside_std,
            },
            trades: trades.to_vec(),
            total_bars: candles.len(),
            monthly_returns,
            trade_pnl_distribution,
        }
    }

    /// 打印完整报告
    pub fn print_summary(&self) {
        let r = &self.engine;
        let medal = if r.total_return_pct > 0.0 { "✅" } else { "❌" };
        let medal_str = format!("{} {:<4}", medal, "");
        println!("\n═══════════════════════════════════════════");
        println!("  📊 回测绩效报告");
        println!("═══════════════════════════════════════════");
        println!("  初始资金: ${:.2}", r.initial_capital);
        println!("  最终权益: ${:.2}", r.final_equity);
        println!("  {} 总收益率: {:>+8.2}%", medal_str, r.total_return_pct);
        println!("  📈 夏普比率: {:.4}", r.sharpe_ratio);
        println!("  📉 Sortino比率: {:.4}", r.sortino_ratio);
        println!("  🎯 Calmar比率: {:.4}", r.calmar_ratio);
        println!("  🔄 恢复因子: {:.4}", r.recovery_factor);
        println!("  ⚡ 最大回撤: ${:.2} ({:.2}%)", r.max_drawdown, r.max_drawdown_pct);
        println!("  ───────────────────────────────────────");
        println!("  总交易: {}笔 | 胜率: {:.1}%", r.total_trades, r.win_rate);
        println!("  盈利: {}笔 | 亏损: {}笔", r.winning_trades, r.losing_trades);
        println!("  多头: {}笔 (胜率{:.1}%) | 空头: {}笔 (胜率{:.1}%)",
            r.long_trades, r.long_win_rate,
            r.short_trades, r.short_win_rate);
        println!("  平均盈利: ${:.2} | 平均亏损: ${:.2}", r.avg_win, r.avg_loss);
        println!("  盈亏比: {:.2} | 利润因子: {:.2}", 
            if r.avg_loss.abs() > 0.0 { r.avg_win / r.avg_loss.abs() } else { 0.0 },
            r.profit_factor);
        println!("  期望值: ${:.4}", r.expectancy);
        println!("  ───────────────────────────────────────");
        println!("  最长连胜: {} | 最长连败: {}", r.max_consecutive_wins, r.max_consecutive_losses);
        println!("  持仓周期: {:.1}bar (范围: {}-{})", r.avg_bars_held, r.min_bars_held, r.max_bars_held);
        println!("  收益标准差: {:.6} | 下行标准差: {:.6}", r.return_std, r.downside_std);
        println!("═══════════════════════════════════════════");

        // 月度收益
        if !self.monthly_returns.is_empty() {
            println!("\n  📅 月度收益:");
            let mut months: Vec<(&String, &f64)> = self.monthly_returns.iter().collect();
            months.sort_by(|a, b| a.0.cmp(b.0));
            for (month, ret) in months.iter().take(12) {
                let icon = if **ret > 0.0 { "🟢" } else { "🔴" };
                println!("    {} {}: {:>+7.2}%", icon, month, ret);
            }
            if months.len() > 12 {
                println!("    ... 还有 {} 个月份", months.len() - 12);
            }
        }

        // 盈亏分布
        let d = &self.trade_pnl_distribution;
        println!("\n  💰 盈亏分布:");
        println!("    最大盈利: ${:.2} | 最大亏损: ${:.2}", d.max_win, d.max_loss);
        println!("    平均盈利: ${:.2} | 平均亏损: ${:.2}", d.avg_win, d.avg_loss);
        println!("    盈亏比: {:.2}", d.win_loss_ratio);
    }
}

/// 计算月度收益
fn compute_monthly_returns(equity_curve: &[f64], candles: &[Candle]) -> HashMap<String, f64> {
    let mut monthly = HashMap::new();
    if equity_curve.is_empty() || candles.is_empty() {
        return monthly;
    }

    let n = equity_curve.len().min(candles.len());
    let mut last_equity = equity_curve[0];

    for i in 1..n {
        let month_key = candles[i - 1].timestamp.format("%Y-%m").to_string();
        let current_equity = equity_curve[i];
        let monthly_return = if last_equity > 0.0 {
            (current_equity - last_equity) / last_equity * 100.0
        } else {
            0.0
        };

        *monthly.entry(month_key).or_insert(0.0) += monthly_return;
        last_equity = current_equity;
    }

    monthly
}

/// 计算交易盈亏分布
fn compute_trade_distribution(trades: &[Trade]) -> TradePnlDistribution {
    let wins: Vec<f64> = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).collect();
    let losses: Vec<f64> = trades.iter().filter(|t| t.pnl <= 0.0).map(|t| t.pnl).collect();

    let max_win = wins.iter().cloned().fold(0.0_f64, f64::max);
    let max_loss = losses.iter().cloned().fold(0.0_f64, f64::min);
    let avg_win = if !wins.is_empty() {
        wins.iter().sum::<f64>() / wins.len() as f64
    } else {
        0.0
    };
    let avg_loss = if !losses.is_empty() {
        losses.iter().sum::<f64>() / losses.len() as f64
    } else {
        0.0
    };
    let win_loss_ratio = if avg_loss.abs() > 0.0 {
        avg_win / avg_loss.abs()
    } else {
        0.0
    };

    // 分 10 个桶展示
    let win_buckets = bucketize(&wins, 10);
    let loss_buckets = bucketize_t(&losses, 10);

    TradePnlDistribution {
        wins: wins.len(),
        losses: losses.len(),
        max_win,
        max_loss,
        avg_win,
        avg_loss,
        win_loss_ratio,
        win_buckets,
        loss_buckets,
    }
}

fn bucketize(values: &[f64], n_buckets: usize) -> Vec<f64> {
    if values.is_empty() || n_buckets == 0 {
        return vec![];
    }
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let range = (max_val - min_val).max(1.0);
    let bucket_size = range / n_buckets as f64;

    let mut buckets = vec![0.0; n_buckets];
    for v in values {
        let idx = ((v - min_val) / bucket_size).floor() as usize;
        let idx = idx.min(n_buckets - 1);
        buckets[idx] += 1.0;
    }
    buckets
}

fn bucketize_t(values: &[f64], n_buckets: usize) -> Vec<f64> {
    if values.is_empty() || n_buckets == 0 {
        return vec![];
    }
    let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = (max_val - min_val).max(1.0);
    let bucket_size = range / n_buckets as f64;

    let mut buckets = vec![0.0; n_buckets];
    for v in values {
        let idx = ((v - min_val) / bucket_size).floor() as usize;
        let idx = idx.min(n_buckets - 1);
        buckets[idx] += 1.0;
    }
    buckets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading::backtest::BacktestEngine;
    use crate::trading::data::DataSource;
    use crate::trading::strategy::SmaCrossover;

    #[test]
    fn test_backtest_report() {
        let candles = DataSource::generate_mock(200, 100.0);
        let strategy = SmaCrossover::new(5, 20);
        let mut engine = BacktestEngine::new(10_000.0, 0.001, 0.001);
        let trades = engine.run(&candles, &strategy).unwrap();
        let report = BacktestResult::new(&engine, &candles, &trades);
        assert!(report.engine.total_return_pct > -100.0);
        assert!(report.engine.sharpe_ratio > -10.0);
        assert!(report.engine.calmar_ratio > -10.0);
        assert!(report.engine.sortino_ratio > -10.0);
    }

    #[test]
    fn test_trade_distribution() {
        let trades = vec![
            Trade {
                entry_time: "2024-01-01".into(),
                exit_time: "2024-01-02".into(),
                side: TradeSide::Long,
                entry_price: 100.0,
                exit_price: 110.0,
                quantity: 1.0,
                pnl: 10.0,
                pnl_percent: 10.0,
                bars_held: 1,
                exit_reason: "test".into(),
            },
            Trade {
                entry_time: "2024-01-03".into(),
                exit_time: "2024-01-04".into(),
                side: TradeSide::Short,
                entry_price: 110.0,
                exit_price: 100.0,
                quantity: 1.0,
                pnl: -5.0,
                pnl_percent: -5.0,
                bars_held: 1,
                exit_reason: "test".into(),
            },
        ];
        let dist = compute_trade_distribution(&trades);
        assert_eq!(dist.wins, 1);
        assert_eq!(dist.losses, 1);
        assert!((dist.max_win - 10.0).abs() < 1e-10);
        assert!((dist.max_loss - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn test_monthly_returns() {
        let candles = DataSource::generate_mock(100, 100.0);
        let equity_curve: Vec<f64> = (0..=100).map(|i| 1000.0 + i as f64 * 10.0).collect();
        let monthly = compute_monthly_returns(&equity_curve, &candles);
        // Should have some monthly data
        assert!(monthly.len() >= 1);
    }
}
