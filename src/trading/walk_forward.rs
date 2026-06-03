/// Walk-Forward 向前验证分析
/// 用于验证策略在样本外数据上的稳健性，避免过拟合
use serde::{Deserialize, Serialize};

use super::data::Candle;
use super::report::BacktestResult;

/// Walk-Forward 窗口类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowType {
    /// 固定窗口：训练窗口和测试窗口大小固定
    Fixed,
    /// 扩展窗口：训练窗口不断扩展，测试窗口大小固定
    Expanding,
    /// 滚动窗口：训练窗口和测试窗口一起滚动
    Rolling,
}

/// Walk-Forward 配置
#[derive(Debug, Clone)]
pub struct WalkForwardConfig {
    /// 训练窗口大小（K线数量）
    pub train_window: usize,
    /// 测试窗口大小（K线数量）
    pub test_window: usize,
    /// 步长（每次前进的K线数量）
    pub step: usize,
    /// 窗口类型
    pub window_type: WindowType,
    /// 最小训练窗口大小（数据不足时跳过）
    pub min_train_size: usize,
}

impl Default for WalkForwardConfig {
    fn default() -> Self {
        Self {
            train_window: 252, // 1年交易日
            test_window: 63,   // 1季度交易日
            step: 63,          // 每次前进1季度
            window_type: WindowType::Rolling,
            min_train_size: 126, // 至少半年数据
        }
    }
}

/// 单个Walk-Forward周期的结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfPeriodResult {
    /// 周期索引
    pub period: usize,
    /// 训练数据起止索引
    pub train_start: usize,
    pub train_end: usize,
    /// 测试数据起止索引
    pub test_start: usize,
    pub test_end: usize,
    /// 训练集结果
    pub train_result: Option<WfMetrics>,
    /// 测试集结果
    pub test_result: Option<WfMetrics>,
}

/// Walk-Forward 指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfMetrics {
    /// 总收益率
    pub total_return: f64,
    /// 年化收益率
    pub annualized_return: f64,
    /// 最大回撤
    pub max_drawdown: f64,
    /// 夏普比率
    pub sharpe_ratio: f64,
    /// 胜率
    pub win_rate: f64,
    /// 盈亏比
    pub profit_factor: f64,
    /// 交易次数
    pub trade_count: usize,
}

impl WfMetrics {
    pub fn from_backtest_result(result: &BacktestResult, total_days: usize) -> Self {
        let eng = &result.engine;
        let years = total_days as f64 / 252.0;
        // P2: 修复 total_return_pct 是百分比(如20.0)需要转小数(0.20)
        let total_return_decimal = eng.total_return_pct / 100.0;
        let annualized_return = if years > 0.0 {
            (1.0_f64 + total_return_decimal).powf(1.0 / years) - 1.0
        } else {
            0.0
        };

        Self {
            total_return: total_return_decimal,
            annualized_return,
            max_drawdown: eng.max_drawdown,
            sharpe_ratio: eng.sharpe_ratio,
            win_rate: eng.win_rate / 100.0, // 转为小数
            profit_factor: eng.profit_factor,
            trade_count: eng.total_trades,
        }
    }
}

/// Walk-Forward 分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardResult {
    /// 各周期结果
    pub periods: Vec<WfPeriodResult>,
    /// 汇总指标
    pub summary: WfSummary,
    /// 是否通过稳健性检验
    pub is_robust: bool,
}

/// Walk-Forward 汇总统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfSummary {
    /// 测试集平均年化收益率
    pub avg_test_return: f64,
    /// 测试集平均夏普比率
    pub avg_test_sharpe: f64,
    /// 测试集平均最大回撤
    pub avg_test_drawdown: f64,
    /// 测试集平均胜率
    pub avg_test_win_rate: f64,
    /// 训练-测试收益率差异（Overfitting 指标）
    pub train_test_return_diff: f64,
    /// 训练-测试夏普差异
    pub train_test_sharpe_diff: f64,
    /// 测试集正收益周期数占比
    pub profitable_periods_pct: f64,
    /// 总测试周期数
    pub total_periods: usize,
    /// OOS（样本外）R²: 训练表现对测试表现的预测能力
    pub oos_r_squared: f64,
}

/// Walk-Forward 分析器
pub struct WalkForwardAnalyzer;

impl WalkForwardAnalyzer {
    /// 执行 Walk-Forward 分析
    ///
    /// 对每个窗口，返回 (训练集指标, 测试集指标)
    pub fn analyze(
        config: &WalkForwardConfig,
        candles: &[Candle],
        backtest_fn: impl Fn(&[Candle]) -> BacktestResult,
    ) -> WalkForwardResult {
        let mut periods = Vec::new();
        let mut period_idx = 0;
        let mut offset = 0;

        loop {
            let (train_start, train_end, test_start, test_end) = match config.window_type {
                WindowType::Fixed | WindowType::Rolling => {
                    let ts = offset;
                    let te = offset + config.train_window;
                    let ss = te;
                    let se = ss + config.test_window;

                    if se > candles.len() {
                        break;
                    }
                    (ts, te, ss, se)
                }
                WindowType::Expanding => {
                    let ts = 0;
                    let te = config.train_window + offset;
                    let ss = te;
                    let se = ss + config.test_window;

                    if se > candles.len() {
                        break;
                    }
                    if te - ts < config.min_train_size {
                        offset += config.step;
                        continue;
                    }
                    (ts, te, ss, se)
                }
            };

            let train_candles = &candles[train_start..train_end];
            let test_candles = &candles[test_start..test_end];

            let train_result = if train_candles.len() >= config.min_train_size {
                let bt = backtest_fn(train_candles);
                Some(WfMetrics::from_backtest_result(&bt, train_candles.len()))
            } else {
                None
            };

            let test_result = {
                let bt = backtest_fn(test_candles);
                Some(WfMetrics::from_backtest_result(&bt, test_candles.len()))
            };

            periods.push(WfPeriodResult {
                period: period_idx,
                train_start,
                train_end,
                test_start,
                test_end,
                train_result,
                test_result,
            });

            period_idx += 1;
            offset += config.step;

            // 防止无限循环
            if period_idx > 100 {
                break;
            }
        }

        let summary = Self::compute_summary(&periods);
        let is_robust = Self::check_robustness(&summary);

        WalkForwardResult {
            periods,
            summary,
            is_robust,
        }
    }

    /// 计算汇总统计
    fn compute_summary(periods: &[WfPeriodResult]) -> WfSummary {
        let valid_periods: Vec<&WfPeriodResult> = periods
            .iter()
            .filter(|p| p.train_result.is_some() && p.test_result.is_some())
            .collect();

        if valid_periods.is_empty() {
            return WfSummary {
                avg_test_return: 0.0,
                avg_test_sharpe: 0.0,
                avg_test_drawdown: 0.0,
                avg_test_win_rate: 0.0,
                train_test_return_diff: 0.0,
                train_test_sharpe_diff: 0.0,
                profitable_periods_pct: 0.0,
                total_periods: 0,
                oos_r_squared: 0.0,
            };
        }

        let n = valid_periods.len() as f64;

        let avg_test_return: f64 = valid_periods
            .iter()
            .map(|p| p.test_result.as_ref().unwrap().annualized_return)
            .sum::<f64>()
            / n;

        let avg_test_sharpe: f64 = valid_periods
            .iter()
            .map(|p| p.test_result.as_ref().unwrap().sharpe_ratio)
            .sum::<f64>()
            / n;

        let avg_test_drawdown: f64 = valid_periods
            .iter()
            .map(|p| p.test_result.as_ref().unwrap().max_drawdown)
            .sum::<f64>()
            / n;

        let avg_test_win_rate: f64 = valid_periods
            .iter()
            .map(|p| p.test_result.as_ref().unwrap().win_rate)
            .sum::<f64>()
            / n;

        let train_test_return_diff: f64 = valid_periods
            .iter()
            .map(|p| {
                p.train_result.as_ref().unwrap().annualized_return
                    - p.test_result.as_ref().unwrap().annualized_return
            })
            .sum::<f64>()
            / n;

        let train_test_sharpe_diff: f64 = valid_periods
            .iter()
            .map(|p| {
                p.train_result.as_ref().unwrap().sharpe_ratio
                    - p.test_result.as_ref().unwrap().sharpe_ratio
            })
            .sum::<f64>()
            / n;

        let profitable_count = valid_periods
            .iter()
            .filter(|p| p.test_result.as_ref().unwrap().annualized_return > 0.0)
            .count();
        let profitable_periods_pct = profitable_count as f64 / n;

        // OOS R² 计算
        let oos_r_squared = Self::compute_r_squared(
            &valid_periods
                .iter()
                .map(|p| p.train_result.as_ref().unwrap().sharpe_ratio)
                .collect::<Vec<_>>(),
            &valid_periods
                .iter()
                .map(|p| p.test_result.as_ref().unwrap().sharpe_ratio)
                .collect::<Vec<_>>(),
        );

        WfSummary {
            avg_test_return,
            avg_test_sharpe,
            avg_test_drawdown,
            avg_test_win_rate,
            train_test_return_diff,
            train_test_sharpe_diff,
            profitable_periods_pct,
            total_periods: valid_periods.len(),
            oos_r_squared,
        }
    }

    /// 计算 R² (决定系数)
    fn compute_r_squared(x: &[f64], y: &[f64]) -> f64 {
        if x.len() < 3 || x.len() != y.len() {
            return 0.0;
        }

        let n = x.len() as f64;
        let mean_x = x.iter().sum::<f64>() / n;
        let mean_y = y.iter().sum::<f64>() / n;

        let ss_tot: f64 = y.iter().map(|yi| (yi - mean_y).powi(2)).sum();
        if ss_tot < 1e-10 {
            return 0.0;
        }

        // 简单线性回归
        let ss_xy: f64 = x
            .iter()
            .zip(y.iter())
            .map(|(xi, yi)| (xi - mean_x) * (yi - mean_y))
            .sum();
        let ss_xx: f64 = x.iter().map(|xi| (xi - mean_x).powi(2)).sum();

        if ss_xx < 1e-10 {
            return 0.0;
        }

        let slope = ss_xy / ss_xx;
        let intercept = mean_y - slope * mean_x;

        let ss_res: f64 = x
            .iter()
            .zip(y.iter())
            .map(|(xi, yi)| {
                let predicted = slope * xi + intercept;
                (yi - predicted).powi(2)
            })
            .sum();

        (1.0 - ss_res / ss_tot).max(0.0)
    }

    /// 稳健性检验
    fn check_robustness(summary: &WfSummary) -> bool {
        // 条件1: 测试集平均收益为正
        let cond1 = summary.avg_test_return > 0.0;

        // 条件2: 训练-测试差异不太大（Overfitting < 50%）
        let cond2 = {
            if summary.avg_test_return.abs() < 1e-6 {
                summary.train_test_return_diff.abs() < 0.10
            } else {
                summary.train_test_return_diff / summary.avg_test_return.abs() < 0.50
            }
        };

        // 条件3: 盈利周期占比 > 50%
        let cond3 = summary.profitable_periods_pct > 0.5;

        // 条件4: OOS R² > -0.5（负相关太强说明过拟合）
        let cond4 = summary.oos_r_squared > -0.5;

        cond1 && cond2 && cond3 && cond4
    }

    /// 生成 Walk-Forward 分析报告
    pub fn generate_report(wf: &WalkForwardResult) -> String {
        let mut report = String::new();
        report.push_str("# Walk-Forward 分析报告\n\n");

        report.push_str("## 配置\n\n");
        report.push_str(&format!("- 总周期数: {}\n", wf.summary.total_periods));
        report.push_str("- 窗口类型: Rolling\n");

        report.push_str("\n## 汇总统计\n\n");
        report.push_str("| 指标 | 值 |\n");
        report.push_str("|------|-----|\n");
        report.push_str(&format!(
            "| 测试集平均年化收益 | {:.2}% |\n",
            wf.summary.avg_test_return * 100.0
        ));
        report.push_str(&format!(
            "| 测试集平均夏普比率 | {:.3} |\n",
            wf.summary.avg_test_sharpe
        ));
        report.push_str(&format!(
            "| 测试集平均最大回撤 | {:.2}% |\n",
            wf.summary.avg_test_drawdown * 100.0
        ));
        report.push_str(&format!(
            "| 测试集平均胜率 | {:.1}% |\n",
            wf.summary.avg_test_win_rate * 100.0
        ));
        report.push_str(&format!(
            "| 训练-测试收益差异 | {:.2}% |\n",
            wf.summary.train_test_return_diff * 100.0
        ));
        report.push_str(&format!(
            "| 训练-测试夏普差异 | {:.3} |\n",
            wf.summary.train_test_sharpe_diff
        ));
        report.push_str(&format!(
            "| 盈利周期占比 | {:.1}% |\n",
            wf.summary.profitable_periods_pct * 100.0
        ));
        report.push_str(&format!("| OOS R² | {:.3} |\n", wf.summary.oos_r_squared));

        report.push_str(&format!(
            "\n## 稳健性检验: {}\n\n",
            if wf.is_robust {
                "✅ 通过"
            } else {
                "❌ 未通过"
            }
        ));

        report.push_str("## 各周期详情\n\n");
        report.push_str(
            "| 周期 | 训练收益 | 测试收益 | 训练夏普 | 测试夏普 | 训练回撤 | 测试回撤 |\n",
        );
        report.push_str("|------|---------|---------|---------|---------|---------|---------|\n");

        for p in &wf.periods {
            let tr = p
                .train_result
                .as_ref()
                .map(|m| m.annualized_return)
                .unwrap_or(0.0);
            let te = p
                .test_result
                .as_ref()
                .map(|m| m.annualized_return)
                .unwrap_or(0.0);
            let tshr = p
                .train_result
                .as_ref()
                .map(|m| m.sharpe_ratio)
                .unwrap_or(0.0);
            let teshr = p
                .test_result
                .as_ref()
                .map(|m| m.sharpe_ratio)
                .unwrap_or(0.0);
            let tdd = p
                .train_result
                .as_ref()
                .map(|m| m.max_drawdown)
                .unwrap_or(0.0);
            let tedd = p
                .test_result
                .as_ref()
                .map(|m| m.max_drawdown)
                .unwrap_or(0.0);

            report.push_str(&format!(
                "| #{} | {:.2}% | {:.2}% | {:.3} | {:.3} | {:.2}% | {:.2}% |\n",
                p.period,
                tr * 100.0,
                te * 100.0,
                tshr,
                teshr,
                tdd * 100.0,
                tedd * 100.0
            ));
        }

        report
    }
}

/// Walk-Forward 参数优化器
/// 在Walk-Forward框架下搜索最优参数
pub struct WfParamOptimizer;

impl WfParamOptimizer {
    /// 在Walk-Forward框架下进行参数搜索
    ///
    /// 对每组参数执行完整的WF分析，选择在样本外表现最好的参数
    pub fn optimize<P, F>(
        param_sets: P,
        config: &WalkForwardConfig,
        candles: &[Candle],
        _backtest_fn: F,
    ) -> Vec<(String, WalkForwardResult)>
    where
        P: Iterator<Item = (String, Box<dyn Fn(&[Candle]) -> BacktestResult>)>,
        F: Fn(&[Candle]) -> BacktestResult,
    {
        let mut results: Vec<(String, WalkForwardResult)> = Vec::new();

        for (name, bt_fn) in param_sets {
            let wf = WalkForwardAnalyzer::analyze(config, candles, |c| bt_fn(c));
            results.push((name, wf));
        }

        // 按OOS夏普排序
        results.sort_by(|a, b| {
            b.1.summary
                .avg_test_sharpe
                .partial_cmp(&a.1.summary.avg_test_sharpe)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading::data::DataSource;
    use crate::trading::report::{BacktestResultData, TradePnlDistribution};

    fn simple_backtest(candles: &[Candle]) -> BacktestResult {
        // 简单均线策略作为测试
        if candles.len() < 20 {
            return BacktestResult {
                engine: BacktestResultData {
                    initial_capital: 100000.0,
                    final_equity: 100000.0,
                    total_return_pct: 0.0,
                    win_rate: 0.0,
                    max_drawdown_pct: 0.0,
                    sharpe_ratio: 0.0,
                    total_trades: 0,
                    winning_trades: 0,
                    losing_trades: 0,
                    avg_win: 0.0,
                    avg_loss: 0.0,
                    profit_factor: 0.0,
                    max_consecutive_wins: 0,
                    max_consecutive_losses: 0,
                    max_drawdown: 0.0,
                    total_pnl: 0.0,
                    avg_bars_held: 0.0,
                    calmar_ratio: 0.0,
                    sortino_ratio: 0.0,
                    recovery_factor: 0.0,
                    expectancy: 0.0,
                    max_bars_held: 0,
                    min_bars_held: 0,
                    long_trades: 0,
                    short_trades: 0,
                    long_win_rate: 0.0,
                    short_win_rate: 0.0,
                    return_std: 0.0,
                    downside_std: 0.0,
                },
                trades: vec![],
                total_bars: 0,
                monthly_returns: std::collections::HashMap::new(),
                trade_pnl_distribution: TradePnlDistribution {
                    wins: 0,
                    losses: 0,
                    max_win: 0.0,
                    max_loss: 0.0,
                    avg_win: 0.0,
                    avg_loss: 0.0,
                    win_loss_ratio: 0.0,
                    win_buckets: vec![],
                    loss_buckets: vec![],
                },
            };
        }

        let period = candles.len();
        // 模拟：价格上涨时收益正
        let first_close = candles[0].close;
        let last_close = candles.last().unwrap().close;
        let ret = (last_close - first_close) / first_close;

        let years = period as f64 / 252.0;
        let ann_ret = if years > 0.0 {
            (1.0 + ret).powf(1.0 / years) - 1.0
        } else {
            0.0
        };

        let sharpe = if ann_ret.is_finite() {
            (ann_ret - 0.03) / 0.15 // 假设波动率15%
        } else {
            0.0
        };

        BacktestResult {
            engine: BacktestResultData {
                initial_capital: 100000.0,
                final_equity: 100000.0 * (1.0 + ret),
                total_return_pct: ret * 100.0,
                win_rate: if ret > 0.0 { 60.0 } else { 40.0 },
                max_drawdown_pct: (ret.abs() * 0.5).min(0.30) * 100.0,
                sharpe_ratio: sharpe.max(0.0),
                total_trades: (period / 20) as usize,
                winning_trades: if ret > 0.0 { (period / 30) as usize } else { 0 },
                losing_trades: if ret <= 0.0 {
                    (period / 30) as usize
                } else {
                    0
                },
                avg_win: if ret > 0.0 { ret / 3.0 } else { 0.0 },
                avg_loss: if ret <= 0.0 { ret.abs() / 3.0 } else { 0.0 },
                profit_factor: if ret > 0.0 { 1.5 } else { 0.5 },
                max_consecutive_wins: 3,
                max_consecutive_losses: 2,
                max_drawdown: (ret.abs() * 0.5).min(0.30) * 100000.0,
                total_pnl: 100000.0 * ret,
                avg_bars_held: 0.0,
                calmar_ratio: 0.0,
                sortino_ratio: 0.0,
                recovery_factor: 0.0,
                expectancy: 0.0,
                max_bars_held: 0,
                min_bars_held: 0,
                long_trades: 0,
                short_trades: 0,
                long_win_rate: 0.0,
                short_win_rate: 0.0,
                return_std: 0.0,
                downside_std: 0.0,
            },
            trades: vec![],
            total_bars: 0,
            monthly_returns: std::collections::HashMap::new(),
            trade_pnl_distribution: TradePnlDistribution {
                wins: if ret > 0.0 { (period / 30) as usize } else { 0 },
                losses: if ret <= 0.0 {
                    (period / 30) as usize
                } else {
                    0
                },
                max_win: if ret > 0.0 { ret / 3.0 } else { 0.0 },
                max_loss: if ret <= 0.0 { ret.abs() / 3.0 } else { 0.0 },
                avg_win: if ret > 0.0 { ret / 3.0 } else { 0.0 },
                avg_loss: if ret <= 0.0 { ret.abs() / 3.0 } else { 0.0 },
                win_loss_ratio: if ret > 0.0 { 1.0 } else { 0.0 },
                win_buckets: vec![],
                loss_buckets: vec![],
            },
        }
    }

    #[test]
    fn test_walk_forward_rolling() {
        let candles = DataSource::generate_mock(600, 100.0);

        let config = WalkForwardConfig {
            train_window: 200,
            test_window: 50,
            step: 50,
            window_type: WindowType::Rolling,
            min_train_size: 100,
        };

        let wf = WalkForwardAnalyzer::analyze(&config, &candles, simple_backtest);

        assert!(!wf.periods.is_empty(), "应至少有一个周期");
        assert!(wf.summary.total_periods > 0, "应有有效周期");

        println!(
            "\nWalk-Forward (Rolling): {} 周期",
            wf.summary.total_periods
        );
        println!(
            "  测试集平均收益: {:.2}%",
            wf.summary.avg_test_return * 100.0
        );
        println!("  测试集平均夏普: {:.3}", wf.summary.avg_test_sharpe);
        println!("  稳健性: {}", if wf.is_robust { "通过" } else { "未通过" });
    }

    #[test]
    fn test_walk_forward_expanding() {
        let candles = DataSource::generate_mock(500, 100.0);

        let config = WalkForwardConfig {
            train_window: 150,
            test_window: 50,
            step: 50,
            window_type: WindowType::Expanding,
            min_train_size: 100,
        };

        let wf = WalkForwardAnalyzer::analyze(&config, &candles, simple_backtest);

        assert!(!wf.periods.is_empty());

        // 扩展窗口：训练集应越来越大
        if wf.periods.len() >= 2 {
            assert!(
                wf.periods[1].train_end - wf.periods[1].train_start
                    >= wf.periods[0].train_end - wf.periods[0].train_start,
                "扩展窗口训练集应递增"
            );
        }

        println!(
            "\nWalk-Forward (Expanding): {} 周期",
            wf.summary.total_periods
        );
    }

    #[test]
    fn test_wf_report_generation() {
        let candles = DataSource::generate_mock(500, 100.0);

        let config = WalkForwardConfig {
            train_window: 150,
            test_window: 50,
            step: 50,
            window_type: WindowType::Rolling,
            min_train_size: 100,
        };

        let wf = WalkForwardAnalyzer::analyze(&config, &candles, simple_backtest);
        let report = WalkForwardAnalyzer::generate_report(&wf);

        assert!(report.contains("Walk-Forward 分析报告"));
        assert!(report.contains("汇总统计"));
        assert!(report.contains("各周期详情"));
        assert!(report.contains("稳健性检验"));
    }

    #[test]
    fn test_r_squared() {
        // 完美正相关
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let r2 = WalkForwardAnalyzer::compute_r_squared(&x, &y);
        assert!(r2 > 0.99, "完美正相关R²应接近1");

        // 负相关
        let y_neg = vec![10.0, 8.0, 6.0, 4.0, 2.0];
        let r2_neg = WalkForwardAnalyzer::compute_r_squared(&x, &y_neg);
        assert!(r2_neg > 0.99, "完美负相关R²也应接近1");

        // 无相关（随机）
        let y_rand = vec![5.0, 1.0, 8.0, 2.0, 7.0];
        let r2_rand = WalkForwardAnalyzer::compute_r_squared(&x, &y_rand);
        assert!(r2_rand < 0.5, "无相关R²应较小");
    }

    #[test]
    fn test_wf_metrics_from_backtest() {
        let bt = BacktestResult {
            engine: BacktestResultData {
                initial_capital: 100000.0,
                final_equity: 120000.0,
                total_return_pct: 20.0,
                win_rate: 60.0,
                max_drawdown_pct: 10.0,
                sharpe_ratio: 1.2,
                total_trades: 50,
                winning_trades: 30,
                losing_trades: 20,
                avg_win: 0.02,
                avg_loss: -0.01,
                profit_factor: 2.0,
                max_consecutive_wins: 5,
                max_consecutive_losses: 3,
                max_drawdown: 0.10,
                total_pnl: 20000.0,
                avg_bars_held: 0.0,
                calmar_ratio: 2.0,
                sortino_ratio: 1.0,
                recovery_factor: 2.0,
                expectancy: 400.0,
                max_bars_held: 0,
                min_bars_held: 0,
                long_trades: 0,
                short_trades: 0,
                long_win_rate: 0.0,
                short_win_rate: 0.0,
                return_std: 0.0,
                downside_std: 0.0,
            },
            trades: vec![],
            total_bars: 0,
            monthly_returns: std::collections::HashMap::new(),
            trade_pnl_distribution: TradePnlDistribution {
                wins: 30,
                losses: 20,
                max_win: 0.02,
                max_loss: 0.01,
                avg_win: 0.02,
                avg_loss: 0.01,
                win_loss_ratio: 2.0,
                win_buckets: vec![],
                loss_buckets: vec![],
            },
        };

        let metrics = WfMetrics::from_backtest_result(&bt, 252);

        assert!(
            (metrics.total_return - 0.20).abs() < 0.001,
            "total_return mismatch: {}",
            metrics.total_return
        );
        assert!(
            metrics.annualized_return > 0.19 && metrics.annualized_return < 0.21,
            "annualized_return: {}",
            metrics.annualized_return
        );
        assert!((metrics.max_drawdown - 0.10).abs() < 0.001);
        assert!((metrics.sharpe_ratio - 1.2).abs() < 0.001);
        assert!(
            (metrics.win_rate - 0.60).abs() < 0.01,
            "win_rate: {}",
            metrics.win_rate
        );
        assert!(metrics.trade_count == 50);
    }

    #[test]
    fn test_wf_summary_empty() {
        let empty_periods: Vec<WfPeriodResult> = vec![];
        let summary = WalkForwardAnalyzer::compute_summary(&empty_periods);

        assert_eq!(summary.total_periods, 0);
        assert_eq!(summary.avg_test_return, 0.0);
        assert_eq!(summary.oos_r_squared, 0.0);
    }
}
