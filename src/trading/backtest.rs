use crate::trading::data::Candle;
use crate::trading::strategy::{Signal, Strategy};
use serde::{Deserialize, Serialize};

/// 交易方向
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TradeSide {
    Long,  // 做多
    Short, // 做空
}

/// 仓位管理模式
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PositionSizing {
    /// 固定比例仓位 (0.0~1.0 之间，例如 0.25 = 25%资金)
    FixedFractional(f64),
    /// 凯利公式仓位管理
    Kelly(f64), // Kelly 系数 (0~1 缩放, 0.25 表示 1/4 Kelly)
    /// 固定数量 (正数表示做多数量，负数表示做空数量)
    FixedQuantity(f64),
}

/// 止损模式
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum StopLoss {
    /// 固定百分比止损 (0.05 = 5%)
    FixedPercent(f64),
    /// ATR 跟踪止损 (倍数 * ATR)
    AtrTrailing(f64),
    /// 固定金额止损
    FixedAmount(f64),
    /// 无止损
    None,
}

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
    pub position: f64, // 正数=多头数量, 负数=空头数量
    pub position_value: f64,
    pub avg_entry_price: f64,
    pub entry_candle_idx: usize, // 入场蜡烛索引（用于计算 bars_held）
    pub commission_rate: f64,
    pub slippage: f64,
    pub total_long_trades: usize,
    pub total_short_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub equity_curve: Vec<f64>,
    pub max_drawdown: f64,
    pub max_drawdown_pct: f64,
    pub position_sizing: PositionSizing,
    pub stop_loss: StopLoss,
    pub trailing_stop_activated: bool,
    pub trailing_stop_price: f64,
    pub max_equity_drawdown_pct: f64, // 最大回撤止损百分比
    pub trades: Vec<Trade>,
    /// 做空保证金比例（如0.5表示50%保证金）
    pub short_margin_requirement: f64,
}

impl BacktestEngine {
    /// 创建新的回测引擎
    pub fn new(initial_capital: f64, commission_rate: f64, slippage: f64) -> Self {
        Self {
            initial_capital,
            current_capital: initial_capital,
            peak_capital: initial_capital,
            position: 0.0,
            position_value: 0.0,
            avg_entry_price: 0.0,
            entry_candle_idx: 0,
            commission_rate,
            slippage,
            total_long_trades: 0,
            total_short_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            equity_curve: vec![initial_capital],
            max_drawdown: 0.0,
            max_drawdown_pct: 0.0,
            position_sizing: PositionSizing::FixedFractional(1.0), // 默认全仓
            stop_loss: StopLoss::None,
            trailing_stop_activated: false,
            trailing_stop_price: 0.0,
            max_equity_drawdown_pct: 100.0, // 默认不止损
            trades: Vec::new(),
            short_margin_requirement: 0.5,
        }
    }

    /// 设置仓位管理
    pub fn with_position_sizing(mut self, sizing: PositionSizing) -> Self {
        self.position_sizing = sizing;
        self
    }

    /// 设置止损
    pub fn with_stop_loss(mut self, sl: StopLoss) -> Self {
        self.stop_loss = sl;
        self
    }

    /// 设置最大回撤止损
    pub fn with_max_drawdown_stop(mut self, max_dd_pct: f64) -> Self {
        self.max_equity_drawdown_pct = max_dd_pct;
        self
    }

    /// 计算仓位数量
    fn calculate_position_size(&self, price: f64, side: TradeSide) -> f64 {
        match self.position_sizing {
            PositionSizing::FixedFractional(frac) => {
                let capital_to_use = self.current_capital * frac.clamp(0.0, 1.0);
                let after_commission = capital_to_use * (1.0 - self.commission_rate);
                match side {
                    TradeSide::Long => after_commission / price,
                    TradeSide::Short => {
                        // 做空使用保证金
                        (after_commission / self.short_margin_requirement) / price
                    }
                }
            }
            PositionSizing::Kelly(kelly_frac) => {
                // 使用简化的Kelly公式：f* = (p*b - q)/b
                // 这里用历史胜率估算，简单实现用固定比例
                let base_kelly = 0.25; // 默认使用 25% 的 Kelly
                let effective_kelly = base_kelly * kelly_frac.clamp(0.0, 1.0);
                let capital_to_use = self.current_capital * effective_kelly;
                let after_commission = capital_to_use * (1.0 - self.commission_rate);
                match side {
                    TradeSide::Long => after_commission / price,
                    TradeSide::Short => (after_commission / self.short_margin_requirement) / price,
                }
            }
            PositionSizing::FixedQuantity(qty) => qty.abs(),
        }
    }

    /// 检查是否触发止损
    fn check_stop_loss(&self, _current_price: f64, high: f64, low: f64) -> Option<TradeSide> {
        if self.position == 0.0 {
            return None;
        }

        match self.stop_loss {
            StopLoss::None => None,
            StopLoss::FixedPercent(pct) => {
                if self.position > 0.0 {
                    // 多头止损
                    let stop_price = self.avg_entry_price * (1.0 - pct);
                    if low <= stop_price {
                        return Some(TradeSide::Short); // 卖出平仓
                    }
                } else {
                    // 空头止损
                    let stop_price = self.avg_entry_price * (1.0 + pct);
                    if high >= stop_price {
                        return Some(TradeSide::Long); // 买入平仓
                    }
                }
                None
            }
            StopLoss::AtrTrailing(_multiplier) => {
                // ATR Trailing Stop 在 run 方法中处理
                None
            }
            StopLoss::FixedAmount(amount) => {
                if self.position > 0.0 {
                    let stop_price = self.avg_entry_price - amount;
                    if low <= stop_price {
                        return Some(TradeSide::Short);
                    }
                } else {
                    let stop_price = self.avg_entry_price + amount;
                    if high >= stop_price {
                        return Some(TradeSide::Long);
                    }
                }
                None
            }
        }
    }

    /// 运行回测
    pub fn run(
        &mut self,
        candles: &[Candle],
        strategy: &dyn Strategy,
    ) -> anyhow::Result<Vec<Trade>> {
        let signals = strategy.generate(candles);
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();

        // 如果启用了 ATR trailing stop，预计算 ATR
        let atr_values = if let StopLoss::AtrTrailing(_) = self.stop_loss {
            crate::trading::indicators::atr(&highs, &lows, &closes, 14)
        } else {
            vec![f64::NAN; candles.len()]
        };

        for i in 0..candles.len() {
            let price = closes[i];
            let high = highs[i];
            let low = lows[i];
            let signal = signals[i];

            // --- 检查最大回撤止损 ---
            let total_equity = self.current_capital + self.position_value;
            let drawdown_from_peak = if self.peak_capital > 0.0 {
                (self.peak_capital - total_equity) / self.peak_capital * 100.0
            } else {
                0.0
            };
            if drawdown_from_peak > self.max_equity_drawdown_pct && self.position != 0.0 {
                // 强制平仓
                self.close_position(i, price, "Max Drawdown", candles);
            }

            // --- 检查止损 ---
            if self.position != 0.0 {
                if let Some(stop_side) = self.check_stop_loss(price, high, low) {
                    match stop_side {
                        TradeSide::Short => {
                            // 多头止损 → 卖出
                            if self.position > 0.0 {
                                self.close_position(i, price, "Stop Loss", candles);
                            }
                        }
                        TradeSide::Long => {
                            // 空头止损 → 买入平仓
                            if self.position < 0.0 {
                                self.close_position(i, price, "Stop Loss", candles);
                            }
                        }
                    }
                }

                // ATR Trailing Stop
                if let StopLoss::AtrTrailing(multiplier) = self.stop_loss {
                    if i < atr_values.len() && !atr_values[i].is_nan() {
                        let current_atr = atr_values[i];
                        if self.position > 0.0 {
                            // 多头 trailing stop
                            let new_stop = high - multiplier * current_atr;
                            if !self.trailing_stop_activated || new_stop > self.trailing_stop_price
                            {
                                self.trailing_stop_price = new_stop;
                                self.trailing_stop_activated = true;
                            }
                            if low <= self.trailing_stop_price && self.trailing_stop_activated {
                                self.close_position(i, price, "Trailing Stop", candles);
                            }
                        } else if self.position < 0.0 {
                            // 空头 trailing stop
                            let new_stop = low + multiplier * current_atr;
                            if !self.trailing_stop_activated || new_stop < self.trailing_stop_price
                            {
                                self.trailing_stop_price = new_stop;
                                self.trailing_stop_activated = true;
                            }
                            if high >= self.trailing_stop_price && self.trailing_stop_activated {
                                self.close_position(i, price, "Trailing Stop", candles);
                            }
                        }
                    }
                }
            }

            // --- 处理交易信号 ---
            match signal {
                Signal::Buy => {
                    if self.position <= 0.0 {
                        // 如果持有空头，先平空
                        if self.position < 0.0 {
                            self.close_position(i, price, "Signal Buy (Cover)", candles);
                        }
                        // 开多头
                        self.open_position(i, price, TradeSide::Long, candles);
                    }
                }
                Signal::Sell => {
                    if self.position >= 0.0 {
                        // 如果持有多头，先平多
                        if self.position > 0.0 {
                            self.close_position(i, price, "Signal Sell", candles);
                        }
                        // 开空头
                        self.open_position(i, price, TradeSide::Short, candles);
                    }
                }
                Signal::Hold => {}
            }

            // 更新持仓价值
            let total_equity = self.update_equity(price);

            // 更新峰值和回撤
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
        if self.position != 0.0 {
            let last_idx = candles.len() - 1;
            self.close_position(last_idx, closes[last_idx], "End of Data", candles);
            let final_equity = self.update_equity(closes[last_idx]);
            self.equity_curve.push(final_equity);
        }

        Ok(std::mem::take(&mut self.trades))
    }

    /// 开仓
    fn open_position(&mut self, i: usize, price: f64, side: TradeSide, _candles: &[Candle]) {
        let qty = self.calculate_position_size(price, side);
        if qty <= 0.0 {
            return;
        }

        let exec_price = match side {
            TradeSide::Long => price * (1.0 + self.slippage),
            TradeSide::Short => price * (1.0 - self.slippage),
        };

        let position_value = qty * exec_price;
        let commission = position_value * self.commission_rate;

        match side {
            TradeSide::Long => {
                self.position = qty;
                self.current_capital -= position_value + commission;
                self.total_long_trades += 1;
            }
            TradeSide::Short => {
                // 做空：收到卖出的资金，但需要冻结保证金
                self.position = -qty;
                let margin = position_value * self.short_margin_requirement;
                self.current_capital += position_value - margin - commission;
                self.total_short_trades += 1;
            }
        }

        self.avg_entry_price = exec_price;
        self.position_value = position_value;
        self.entry_candle_idx = i;
        self.trailing_stop_activated = false;
        self.trailing_stop_price = 0.0;
    }

    /// 平仓
    fn close_position(&mut self, i: usize, price: f64, reason: &str, candles: &[Candle]) {
        if self.position == 0.0 {
            return;
        }

        let is_long = self.position > 0.0;
        let qty = self.position.abs();

        let exec_price = if is_long {
            price * (1.0 - self.slippage) // 多头卖出
        } else {
            price * (1.0 + self.slippage) // 空头买入平仓
        };

        let revenue = qty * exec_price;
        let commission = revenue * self.commission_rate;

        let pnl = if is_long {
            revenue - commission - self.position_value
        } else {
            self.position_value - revenue - commission
        };

        let entry_price = self.avg_entry_price;
        let pnl_pct = if entry_price > 0.0 {
            if is_long {
                (exec_price - entry_price) / entry_price * 100.0
            } else {
                (entry_price - exec_price) / entry_price * 100.0
            }
        } else {
            0.0
        };

        if is_long {
            self.current_capital += revenue - commission;
        } else {
            let margin = self.position_value * self.short_margin_requirement;
            self.current_capital += margin + revenue - commission;
        }

        if pnl > 0.0 {
            self.winning_trades += 1;
        } else {
            self.losing_trades += 1;
        }

        let bars_held = i.saturating_sub(self.entry_candle_idx);
        let entry_time = candles[self.entry_candle_idx.min(candles.len().saturating_sub(1))]
            .timestamp
            .to_rfc3339();

        self.trades.push(Trade {
            entry_time,
            exit_time: candles[i].timestamp.to_rfc3339(),
            side: if is_long { TradeSide::Long } else { TradeSide::Short },
            entry_price,
            exit_price: exec_price,
            quantity: qty,
            pnl,
            pnl_percent: pnl_pct,
            bars_held,
            exit_reason: reason.to_string(),
        });

        self.position = 0.0;
        self.position_value = 0.0;
        self.avg_entry_price = 0.0;
        self.trailing_stop_activated = false;
    }

    /// 更新权益
    fn update_equity(&mut self, current_price: f64) -> f64 {
        if self.position > 0.0 {
            self.position_value = self.position * current_price;
        } else if self.position < 0.0 {
            // 空头：持仓价值按当前市价计算
            self.position_value = (-self.position) * current_price;
        }

        let total_equity = if self.position >= 0.0 {
            self.current_capital + self.position_value
        } else {
            // 空头：资金 = 现金 + 卖出收入 - 当前持仓市值
            self.current_capital + self.position_value
        };

        self.equity_curve.push(total_equity);
        total_equity
    }

    /// 获取总权益
    pub fn total_equity(&self) -> f64 {
        self.equity_curve.last().copied().unwrap_or(self.initial_capital)
    }

    /// 获取收益率
    pub fn total_return(&self) -> f64 {
        if self.initial_capital > 0.0 {
            (self.total_equity() - self.initial_capital) / self.initial_capital * 100.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading::data::DataSource;
    use crate::trading::strategy::SmaCrossover;

    #[test]
    fn test_backtest_runs() {
        let candles = DataSource::generate_mock(200, 100.0);
        let strategy = SmaCrossover::new(5, 20);
        let mut engine = BacktestEngine::new(10_000.0, 0.001, 0.001);
        let _trades = engine.run(&candles, &strategy).unwrap();
        assert!(engine.total_equity() > 0.0);
    }

    #[test]
    fn test_backtest_with_position_sizing() {
        let candles = DataSource::generate_mock(200, 100.0);
        let strategy = SmaCrossover::new(5, 20);
        let mut engine = BacktestEngine::new(10_000.0, 0.001, 0.001)
            .with_position_sizing(PositionSizing::FixedFractional(0.5));
        let _trades = engine.run(&candles, &strategy).unwrap();
        assert!(engine.total_equity() > 0.0);
    }

    #[test]
    fn test_backtest_with_stop_loss() {
        let candles = DataSource::generate_mock(200, 100.0);
        let strategy = SmaCrossover::new(5, 20);
        let mut engine = BacktestEngine::new(10_000.0, 0.001, 0.001)
            .with_stop_loss(StopLoss::FixedPercent(0.05));
        let _trades = engine.run(&candles, &strategy).unwrap();
        assert!(engine.total_equity() > 0.0);
    }

    #[test]
    fn test_backtest_with_atr_trailing_stop() {
        let candles = DataSource::generate_mock(200, 100.0);
        let strategy = SmaCrossover::new(5, 20);
        let mut engine =
            BacktestEngine::new(10_000.0, 0.001, 0.001).with_stop_loss(StopLoss::AtrTrailing(3.0));
        let _trades = engine.run(&candles, &strategy).unwrap();
        assert!(engine.total_equity() > 0.0);
    }

    #[test]
    fn test_backtest_with_max_drawdown_stop() {
        let candles = DataSource::generate_mock(200, 100.0);
        let strategy = SmaCrossover::new(5, 20);
        let mut engine = BacktestEngine::new(10_000.0, 0.001, 0.001).with_max_drawdown_stop(15.0);
        let _trades = engine.run(&candles, &strategy).unwrap();
        assert!(engine.total_equity() > 0.0);
    }

    #[test]
    fn test_backtest_with_kelly_sizing() {
        let candles = DataSource::generate_mock(200, 100.0);
        let strategy = SmaCrossover::new(5, 20);
        let mut engine = BacktestEngine::new(10_000.0, 0.001, 0.001)
            .with_position_sizing(PositionSizing::Kelly(0.25));
        let _trades = engine.run(&candles, &strategy).unwrap();
        assert!(engine.total_equity() > 0.0);
    }

    #[test]
    fn test_backtest_all_features() {
        let candles = DataSource::generate_mock(300, 100.0);
        let strategy = SmaCrossover::new(5, 20);
        let mut engine = BacktestEngine::new(10_000.0, 0.001, 0.001)
            .with_position_sizing(PositionSizing::FixedFractional(0.5))
            .with_stop_loss(StopLoss::AtrTrailing(3.0))
            .with_max_drawdown_stop(20.0);
        let trades = engine.run(&candles, &strategy).unwrap();
        assert!(engine.total_equity() > 0.0);
        assert!(engine.total_long_trades + engine.total_short_trades >= trades.len());
    }
}
