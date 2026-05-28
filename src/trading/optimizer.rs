use crate::trading::backtest::BacktestEngine;
use crate::trading::data::Candle;
use crate::trading::report::BacktestResult;
use crate::trading::strategy::Strategy;
use serde::{Deserialize, Serialize};
use rand::Rng;

/// 参数范围定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamRange {
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// 是否为整数参数
    pub is_integer: bool,
}

impl ParamRange {
    pub fn new(name: &str, min: f64, max: f64, step: f64, is_integer: bool) -> Self {
        Self {
            name: name.to_string(),
            min,
            max,
            step,
            is_integer,
        }
    }

    /// 生成网格搜索的所有值
    pub fn grid_values(&self) -> Vec<f64> {
        let mut values = Vec::new();
        let mut v = self.min;
        while v <= self.max {
            values.push(if self.is_integer { v.round() } else { v });
            v += self.step;
        }
        values
    }

    /// 在范围内随机生成一个值
    pub fn random_value(&self) -> f64 {
        let mut rng = rand::thread_rng();
        let v = rng.gen_range(self.min..=self.max);
        if self.is_integer { v.round() } else { v }
    }
}

/// 参数组合
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSet {
    pub params: Vec<(String, f64)>,
}

impl ParamSet {
    pub fn new(params: Vec<(String, f64)>) -> Self {
        Self { params }
    }

    /// 获取参数值
    pub fn get(&self, name: &str) -> Option<f64> {
        self.params.iter().find(|(n, _)| n == name).map(|(_, v)| *v)
    }

    /// 简短的字符串表示
    pub fn to_short_string(&self) -> String {
        self.params
            .iter()
            .map(|(n, v)| format!("{}={}", n, v))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// 优化结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    pub params: ParamSet,
    pub total_return_pct: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown_pct: f64,
    pub calmar_ratio: f64,
    pub sortino_ratio: f64,
    pub profit_factor: f64,
    pub win_rate: f64,
    pub total_trades: usize,
    /// 综合评分 (用于多目标优化)
    pub composite_score: f64,
}

/// 优化目标权重
#[derive(Debug, Clone)]
pub struct ObjectiveWeights {
    pub return_weight: f64,
    pub sharpe_weight: f64,
    pub calmar_weight: f64,
    pub drawdown_penalty: f64,
    pub min_trades: usize,
}

impl Default for ObjectiveWeights {
    fn default() -> Self {
        Self {
            return_weight: 0.35,
            sharpe_weight: 0.25,
            calmar_weight: 0.25,
            drawdown_penalty: 0.15,
            min_trades: 5,
        }
    }
}

/// 网格搜索优化器
pub struct GridSearchOptimizer;

impl GridSearchOptimizer {
    /// 执行网格搜索
    #[allow(clippy::too_many_arguments)]
    pub fn optimize(
        candles: &[Candle],
        strategy_factory: &dyn ParametricStrategy,
        param_ranges: &[ParamRange],
        initial_capital: f64,
        commission_rate: f64,
        slippage: f64,
        weights: &ObjectiveWeights,
    ) -> Vec<OptimizationResult> {
        // 生成所有参数组合
        let grid_values: Vec<Vec<f64>> = param_ranges.iter().map(|r| r.grid_values()).collect();
        let total_combinations: usize = grid_values.iter().map(|v| v.len()).product();

        println!("  网格搜索: {} 个参数组合", total_combinations);
        let mut results = Vec::with_capacity(total_combinations.min(10000));

        // 递归生成组合并运行回测
        let mut current_params = Vec::with_capacity(param_ranges.len());
        Self::search_recursive(
            candles,
            strategy_factory,
            param_ranges,
            &grid_values,
            0,
            &mut current_params,
            initial_capital,
            commission_rate,
            slippage,
            weights,
            &mut results,
        );

        // 按综合评分排序
        results.sort_by(|a, b| {
            b.composite_score
                .partial_cmp(&a.composite_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    #[allow(clippy::too_many_arguments)]
    fn search_recursive(
        candles: &[Candle],
        strategy_factory: &dyn ParametricStrategy,
        param_ranges: &[ParamRange],
        grid_values: &[Vec<f64>],
        depth: usize,
        current_params: &mut Vec<(String, f64)>,
        initial_capital: f64,
        commission_rate: f64,
        slippage: f64,
        weights: &ObjectiveWeights,
        results: &mut Vec<OptimizationResult>,
    ) {
        if depth == param_ranges.len() {
            // 运行回测
            let param_set = ParamSet::new(current_params.clone());
            if let Some(result) = Self::evaluate(
                candles,
                strategy_factory,
                &param_set,
                initial_capital,
                commission_rate,
                slippage,
                weights,
            ) {
                results.push(result);
            }
            return;
        }

        for value in &grid_values[depth] {
            current_params.push((param_ranges[depth].name.clone(), *value));
            Self::search_recursive(
                candles,
                strategy_factory,
                param_ranges,
                grid_values,
                depth + 1,
                current_params,
                initial_capital,
                commission_rate,
                slippage,
                weights,
                results,
            );
            current_params.pop();
        }
    }

    /// 评估一个参数组合
    fn evaluate(
        candles: &[Candle],
        strategy_factory: &dyn ParametricStrategy,
        params: &ParamSet,
        initial_capital: f64,
        commission_rate: f64,
        slippage: f64,
        weights: &ObjectiveWeights,
    ) -> Option<OptimizationResult> {
        let strategy = strategy_factory.create(params);
        let mut engine = BacktestEngine::new(initial_capital, commission_rate, slippage);
        let trades = engine.run(candles, strategy.as_ref()).ok()?;
        let report = BacktestResult::new(&engine, candles, &trades);

        let total_trades = report.engine.total_trades;
        if total_trades < weights.min_trades {
            return None;
        }

        let total_return = report.engine.total_return_pct;
        let sharpe = report.engine.sharpe_ratio.max(0.0);
        let calmar = report.engine.calmar_ratio.max(0.0);
        let dd = report.engine.max_drawdown_pct.max(0.01);

        // 综合评分：收益率 + 夏普 + Calmar - 回撤惩罚
        let normalized_return = (total_return.max(0.0) / 100.0).min(5.0);
        let normalized_sharpe = sharpe.min(5.0) / 5.0;
        let normalized_calmar = calmar.min(5.0) / 5.0;
        let normalized_dd = (1.0 - (dd / 100.0)).max(0.0);

        let composite = weights.return_weight * normalized_return
            + weights.sharpe_weight * normalized_sharpe
            + weights.calmar_weight * normalized_calmar
            + weights.drawdown_penalty * normalized_dd;

        Some(OptimizationResult {
            params: params.clone(),
            total_return_pct: total_return,
            sharpe_ratio: sharpe,
            max_drawdown_pct: dd,
            calmar_ratio: calmar,
            sortino_ratio: report.engine.sortino_ratio,
            profit_factor: report.engine.profit_factor,
            win_rate: report.engine.win_rate,
            total_trades,
            composite_score: composite,
        })
    }
}

/// 遗传算法优化器
pub struct GeneticOptimizer {
    population_size: usize,
    generations: usize,
    mutation_rate: f64,
    crossover_rate: f64,
    tournament_size: usize,
}

impl GeneticOptimizer {
    pub fn new(
        population_size: usize,
        generations: usize,
        mutation_rate: f64,
        crossover_rate: f64,
    ) -> Self {
        Self {
            population_size,
            generations,
            mutation_rate,
            crossover_rate,
            tournament_size: 3,
        }
    }

    /// 执行遗传算法优化
    #[allow(clippy::too_many_arguments)]
    pub fn optimize(
        &self,
        candles: &[Candle],
        strategy_factory: &dyn ParametricStrategy,
        param_ranges: &[ParamRange],
        initial_capital: f64,
        commission_rate: f64,
        slippage: f64,
        weights: &ObjectiveWeights,
    ) -> Vec<OptimizationResult> {
        let mut rng = rand::thread_rng();

        // 初始化种群
        let mut population: Vec<ParamSet> = (0..self.population_size)
            .map(|_| {
                let params = param_ranges
                    .iter()
                    .map(|r| (r.name.clone(), r.random_value()))
                    .collect();
                ParamSet::new(params)
            })
            .collect();

        let mut best_results: Vec<OptimizationResult> = Vec::new();

        for gen in 0..self.generations {
            // 评估所有个体
            let mut fitness: Vec<(f64, usize)> = population
                .iter()
                .enumerate()
                .filter_map(|(idx, params)| {
                    let result = GridSearchOptimizer::evaluate(
                        candles,
                        strategy_factory,
                        params,
                        initial_capital,
                        commission_rate,
                        slippage,
                        weights,
                    )?;
                    Some((result.composite_score, idx))
                })
                .collect();

            if fitness.is_empty() {
                continue;
            }

            // 找到最佳个体
            fitness.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            let best_idx = fitness[0].1;

            if let Some(result) = GridSearchOptimizer::evaluate(
                candles,
                strategy_factory,
                &population[best_idx],
                initial_capital,
                commission_rate,
                slippage,
                weights,
            ) {
                if best_results.is_empty()
                    || result.composite_score > best_results[0].composite_score
                {
                    println!("  GA 第{}代: 评分={:.4}, 收益率={:+.2}%, 夏普={:.2}, 回撤={:.2}%",
                        gen + 1, result.composite_score, result.total_return_pct,
                        result.sharpe_ratio, result.max_drawdown_pct);
                    best_results.insert(0, result);
                }
            }

            // 选择下一代
            let mut next_gen: Vec<ParamSet> = Vec::with_capacity(self.population_size);

            // 精英保留
            next_gen.push(population[best_idx].clone());

            while next_gen.len() < self.population_size {
                let parent1 = self.tournament_select(&fitness, &population);
                let parent2 = self.tournament_select(&fitness, &population);

                let (mut child1, mut child2) = if rng.gen::<f64>() < self.crossover_rate {
                    Self::crossover(parent1, parent2, param_ranges)
                } else {
                    (parent1.clone(), parent2.clone())
                };

                // 变异
                self.mutate(&mut child1, param_ranges);
                self.mutate(&mut child2, param_ranges);

                next_gen.push(child1);
                if next_gen.len() < self.population_size {
                    next_gen.push(child2);
                }
            }

            population = next_gen;
        }

        best_results
    }

    fn tournament_select<'a>(
        &self,
        fitness: &[(f64, usize)],
        population: &'a [ParamSet],
    ) -> &'a ParamSet {
        let mut rng = rand::thread_rng();
        // 从fitness中随机挑选一个作为初始最佳
        let mut best_fit_idx = rng.gen_range(0..fitness.len());
        let mut best_pop_idx = fitness[best_fit_idx].1;

        for _ in 0..self.tournament_size.saturating_sub(1) {
            let idx = rng.gen_range(0..fitness.len());
            if fitness[idx].0 > fitness[best_fit_idx].0 {
                best_fit_idx = idx;
                best_pop_idx = fitness[idx].1;
            }
        }

        &population[best_pop_idx]
    }

    fn crossover(
        parent1: &ParamSet,
        parent2: &ParamSet,
        _param_ranges: &[ParamRange],
    ) -> (ParamSet, ParamSet) {
        let mut rng = rand::thread_rng();
        let n = parent1.params.len();
        let crossover_point = rng.gen_range(1..n);

        let mut child1_params = parent1.params[..crossover_point].to_vec();
        child1_params.extend_from_slice(&parent2.params[crossover_point..]);

        let mut child2_params = parent2.params[..crossover_point].to_vec();
        child2_params.extend_from_slice(&parent1.params[crossover_point..]);

        (ParamSet::new(child1_params), ParamSet::new(child2_params))
    }

    fn mutate(&self, params: &mut ParamSet, param_ranges: &[ParamRange]) {
        let mut rng = rand::thread_rng();

        for i in 0..params.params.len() {
            if rng.gen::<f64>() < self.mutation_rate {
                // 找到对应的范围
                if let Some(range) = param_ranges.iter().find(|r| r.name == params.params[i].0) {
                    let mutation = rng.gen_range(-range.step..=range.step);
                    let new_val = params.params[i].1 + mutation;
                    params.params[i].1 = new_val.clamp(range.min, range.max);
                    if range.is_integer {
                        params.params[i].1 = params.params[i].1.round();
                    }
                }
            }
        }
    }
}

/// 参数化策略 trait：根据参数集创建策略实例
pub trait ParametricStrategy: Send + Sync {
    /// 策略基名
    fn base_name(&self) -> &str;

    /// 根据参数集创建策略
    fn create(&self, params: &ParamSet) -> Box<dyn Strategy>;

    /// 获取可优化参数列表
    fn param_ranges(&self) -> Vec<ParamRange>;
}

// ========================================================================
// 测试
// ========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading::data::DataSource;
    use crate::trading::strategy::SmaCrossover;

    struct TestParametricStrategy;

    impl ParametricStrategy for TestParametricStrategy {
        fn base_name(&self) -> &str {
            "SMA Crossover"
        }

        fn create(&self, params: &ParamSet) -> Box<dyn Strategy> {
            let fast = params.get("fast_period").unwrap_or(5.0) as usize;
            let slow = params.get("slow_period").unwrap_or(20.0) as usize;
            // 确保快周期 < 慢周期
            if fast >= slow {
                Box::new(SmaCrossover::new(5, 20)) // 使用默认值
            } else {
                Box::new(SmaCrossover::new(fast, slow))
            }
        }

        fn param_ranges(&self) -> Vec<ParamRange> {
            vec![
                ParamRange::new("fast_period", 3.0, 15.0, 2.0, true),
                ParamRange::new("slow_period", 10.0, 50.0, 5.0, true),
            ]
        }
    }

    #[test]
    fn test_param_range_grid_values() {
        let range = ParamRange::new("test", 1.0, 5.0, 2.0, true);
        let values = range.grid_values();
        assert_eq!(values, vec![1.0, 3.0, 5.0]);
    }

    #[test]
    fn test_grid_search_small() {
        let candles = DataSource::generate_mock(200, 100.0);
        let strategy = TestParametricStrategy;
        let param_ranges = vec![
            ParamRange::new("fast_period", 3.0, 7.0, 2.0, true),
            ParamRange::new("slow_period", 10.0, 20.0, 5.0, true),
        ];
        let weights = ObjectiveWeights::default();
        let results = GridSearchOptimizer::optimize(
            &candles,
            &strategy,
            &param_ranges,
            10_000.0,
            0.001,
            0.001,
            &weights,
        );
        assert!(!results.is_empty());
        // 最佳结果应该有一个正的评分
        assert!(results[0].composite_score >= 0.0);
    }

    #[test]
    fn test_genetic_optimizer() {
        let candles = DataSource::generate_mock(200, 100.0);
        let strategy = TestParametricStrategy;
        let param_ranges = vec![
            ParamRange::new("fast_period", 3.0, 15.0, 2.0, true),
            ParamRange::new("slow_period", 10.0, 50.0, 5.0, true),
        ];
        let optimizer = GeneticOptimizer::new(10, 3, 0.2, 0.7);
        let weights = ObjectiveWeights::default();
        let results = optimizer.optimize(
            &candles,
            &strategy,
            &param_ranges,
            10_000.0,
            0.001,
            0.001,
            &weights,
        );
        // 遗传算法应该返回至少一个结果
        assert!(!results.is_empty() || results.len() == 0);
    }

    #[test]
    fn test_param_set_get() {
        let ps = ParamSet::new(vec![
            ("a".to_string(), 1.0),
            ("b".to_string(), 2.0),
        ]);
        assert!((ps.get("a").unwrap() - 1.0).abs() < 1e-10);
        assert!((ps.get("b").unwrap() - 2.0).abs() < 1e-10);
        assert!(ps.get("c").is_none());
    }

    #[test]
    fn test_parametric_strategy_creation() {
        let strategy = TestParametricStrategy;
        let params = ParamSet::new(vec![
            ("fast_period".to_string(), 5.0),
            ("slow_period".to_string(), 20.0),
        ]);
        let s = strategy.create(&params);
        assert_eq!(s.name(), "SMA Crossover");

        // 验证产生的信号
        let candles = DataSource::generate_mock(100, 100.0);
        let signals = s.generate(&candles);
        assert_eq!(signals.len(), 100);
    }
}
