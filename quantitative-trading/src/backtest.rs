use crate::data::Candle;
use crate::strategy::{Signal, Strategy};

/// 交易方向
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

use serde::{Deserialize, Serialize};

/// 成交记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub entry_time: String,
    pub exit_time: String,
    pub side: TradeSide,
    pub entry_price: f64,
    pub exit_price: f64,
    pub quantity: f64,
    pub pnl: f64,
    pub pnl_percent: f64,
    pub bars_held: usize,
    pub exit_reason: String,
}

/// 回测引擎
pub struct BacktestEngine {
    pub initial_capital: f64,
    pub current_capital: f64,
    pub peak_capital: f64,
    pub position: f64,           // 当前持仓数量
    pub position_value: f64,     // 持仓市值
    pub commission_rate: f64,    // 手续费率
    pub slippage: f64,           // 滑点 (百分比)
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub equity_curve: Vec<f64>,
    pub max_drawdown: f64,
    pub max_drawdown_pct: f64,
}

impl BacktestEngine {
    pub fn new(initial_capital: f64, commission_rate: f64, slippage: f64) -> Self {
        Self {
            initial_capital,
            current_capital: initial_capital,
            peak_capital: initial_capital,
            position: 0.0,
            position_value: 0.0,
            commission_rate,
            slippage,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            equity_curve: vec![initial_capital],
            max_drawdown: 0.0,
            max_drawdown_pct: 0.0,
        }
    }

    /// 运行回测
    pub fn run(&mut self, candles: &[Candle], strategy: &dyn Strategy) -> anyhow::Result<Vec<Trade>> {
        let signals = strategy.generate(candles);
        let mut trades: Vec<Trade> = Vec::new();
        let mut current_trade: Option<(usize, f64, f64, TradeSide)> = None; // (entry_index, entry_price, quantity, side)

        println!(
            "   📈 策略: {} | 初始资金: ${:.2} | 手续费: {:.1}% | 滑点: {:.1}%",
            strategy.name(),
            self.initial_capital,
            self.commission_rate * 100.0,
            self.slippage * 100.0
        );

        for i in 0..candles.len() {
            let signal = signals[i];
            let price = candles[i].close;

            match signal {
                Signal::Buy => {
                    // 如果当前没有持仓，开多头仓位
                    if current_trade.is_none() && self.position == 0.0 {
                        let buy_price = price * (1.0 + self.slippage);
                        let after_commission = self.current_capital * (1.0 - self.commission_rate);
                        let quantity = after_commission / buy_price;
                        let cost = quantity * buy_price;
                        let commission = cost * self.commission_rate;

                        self.position = quantity;
                        self.position_value = cost;
                        self.current_capital -= cost + commission;

                        current_trade = Some((i, buy_price, quantity, TradeSide::Buy));
                        self.total_trades += 1;
                    }
                }
                Signal::Sell => {
                    // 如果有持仓，平仓
                    if let Some((entry_idx, entry_price, qty, side)) = current_trade.take() {
                        let sell_price = price * (1.0 - self.slippage);
                        let revenue = qty * sell_price;
                        let commission = revenue * self.commission_rate;
                        let pnl = revenue - commission - (qty * entry_price);

                        self.current_capital += revenue - commission;
                        self.position = 0.0;
                        self.position_value = 0.0;

                        let pnl_pct = if entry_price > 0.0 {
                            (sell_price - entry_price) / entry_price * 100.0
                        } else {
                            0.0
                        };

                        if pnl > 0.0 {
                            self.winning_trades += 1;
                        } else {
                            self.losing_trades += 1;
                        }

                        trades.push(Trade {
                            entry_time: candles[entry_idx].timestamp.to_rfc3339(),
                            exit_time: candles[i].timestamp.to_rfc3339(),
                            side,
                            entry_price,
                            exit_price: sell_price,
                            quantity: qty,
                            pnl,
                            pnl_percent: pnl_pct,
                            bars_held: i - entry_idx,
                            exit_reason: "Signal Sell".to_string(),
                        });
                    }
                }
                Signal::Hold => {
                    // 更新持仓市值
                    if self.position > 0.0 {
                        self.position_value = self.position * price;
                    }
                }
            }

            // 更新权益曲线
            let total_equity = self.current_capital + self.position_value;
            self.equity_curve.push(total_equity);

            // 更新最大回撤
            if total_equity > self.peak_capital {
                self.peak_capital = total_equity;
            }
            let drawdown = self.peak_capital - total_equity;
            if drawdown > self.max_drawdown {
                self.max_drawdown = drawdown;
                self.max_drawdown_pct = drawdown / self.peak_capital * 100.0;
            }
        }

        // 如果最后还有持仓，强制平仓
        if let Some((entry_idx, entry_price, qty, side)) = current_trade.take() {
            let last_idx = candles.len() - 1;
            let last_price = candles[last_idx].close;
            let sell_price = last_price * (1.0 - self.slippage);
            let revenue = qty * sell_price;
            let commission = revenue * self.commission_rate;
            let pnl = revenue - commission - (qty * entry_price);

            self.current_capital += revenue - commission;
            self.position = 0.0;
            self.position_value = 0.0;

            let pnl_pct = (sell_price - entry_price) / entry_price * 100.0;

            if pnl > 0.0 {
                self.winning_trades += 1;
            } else {
                self.losing_trades += 1;
            }

            trades.push(Trade {
                entry_time: candles[entry_idx].timestamp.to_rfc3339(),
                exit_time: candles[last_idx].timestamp.to_rfc3339(),
                side,
                entry_price,
                exit_price: sell_price,
                quantity: qty,
                pnl,
                pnl_percent: pnl_pct,
                bars_held: last_idx - entry_idx,
                exit_reason: "End of Data".to_string(),
            });
        }

        Ok(trades)
    }

    /// 获取总权益
    pub fn total_equity(&self) -> f64 {
        self.current_capital + self.position_value
    }

    /// 获取收益率
    pub fn total_return(&self) -> f64 {
        (self.total_equity() - self.initial_capital) / self.initial_capital * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::DataSource;
    use crate::strategy::SmaCrossover;

    #[test]
    fn test_backtest_runs() {
        let candles = DataSource::generate_mock(200, 100.0);
        let strategy = SmaCrossover::new(5, 20);
        let mut engine = BacktestEngine::new(10_000.0, 0.001, 0.001);
        let trades = engine.run(&candles, &strategy).unwrap();
        assert!(engine.total_equity() > 0.0);
        println!(
            "回测完成: {} 笔交易, 最终权益 ${:.2}",
            trades.len(),
            engine.total_equity()
        );
    }
}
