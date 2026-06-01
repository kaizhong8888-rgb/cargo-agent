/// 动态风险管理模块
/// 包括：Kelly公式仓位、ATR动态止损止盈、最大回撤控制、波动率调整仓位

use crate::data::Candle;
use crate::indicators;

/// 仓位管理配置
#[derive(Debug, Clone)]
pub struct PositionConfig {
    /// 单笔交易最大风险百分比 (默认 2%)
    pub risk_per_trade_pct: f64,
    /// Kelly 分数折扣 (默认 0.5 = 半Kelly，降低风险)
    pub kelly_fraction: f64,
    /// 最大仓位占比 (默认 25%，单标的上限)
    pub max_position_pct: f64,
    /// 最小仓位占比 (默认 5%)
    pub min_position_pct: f64,
}

impl Default for PositionConfig {
    fn default() -> Self {
        Self {
            risk_per_trade_pct: 0.02,
            kelly_fraction: 0.5,
            max_position_pct: 0.25,
            min_position_pct: 0.05,
        }
    }
}

/// 动态止损止盈配置
#[derive(Debug, Clone)]
pub struct StopLossConfig {
    /// ATR 止损倍数 (默认 2.0)
    pub atr_stop_multiplier: f64,
    /// ATR 止盈倍数 (默认 3.0)
    pub atr_take_profit_multiplier: f64,
    /// 追踪止损 ATR 倍数 (默认 1.5)
    pub trailing_stop_atr: f64,
    /// 固定百分比止损 (默认 5%)
    pub fixed_stop_pct: f64,
    /// 固定百分比止盈 (默认 10%)
    pub fixed_take_profit_pct: f64,
    /// 启用追踪止损
    pub enable_trailing_stop: bool,
    /// 启用盈亏平衡止损 (盈利达到某个阈值后将止损移至成本价)
    pub enable_breakeven: bool,
    /// 盈亏平衡触发阈值 (默认 1.0 * ATR)
    pub breakeven_threshold_atr: f64,
}

impl Default for StopLossConfig {
    fn default() -> Self {
        Self {
            atr_stop_multiplier: 2.0,
            atr_take_profit_multiplier: 3.0,
            trailing_stop_atr: 1.5,
            fixed_stop_pct: 0.05,
            fixed_take_profit_pct: 0.10,
            enable_trailing_stop: true,
            enable_breakeven: true,
            breakeven_threshold_atr: 1.0,
        }
    }
}

/// 最大回撤控制配置
#[derive(Debug, Clone)]
pub struct DrawdownControl {
    /// 最大允许回撤百分比 (默认 20%)
    pub max_drawdown_pct: f64,
    /// 回撤达到此阈值时减仓比例 (默认 15%)
    pub reduce_position_at_pct: f64,
    /// 减仓比例 (默认 50%)
    pub reduce_pct: f64,
    /// 回撤恢复后重新入场的阈值 (默认 恢复 80%)
    pub recovery_threshold_pct: f64,
}

impl Default for DrawdownControl {
    fn default() -> Self {
        Self {
            max_drawdown_pct: 0.20,
            reduce_position_at_pct: 0.15,
            reduce_pct: 0.50,
            recovery_threshold_pct: 0.80,
        }
    }
}

/// 风险管理器
pub struct RiskManager {
    pub position_config: PositionConfig,
    pub stop_loss_config: StopLossConfig,
    pub drawdown_control: DrawdownControl,
    // 内部状态
    peak_equity: f64,
    current_equity: f64,
    initial_equity: f64,
    is_reduced: bool,
    trade_history: Vec<f64>, // 存储每笔交易的PnL百分比
}

impl RiskManager {
    pub fn new(initial_equity: f64) -> Self {
        Self {
            position_config: PositionConfig::default(),
            stop_loss_config: StopLossConfig::default(),
            drawdown_control: DrawdownControl::default(),
            peak_equity: initial_equity,
            current_equity: initial_equity,
            initial_equity,
            is_reduced: false,
            trade_history: Vec::new(),
        }
    }

    /// 计算 Kelly 最优仓位比例
    /// Kelly % = W - [(1 - W) / R]
    /// W = 胜率, R = 平均盈利/平均亏损
    pub fn kelly_percentage(&self) -> f64 {
        if self.trade_history.len() < 5 {
            return 0.0; // 样本不足
        }

        let wins: Vec<f64> = self.trade_history.iter().filter(|&&p| p > 0.0).copied().collect();
        let losses: Vec<f64> = self.trade_history.iter().filter(|&&p| p < 0.0).copied().collect();

        if wins.is_empty() || losses.is_empty() {
            return 0.0;
        }

        let win_rate = wins.len() as f64 / self.trade_history.len() as f64;
        let avg_win: f64 = wins.iter().sum::<f64>() / wins.len() as f64;
        let avg_loss: f64 = losses.iter().sum::<f64>().abs() / losses.len() as f64;

        if avg_loss == 0.0 {
            return 0.0;
        }

        let win_loss_ratio = avg_win / avg_loss;
        let kelly = win_rate - (1.0 - win_rate) / win_loss_ratio;

        // 应用 Kelly 折扣 (半Kelly更稳健)
        (kelly * self.position_config.kelly_fraction).max(0.0).min(self.position_config.max_position_pct)
    }

    /// 基于波动率调整仓位
    /// 高波动时减仓，低波动时加仓
    pub fn volatility_adjusted_position(
        &self,
        candles: &[Candle],
        atr_period: usize,
        target_volatility_pct: f64,
    ) -> f64 {
        if candles.len() < atr_period + 1 {
            return self.position_config.risk_per_trade_pct;
        }

        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        let atr_values = indicators::atr(&highs, &lows, &closes, atr_period);
        let current_atr = atr_values.last().copied().unwrap_or(0.0);
        let current_price = closes.last().copied().unwrap_or(1.0);

        if current_price == 0.0 || current_atr == 0.0 {
            return self.position_config.risk_per_trade_pct;
        }

        // 当前波动率 = ATR / 价格
        let current_volatility = current_atr / current_price;

        // 目标仓位 = 基础风险 * (目标波动率 / 当前波动率)
        let adjusted_risk = if current_volatility > 0.0 {
            self.position_config.risk_per_trade_pct * (target_volatility_pct / current_volatility)
        } else {
            self.position_config.risk_per_trade_pct
        };

        // 限制在最小和最大范围内
        adjusted_risk
            .max(self.position_config.min_position_pct)
            .min(self.position_config.max_position_pct)
    }

    /// 计算基于 ATR 的动态止损价格
    pub fn calculate_atr_stop_loss(
        &self,
        entry_price: f64,
        atr: f64,
        is_long: bool,
    ) -> f64 {
        if is_long {
            entry_price - atr * self.stop_loss_config.atr_stop_multiplier
        } else {
            entry_price + atr * self.stop_loss_config.atr_stop_multiplier
        }
    }

    /// 计算基于 ATR 的动态止盈价格
    pub fn calculate_atr_take_profit(
        &self,
        entry_price: f64,
        atr: f64,
        is_long: bool,
    ) -> f64 {
        if is_long {
            entry_price + atr * self.stop_loss_config.atr_take_profit_multiplier
        } else {
            entry_price - atr * self.stop_loss_config.atr_take_profit_multiplier
        }
    }

    /// 计算追踪止损价格
    pub fn calculate_trailing_stop(
        &self,
        current_price: f64,
        highest_price: f64,
        atr: f64,
        is_long: bool,
    ) -> f64 {
        if is_long {
            // 多头：追踪止损 = 最高价 - ATR * 倍数
            highest_price - atr * self.stop_loss_config.trailing_stop_atr
        } else {
            // 空头：追踪止损 = 最低价 + ATR * 倍数
            highest_price + atr * self.stop_loss_config.trailing_stop_atr
        }
    }

    /// 检查是否触发最大回撤控制
    /// 返回: (是否应该停止交易, 是否应该减仓, 减仓比例)
    pub fn check_drawdown_control(&mut self, current_equity: f64) -> (bool, bool, f64) {
        self.current_equity = current_equity;

        // 更新峰值
        if current_equity > self.peak_equity {
            self.peak_equity = current_equity;
            self.is_reduced = false;
        }

        let drawdown_pct = if self.peak_equity > 0.0 {
            (self.peak_equity - current_equity) / self.peak_equity
        } else {
            0.0
        };

        // 超过最大回撤阈值 → 停止所有交易
        if drawdown_pct >= self.drawdown_control.max_drawdown_pct {
            return (true, false, 0.0);
        }

        // 超过减仓阈值 → 减仓
        if drawdown_pct >= self.drawdown_control.reduce_position_at_pct && !self.is_reduced {
            self.is_reduced = true;
            return (false, true, self.drawdown_control.reduce_pct);
        }

        (false, false, 0.0)
    }

    /// 记录交易结果
    pub fn record_trade(&mut self, pnl_pct: f64) {
        self.trade_history.push(pnl_pct);
    }

    /// 获取当前回撤百分比
    pub fn current_drawdown_pct(&self) -> f64 {
        if self.peak_equity > 0.0 {
            (self.peak_equity - self.current_equity) / self.peak_equity
        } else {
            0.0
        }
    }

    /// 获取综合仓位建议
    /// 结合 Kelly、波动率调整、回撤控制
    pub fn get_position_size(
        &mut self,
        current_equity: f64,
        candles: &[Candle],
        atr_period: usize,
        target_volatility_pct: f64,
    ) -> f64 {
        // 1. 检查回撤控制
        let (stop_trading, should_reduce, reduce_pct) = self.check_drawdown_control(current_equity);

        if stop_trading {
            return 0.0; // 停止交易
        }

        // 2. 计算基础仓位 (Kelly 或固定风险)
        let base_position = if self.trade_history.len() >= 10 {
            self.kelly_percentage()
        } else {
            self.position_config.risk_per_trade_pct
        };

        // 3. 波动率调整
        let vol_adjusted = self.volatility_adjusted_position(
            candles,
            atr_period,
            target_volatility_pct,
        );

        // 4. 取两者中较小值 (更保守)
        let mut position = base_position.min(vol_adjusted);

        // 5. 应用回撤减仓
        if should_reduce {
            position *= (1.0 - reduce_pct);
        }

        // 6. 限制范围
        position
            .max(self.position_config.min_position_pct)
            .min(self.position_config.max_position_pct)
    }
}

/// 出场信号检测结果
#[derive(Debug, Clone)]
pub struct ExitSignal {
    pub should_exit: bool,
    pub reason: String,
    pub exit_price: f64,
}

impl ExitSignal {
    pub fn none() -> Self {
        Self {
            should_exit: false,
            reason: String::new(),
            exit_price: 0.0,
        }
    }

    pub fn stop_loss(price: f64, reason: &str) -> Self {
        Self {
            should_exit: true,
            reason: reason.to_string(),
            exit_price: price,
        }
    }

    pub fn take_profit(price: f64, reason: &str) -> Self {
        Self {
            should_exit: true,
            reason: reason.to_string(),
            exit_price: price,
        }
    }
}

/// 动态出场管理器
pub struct ExitManager {
    pub config: StopLossConfig,
    entry_price: f64,
    highest_price: f64,
    lowest_price: f64,
    is_long: bool,
}

impl ExitManager {
    pub fn new(entry_price: f64, is_long: bool) -> Self {
        Self {
            config: StopLossConfig::default(),
            entry_price,
            highest_price: entry_price,
            lowest_price: entry_price,
            is_long,
        }
    }

    pub fn with_config(mut self, config: StopLossConfig) -> Self {
        self.config = config;
        self
    }

    /// 更新最高/最低价
    pub fn update_price(&mut self, price: f64) {
        self.highest_price = self.highest_price.max(price);
        self.lowest_price = self.lowest_price.min(price);
    }

    /// 检查是否应该出场
    pub fn check_exit(
        &self,
        current_price: f64,
        atr: f64,
        breakeven_triggered: bool,
    ) -> ExitSignal {
        if self.is_long {
            self.check_long_exit(current_price, atr, breakeven_triggered)
        } else {
            self.check_short_exit(current_price, atr, breakeven_triggered)
        }
    }

    fn check_long_exit(
        &self,
        current_price: f64,
        atr: f64,
        breakeven_triggered: bool,
    ) -> ExitSignal {
        // 1. 固定止损
        let fixed_sl = self.entry_price * (1.0 - self.config.fixed_stop_pct);
        if current_price <= fixed_sl {
            return ExitSignal::stop_loss(current_price, "Fixed Stop Loss");
        }

        // 2. ATR 止损
        let atr_sl = self.entry_price - atr * self.config.atr_stop_multiplier;
        if current_price <= atr_sl {
            return ExitSignal::stop_loss(current_price, "ATR Stop Loss");
        }

        // 3. 追踪止损
        if self.config.enable_trailing_stop {
            let trailing_sl = self.highest_price - atr * self.config.trailing_stop_atr;
            if current_price <= trailing_sl && self.highest_price > self.entry_price {
                return ExitSignal::stop_loss(current_price, "Trailing Stop");
            }
        }

        // 4. 盈亏平衡止损
        if self.config.enable_breakeven && breakeven_triggered {
            let breakeven_price = self.entry_price * (1.0 + 0.001); // 略高于成本覆盖手续费
            if current_price <= breakeven_price {
                return ExitSignal::stop_loss(current_price, "Breakeven Stop");
            }
        }

        // 5. 固定止盈
        let fixed_tp = self.entry_price * (1.0 + self.config.fixed_take_profit_pct);
        if current_price >= fixed_tp {
            return ExitSignal::take_profit(current_price, "Fixed Take Profit");
        }

        // 6. ATR 止盈
        let atr_tp = self.entry_price + atr * self.config.atr_take_profit_multiplier;
        if current_price >= atr_tp {
            return ExitSignal::take_profit(current_price, "ATR Take Profit");
        }

        ExitSignal::none()
    }

    fn check_short_exit(
        &self,
        current_price: f64,
        atr: f64,
        breakeven_triggered: bool,
    ) -> ExitSignal {
        // 1. 固定止损
        let fixed_sl = self.entry_price * (1.0 + self.config.fixed_stop_pct);
        if current_price >= fixed_sl {
            return ExitSignal::stop_loss(current_price, "Fixed Stop Loss");
        }

        // 2. ATR 止损
        let atr_sl = self.entry_price + atr * self.config.atr_stop_multiplier;
        if current_price >= atr_sl {
            return ExitSignal::stop_loss(current_price, "ATR Stop Loss");
        }

        // 3. 追踪止损
        if self.config.enable_trailing_stop {
            let trailing_sl = self.lowest_price + atr * self.config.trailing_stop_atr;
            if current_price >= trailing_sl && self.lowest_price < self.entry_price {
                return ExitSignal::stop_loss(current_price, "Trailing Stop");
            }
        }

        // 4. 盈亏平衡止损
        if self.config.enable_breakeven && breakeven_triggered {
            let breakeven_price = self.entry_price * (1.0 - 0.001);
            if current_price >= breakeven_price {
                return ExitSignal::stop_loss(current_price, "Breakeven Stop");
            }
        }

        // 5. 固定止盈
        let fixed_tp = self.entry_price * (1.0 - self.config.fixed_take_profit_pct);
        if current_price <= fixed_tp {
            return ExitSignal::take_profit(current_price, "Fixed Take Profit");
        }

        // 6. ATR 止盈
        let atr_tp = self.entry_price - atr * self.config.atr_take_profit_multiplier;
        if current_price <= atr_tp {
            return ExitSignal::take_profit(current_price, "ATR Take Profit");
        }

        ExitSignal::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::DataSource;

    #[test]
    fn test_kelly_percentage() {
        let mut rm = RiskManager::new(10000.0);

        // 模拟60%胜率，平均盈利2%，平均亏损1%的交易
        for _ in 0..30 {
            rm.trade_history.push(0.02);
        }
        for _ in 0..20 {
            rm.trade_history.push(-0.01);
        }

        let kelly = rm.kelly_percentage();
        // Kelly = 0.6 - (0.4 / 2.0) = 0.6 - 0.2 = 0.4
        // 半Kelly = 0.2
        assert!(kelly > 0.1 && kelly <= 0.25);
    }

    #[test]
    fn test_atr_stop_loss() {
        let rm = RiskManager::new(10000.0);
        let entry = 100.0;
        let atr = 2.0;

        let long_sl = rm.calculate_atr_stop_loss(entry, atr, true);
        assert!((long_sl - 96.0).abs() < 0.01); // 100 - 2*2 = 96

        let short_sl = rm.calculate_atr_stop_loss(entry, atr, false);
        assert!((short_sl - 104.0).abs() < 0.01); // 100 + 2*2 = 104
    }

    #[test]
    fn test_drawdown_control() {
        let mut rm = RiskManager::new(10000.0);
        rm.drawdown_control.max_drawdown_pct = 0.20;
        rm.drawdown_control.reduce_position_at_pct = 0.15;

        // 正常情况
        let (stop, reduce, pct) = rm.check_drawdown_control(9500.0); // 5% DD
        assert!(!stop && !reduce);

        // 减仓阈值
        let (stop, reduce, pct) = rm.check_drawdown_control(8400.0); // 16% DD
        assert!(!stop && reduce);
        assert!((pct - 0.5).abs() < 0.01);

        // 停止交易阈值
        let (stop, reduce, pct) = rm.check_drawdown_control(7900.0); // 21% DD
        assert!(stop && !reduce);
    }

    #[test]
    fn test_exit_manager_long() {
        let exit_mgr = ExitManager::new(100.0, true);

        // 价格未触发任何条件
        let signal = exit_mgr.check_exit(100.5, 2.0, false);
        assert!(!signal.should_exit);

        // 触发固定止损
        let signal = exit_mgr.check_exit(94.0, 2.0, false);
        assert!(signal.should_exit);
        assert!(signal.reason.contains("Fixed Stop Loss"));

        // 触发固定止盈
        let signal = exit_mgr.check_exit(111.0, 2.0, false);
        assert!(signal.should_exit);
        assert!(signal.reason.contains("Fixed Take Profit"));
    }

    #[test]
    fn test_volatility_adjusted_position() {
        let rm = RiskManager::new(10000.0);
        let candles = DataSource::generate_mock(100, 100.0);

        let position = rm.volatility_adjusted_position(&candles, 14, 0.02);
        assert!(position > 0.0 && position <= 0.25);
    }
}
