/// 参数优化框架
/// 包括：网格搜索、Walk-Forward Analysis、过拟合检测、参数敏感性分析
use super::data::Candle;
use super::report::BacktestResult;
use super::strategy::Strategy;
use std::collections::HashMap;

/// 参数网格定义
#[derive(Debug, Clone)]
pub struct ParameterGrid {
    pub name: String,
    pub values: Vec<f64>,
}

impl ParameterGrid {
    pub fn new(name: &str, values: Vec<f64>) -> Self {
        Self {
            name: name.to_string(),
            values,
        }
    }

    /// 创建整数范围的参数网格
    pub fn range(name: &str, start: usize, end: usize, step: usize) -> Self {
        let values: Vec<f64> = (start..=end).step_by(step).map(|x| x as f64).collect();
        Self {
            name: name.to_string(),
            values,
        }
    }
}

/// 参数组合
#[derive(Debug, Clone)]
pub struct ParameterSet {
    pub parameters: HashMap<String, f64>,
}

impl Default for ParameterSet {
    fn default() -> Self {
        Self::new()
    }
}

impl ParameterSet {
    pub fn new() -> Self {
        Self {
            parameters: HashMap::new(),
        }
    }

    pub fn with(mut self, name: &str, value: f64) -> Self {
        self.parameters.insert(name.to_string(), value);
        self
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        self.parameters.get(name).copied()
    }

    pub fn display(&self) -> String {
        self.parameters
            .iter()
            .map(|(k, v)| format!("{}={:.1}", k, v))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// 参数优化结果
#[derive(Debug, Clone)]
pub struct ParameterOptimizationResult {
    /// 所有参数组合的回测结果
    pub results: Vec<(ParameterSet, BacktestResult)>,
    /// 最优参数组合
    pub best_parameters: ParameterSet,
    /// 最优结果
    pub best_result: BacktestResult,
    /// 参数敏感性分析
    pub sensitivity: HashMap<String, SensitivityAnalysis>,
    /// 过拟合指标
    pub overfitting_metrics: OverfittingMetrics,
}

/// 敏感性分析结果
#[derive(Debug, Clone)]
pub struct SensitivityAnalysis {
    pub parameter_name: String,
    /// 参数值与夏普比率的相关性
    pub correlation_with_sharpe: f64,
    /// 参数值与最大回撤的相关性
    pub correlation_with_drawdown: f64,
    /// 最优参数附近的性能变化率
    pub local_sensitivity: f64,
}

/// 过拟合指标
#[derive(Debug, Clone)]
pub struct OverfittingMetrics {
    /// 参数数量
    pub num_parameters: usize,
    /// 测试次数
    pub num_tests: usize,
    /// 概率过拟合 (PBO) - 基于交叉验证
    pub probability_backtest_overfitting: f64,
    /// 过拟合风险评分 (0-1, 越高越危险)
    pub overfitting_risk_score: f64,
    /// 样本内 vs 样本外性能差异
    pub in_sample_vs_oos_gap: f64,
    /// 参数稳定性评分 (0-1, 越高越稳定)
    pub parameter_stability_score: f64,
}

/// 网格搜索优化器
pub struct GridSearchOptimizer {
    /// 优化目标指标
    pub optimization_target: OptimizationTarget,
    /// 最大回撤阈值 (用于过滤)
    pub max_drawdown_threshold: f64,
}

/// 优化目标
#[derive(Debug, Clone)]
pub enum OptimizationTarget {
    SharpeRatio,
    SortinoRatio,
    CalmarRatio,
    TotalReturn,
    ProfitFactor,
}

impl Default for GridSearchOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl GridSearchOptimizer {
    pub fn new() -> Self {
        Self {
            optimization_target: OptimizationTarget::SharpeRatio,
            max_drawdown_threshold: 0.30, // 最大回撤不超过30%
        }
    }

    pub fn with_target(mut self, target: OptimizationTarget) -> Self {
        self.optimization_target = target;
        self
    }

    pub fn with_max_drawdown(mut self, threshold: f64) -> Self {
        self.max_drawdown_threshold = threshold;
        self
    }

    /// 执行网格搜索
    pub fn optimize<F>(
        &self,
        candles: &[Candle],
        param_grids: &[ParameterGrid],
        strategy_builder: F,
        initial_capital: f64,
        commission_rate: f64,
        slippage: f64,
    ) -> ParameterOptimizationResult
    where
        F: Fn(&ParameterSet) -> Box<dyn Strategy>,
    {
        // 生成所有参数组合
        let param_combinations = Self::generate_combinations(param_grids);
        println!("   🔍 网格搜索: {} 种参数组合", param_combinations.len());

        let mut results = Vec::new();

        for (idx, param_set) in param_combinations.iter().enumerate() {
            if idx % 10 == 0 && param_combinations.len() > 50 {
                println!("   进度: {}/{}", idx, param_combinations.len());
            }

            let strategy = strategy_builder(param_set);

            // 回测
            let mut engine =
                super::backtest::BacktestEngine::new(initial_capital, commission_rate, slippage);

            if let Ok(trades) = engine.run(candles, strategy.as_ref()) {
                let result = BacktestResult::from_trades(&trades, initial_capital);
                results.push((param_set.clone(), result));
            }
        }

        // 过滤不满足条件的结果
        let valid_results: Vec<_> = results
            .into_iter()
            .filter(|(_, r)| r.engine.max_drawdown_pct < self.max_drawdown_threshold)
            .collect();

        if valid_results.is_empty() {
            panic!("没有满足最大回撤约束的参数组合");
        }

        // 找到最优参数
        let best_idx = valid_results
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                self.target_value(&a.1)
                    .partial_cmp(&self.target_value(&b.1))
                    .unwrap()
            })
            .map(|(i, _)| i)
            .unwrap_or(0);

        let (best_params, best_result) = valid_results[best_idx].clone();

        // 敏感性分析
        let sensitivity = Self::analyze_sensitivity(&valid_results, param_grids);

        // 过拟合检测
        let overfitting = Self::detect_overfitting(&valid_results, param_grids.len());

        ParameterOptimizationResult {
            results: valid_results,
            best_parameters: best_params,
            best_result,
            sensitivity,
            overfitting_metrics: overfitting,
        }
    }

    fn target_value(&self, result: &BacktestResult) -> f64 {
        match self.optimization_target {
            OptimizationTarget::SharpeRatio => result.engine.sharpe_ratio,
            OptimizationTarget::SortinoRatio => result.engine.sharpe_ratio,
            OptimizationTarget::CalmarRatio => {
                if result.engine.max_drawdown_pct > 0.0 {
                    result.engine.total_return_pct / result.engine.max_drawdown_pct
                } else {
                    result.engine.total_return_pct
                }
            }
            OptimizationTarget::TotalReturn => result.engine.total_return_pct / 100.0,
            OptimizationTarget::ProfitFactor => result.engine.profit_factor,
        }
    }

    /// 生成所有参数组合 (笛卡尔积)
    fn generate_combinations(grids: &[ParameterGrid]) -> Vec<ParameterSet> {
        if grids.is_empty() {
            return vec![ParameterSet::new()];
        }

        let mut combinations = vec![ParameterSet::new()];

        for grid in grids {
            let mut new_combinations = Vec::new();
            for combo in &combinations {
                for &value in &grid.values {
                    let mut new_combo = combo.clone();
                    new_combo.parameters.insert(grid.name.clone(), value);
                    new_combinations.push(new_combo);
                }
            }
            combinations = new_combinations;
        }

        combinations
    }

    /// 参数敏感性分析
    fn analyze_sensitivity(
        results: &[(ParameterSet, BacktestResult)],
        grids: &[ParameterGrid],
    ) -> HashMap<String, SensitivityAnalysis> {
        let mut sensitivity = HashMap::new();

        for grid in grids {
            let param_name = &grid.name;

            // 收集参数值和对应性能
            let mut values = Vec::new();
            let mut sharpes = Vec::new();
            let mut drawdowns = Vec::new();

            for (params, result) in results {
                if let Some(&value) = params.parameters.get(param_name) {
                    values.push(value);
                    sharpes.push(result.engine.sharpe_ratio);
                    drawdowns.push(result.engine.max_drawdown_pct);
                }
            }

            if values.len() < 3 {
                continue;
            }

            let correlation_sharpe = Self::pearson_correlation(&values, &sharpes);
            let correlation_dd = Self::pearson_correlation(&values, &drawdowns);

            // 局部敏感性: 最优值附近 ±1 档的性能变化
            let local_sensitivity = Self::compute_local_sensitivity(&values, &sharpes);

            sensitivity.insert(
                param_name.clone(),
                SensitivityAnalysis {
                    parameter_name: param_name.clone(),
                    correlation_with_sharpe: correlation_sharpe,
                    correlation_with_drawdown: correlation_dd,
                    local_sensitivity,
                },
            );
        }

        sensitivity
    }

    /// Pearson 相关系数
    fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
        if x.len() != y.len() || x.len() < 2 {
            return 0.0;
        }

        let n = x.len() as f64;
        let sum_x: f64 = x.iter().sum();
        let sum_y: f64 = y.iter().sum();
        let sum_xy: f64 = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum();
        let sum_x2: f64 = x.iter().map(|v| v * v).sum();
        let sum_y2: f64 = y.iter().map(|v| v * v).sum();

        let numerator = n * sum_xy - sum_x * sum_y;
        let denominator = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();

        if denominator < 1e-10 {
            return 0.0;
        }

        numerator / denominator
    }

    /// 局部敏感性: 最优值附近的性能变化率
    fn compute_local_sensitivity(_values: &[f64], sharpes: &[f64]) -> f64 {
        // 找到最优夏普对应的值
        let best_idx = sharpes
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        let best_sharpe = sharpes[best_idx];

        // 检查相邻值的变化
        let mut sensitivity_sum = 0.0;
        let mut count = 0;

        for &diff in &[-1, 1] {
            let neighbor_idx = best_idx as isize + diff;
            if neighbor_idx >= 0 && neighbor_idx < sharpes.len() as isize {
                let neighbor_sharpe = sharpes[neighbor_idx as usize];
                sensitivity_sum += (best_sharpe - neighbor_sharpe).abs();
                count += 1;
            }
        }

        if count == 0 {
            return 0.0;
        }

        sensitivity_sum / count as f64
    }

    /// 过拟合检测
    fn detect_overfitting(
        results: &[(ParameterSet, BacktestResult)],
        num_params: usize,
    ) -> OverfittingMetrics {
        let num_tests = results.len();

        // 概率过拟合 (简化版 PBO)
        // 基于: 如果最优参数在交叉验证中的表现远差于样本内，则可能过拟合
        let sharpes: Vec<f64> = results.iter().map(|(_, r)| r.engine.sharpe_ratio).collect();
        let mean_sharpe = sharpes.iter().sum::<f64>() / sharpes.len() as f64;
        let max_sharpe = sharpes.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        // 过拟合风险: 最优结果远好于平均值
        let overfitting_risk = if mean_sharpe > 0.0 {
            ((max_sharpe - mean_sharpe) / mean_sharpe).clamp(0.0, 1.0)
        } else {
            0.5
        };

        // 参数稳定性: 前10%结果中参数值的变异系数
        let mut sorted_results = results.to_vec();
        sorted_results.sort_by(|a, b| {
            b.1.engine
                .sharpe_ratio
                .partial_cmp(&a.1.engine.sharpe_ratio)
                .unwrap()
        });

        let top_n = (results.len() as f64 * 0.1).ceil().max(3.0) as usize;
        let top_results = &sorted_results[..top_n.min(results.len())];

        // 计算参数稳定性 (简化)
        let parameter_stability = Self::compute_parameter_stability(top_results);

        OverfittingMetrics {
            num_parameters: num_params,
            num_tests,
            probability_backtest_overfitting: overfitting_risk,
            overfitting_risk_score: overfitting_risk,
            in_sample_vs_oos_gap: 0.0, // 需要WFA才能计算
            parameter_stability_score: parameter_stability,
        }
    }

    fn compute_parameter_stability(results: &[(ParameterSet, BacktestResult)]) -> f64 {
        if results.is_empty() {
            return 0.0;
        }

        // 计算所有参数的变异系数 (CV = std/mean)
        let param_names: Vec<&String> = results[0].0.parameters.keys().collect();
        let mut cvs = Vec::new();

        for name in param_names {
            let values: Vec<f64> = results
                .iter()
                .filter_map(|(p, _)| p.parameters.get(name).copied())
                .collect();

            if values.len() < 2 {
                continue;
            }

            let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
            if mean == 0.0 {
                continue;
            }

            let variance: f64 =
                values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
            let std = variance.sqrt();
            let cv = std / mean.abs();

            cvs.push(cv);
        }

        if cvs.is_empty() {
            return 1.0;
        }

        // 稳定性 = 1 - 平均CV (CV越小越稳定)
        let avg_cv = cvs.iter().sum::<f64>() / cvs.len() as f64;
        (1.0 - avg_cv).clamp(0.0, 1.0)
    }
}

/// Walk-Forward Analysis (WFA)
pub struct WalkForwardAnalyzer {
    /// 样本内窗口大小 (条数)
    pub in_sample_window: usize,
    /// 样本外窗口大小 (条数)
    pub out_of_sample_window: usize,
    /// 滚动步长
    pub step_size: usize,
    /// 参数网格
    pub param_grids: Vec<ParameterGrid>,
}

impl WalkForwardAnalyzer {
    pub fn new(
        in_sample: usize,
        out_of_sample: usize,
        step: usize,
        grids: Vec<ParameterGrid>,
    ) -> Self {
        Self {
            in_sample_window: in_sample,
            out_of_sample_window: out_of_sample,
            step_size: step,
            param_grids: grids,
        }
    }

    /// 执行 Walk-Forward Analysis
    pub fn analyze<F>(
        &self,
        candles: &[Candle],
        strategy_builder: F,
        initial_capital: f64,
        commission_rate: f64,
        slippage: f64,
    ) -> Vec<WfaPeriodResult>
    where
        F: Fn(&ParameterSet) -> Box<dyn Strategy>,
    {
        let total = candles.len();
        if total < self.in_sample_window + self.out_of_sample_window {
            panic!("数据不足以进行WFA分析");
        }

        let mut period_results = Vec::new();
        let mut start = 0;

        loop {
            let is_end = start + self.in_sample_window + self.out_of_sample_window;
            if is_end > total {
                break;
            }

            let is_start = start;
            let is_end_idx = start + self.in_sample_window;
            let oos_start = is_end_idx;
            let oos_end = is_end;

            // 样本内优化
            let is_candles = &candles[is_start..is_end_idx];
            let optimizer = GridSearchOptimizer::new().with_target(OptimizationTarget::SharpeRatio);

            let opt_result = optimizer.optimize(
                is_candles,
                &self.param_grids,
                &strategy_builder,
                initial_capital,
                commission_rate,
                slippage,
            );

            // 样本外测试 (使用样本内最优参数)
            let oos_candles = &candles[oos_start..oos_end];
            let oos_strategy = strategy_builder(&opt_result.best_parameters);

            let mut oos_engine =
                super::backtest::BacktestEngine::new(initial_capital, commission_rate, slippage);

            let oos_trades = oos_engine
                .run(oos_candles, oos_strategy.as_ref())
                .unwrap_or_default();
            let oos_result = BacktestResult::from_trades(&oos_trades, initial_capital);

            // 计算样本内样本外差异
            let gap = opt_result.best_result.engine.sharpe_ratio - oos_result.engine.sharpe_ratio;

            period_results.push(WfaPeriodResult {
                period_start: is_start,
                period_end: oos_end,
                in_sample_result: opt_result.best_result.clone(),
                out_of_sample_result: oos_result,
                best_parameters: opt_result.best_parameters,
                sharpe_gap: gap,
            });

            start += self.step_size;
        }

        period_results
    }
}

/// WFA 单期结果
#[derive(Debug, Clone)]
pub struct WfaPeriodResult {
    pub period_start: usize,
    pub period_end: usize,
    pub in_sample_result: BacktestResult,
    pub out_of_sample_result: BacktestResult,
    pub best_parameters: ParameterSet,
    pub sharpe_gap: f64,
}

impl WfaPeriodResult {
    /// 是否过拟合 (样本外夏普显著低于样本内)
    pub fn is_overfitting(&self, threshold: f64) -> bool {
        self.sharpe_gap > threshold
    }
}

/// 参数优化报告
pub struct OptimizationReport;

impl OptimizationReport {
    pub fn generate(result: &ParameterOptimizationResult, top_n: usize) -> String {
        let mut report = String::new();

        report.push_str("# 参数优化报告\n\n");

        report.push_str("## 最优参数\n");
        report.push_str(&format!("{}\n\n", result.best_parameters.display()));

        report.push_str("### 最优结果\n");
        report.push_str(&format!(
            "- 夏普比率: {:.2}\n",
            result.best_result.engine.sharpe_ratio
        ));
        report.push_str(&format!(
            "- 总收益率: {:.2}%\n",
            result.best_result.engine.total_return_pct
        ));
        report.push_str(&format!(
            "- 最大回撤: {:.2}%\n",
            result.best_result.engine.max_drawdown_pct
        ));
        report.push_str(&format!(
            "- 胜率: {:.2}%\n",
            result.best_result.engine.win_rate * 100.0
        ));
        report.push_str(&format!(
            "- 盈利因子: {:.2}\n",
            result.best_result.engine.profit_factor
        ));

        report.push_str("\n## 过拟合检测\n\n");
        let m = &result.overfitting_metrics;
        report.push_str(&format!("- 参数数量: {}\n", m.num_parameters));
        report.push_str(&format!("- 测试次数: {}\n", m.num_tests));
        report.push_str(&format!(
            "- 过拟合风险评分: {:.2} (0=无风险, 1=高风险)\n",
            m.overfitting_risk_score
        ));
        report.push_str(&format!(
            "- 参数稳定性评分: {:.2} (0=不稳定, 1=稳定)\n",
            m.parameter_stability_score
        ));

        report.push_str("\n## 参数敏感性\n\n");
        for (name, analysis) in &result.sensitivity {
            report.push_str(&format!("### {}\n", name));
            report.push_str(&format!(
                "- 与夏普比率相关性: {:.3}\n",
                analysis.correlation_with_sharpe
            ));
            report.push_str(&format!(
                "- 与最大回撤相关性: {:.3}\n",
                analysis.correlation_with_drawdown
            ));
            report.push_str(&format!(
                "- 局部敏感性: {:.3}\n",
                analysis.local_sensitivity
            ));
        }

        report.push_str("\n## Top 参数组合\n\n");
        report.push_str("| 排名 | 参数 | 夏普 | 收益率% | 回撤% | 胜率% |\n");
        report.push_str("|------|------|------|---------|-------|-------|\n");

        let mut sorted = result.results.clone();
        sorted.sort_by(|a, b| {
            b.1.engine
                .sharpe_ratio
                .partial_cmp(&a.1.engine.sharpe_ratio)
                .unwrap()
        });

        for (i, (params, r)) in sorted.iter().take(top_n).enumerate() {
            report.push_str(&format!(
                "| {} | {} | {:.2} | {:.2} | {:.2} | {:.1} |\n",
                i + 1,
                params.display(),
                r.engine.sharpe_ratio,
                r.engine.total_return_pct,
                r.engine.max_drawdown_pct,
                r.engine.win_rate * 100.0
            ));
        }

        report
    }
}
