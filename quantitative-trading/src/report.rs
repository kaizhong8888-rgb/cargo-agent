use crate::backtest::{BacktestEngine, Trade, TradeSide};
use crate::data::Candle;
use serde::{Deserialize, Serialize};

/// 回测结果报告
pub struct BacktestResult {
    pub engine: BacktestResultData,
    #[allow(dead_code)]
    pub trades: Vec<Trade>,
    #[allow(dead_code)]
    pub total_bars: usize,
}

/// 可序列化的回测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResultData {
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

        // 夏普比率（简化计算：使用每日收益）
        let sharpe_ratio = if engine.equity_curve.len() > 1 {
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
                if std > 0.0 {
                    mean / std * (252.0_f64).sqrt() // 年化（假设日数据）
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0
        };

        let avg_bars_held = if !trades.is_empty() {
            trades.iter().map(|t| t.bars_held as f64).sum::<f64>() / trades.len() as f64
        } else {
            0.0
        };

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
            },
            trades: trades.to_vec(),
            total_bars: candles.len(),
        }
    }

    /// 打印精简摘要（用于多策略对比）
    pub fn print_summary(&self) {
        let r = &self.engine;
        let medal = if r.total_return_pct > 0.0 { "✅" } else { "❌" };
        println!(
            "   {:<4} 收益率: {:>+8.2}% {} | 夏普: {:>7.4} | 回撤: {:>6.2}% | 交易: {:>4}笔 | 胜率: {:>5.1}%",
            medal,
            r.total_return_pct,
            if r.total_return_pct > 10.0 {
                "🔥"
            } else if r.total_return_pct > 0.0 {
                "📈"
            } else {
                "📉"
            },
            r.sharpe_ratio,
            r.max_drawdown_pct,
            r.total_trades,
            r.win_rate,
        );
    }

    /// 打印完整回测报告
    #[allow(dead_code)]
    pub fn print(&self) {
        let r = &self.engine;
        println!("{}", "=".repeat(60));
        println!("📋 回测结果报告");
        println!("{}", "=".repeat(60));

        println!("\n📊 基础信息:");
        println!("   📅 回测周期: {} 根 K 线", self.total_bars);
        println!("   💰 初始资金: ${:.2}", r.initial_capital);
        println!("   💰 最终权益: ${:.2}", r.final_equity);

        println!("\n📈 收益指标:");
        println!(
            "   🔼 总收益率:   {:>+8.2}% {}",
            r.total_return_pct,
            if r.total_return_pct > 0.0 { "✅" } else { "❌" }
        );
        println!("   💵 总盈亏:     ${:>+8.2}", r.total_pnl);
        println!("   📊 夏普比率:   {:>8.4}", r.sharpe_ratio);
        println!(
            "   📉 最大回撤:   ${:.2} ({:.2}%)",
            r.max_drawdown, r.max_drawdown_pct
        );

        println!("\n🎯 交易统计:");
        println!("   🔄 总交易数:   {}", r.total_trades);
        println!("   ✅ 盈利交易:   {} ({:.1}%)", r.winning_trades, r.win_rate);
        println!("   ❌ 亏损交易:   {}", r.losing_trades);
        println!("   💹 盈亏比:     {:.2}", r.profit_factor);
        println!("   📈 平均盈利:   ${:+.2}", r.avg_win);
        println!("   📉 平均亏损:   ${:+.2}", r.avg_loss);
        println!("   ⏱  平均持仓:   {:.1} 根 K 线", r.avg_bars_held);

        println!("\n📋 最近交易记录 (最多10笔):");
        println!(
            "  {:<4} {:<8} {:<10} {:<10} {:<10} {:<10}",
            "#", "方向", "入场价", "出场价", "盈亏($)", "盈亏(%)"
        );
        println!("  {}", "-".repeat(56));

        let recent_trades = if self.trades.len() > 10 {
            &self.trades[self.trades.len() - 10..]
        } else {
            &self.trades[..]
        };

        for (j, trade) in recent_trades.iter().enumerate() {
            let side_str = match trade.side {
                TradeSide::Buy => "做多",
                TradeSide::Sell => "做空",
            };
            println!(
                "  {:<4} {:<8} ${:<8.2} ${:<8.2} ${:<+8.2} {:>+7.2}%",
                j + 1,
                side_str,
                trade.entry_price,
                trade.exit_price,
                trade.pnl,
                trade.pnl_percent
            );
        }

        println!("{}", "=".repeat(60));
    }

    /// 导出交易记录到 JSON 文件
    #[allow(dead_code)]
    pub fn export_trades(&self, path: &str) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(&self.engine)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}
