/// 组合优化模块
/// 包括：Markowitz 均值方差优化、Black-Litterman 模型、有效前沿计算、风险平价
use serde::{Deserialize, Serialize};

/// 资产权重
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetWeight {
    pub symbol: String,
    pub weight: f64,
}

/// 组合优化结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioResult {
    /// 各资产权重
    pub weights: Vec<AssetWeight>,
    /// 预期收益率
    pub expected_return: f64,
    /// 预期波动率（标准差）
    pub expected_volatility: f64,
    /// 夏普比率（无风险利率已扣除）
    pub sharpe_ratio: f64,
    /// 优化方法
    pub method: String,
}

/// 优化目标
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationObjective {
    /// 最大化夏普比率
    MaxSharpe,
    /// 最小波动率
    MinVolatility,
    /// 给定目标收益下的最小波动率
    MinVolatilityForReturn,
    /// 风险平价（等风险贡献）
    RiskParity,
}

/// Markowitz 均值方差优化器
pub struct MarkowitzOptimizer {
    /// 无风险利率
    pub risk_free_rate: f64,
    /// 最大单资产权重上限
    pub max_weight: f64,
    /// 最小单资产权重下限
    pub min_weight: f64,
    /// 目标收益率（仅在 MinVolatilityForReturn 时使用）
    pub target_return: Option<f64>,
}

impl MarkowitzOptimizer {
    pub fn new(risk_free_rate: f64) -> Self {
        Self {
            risk_free_rate,
            max_weight: 0.40,
            min_weight: 0.0,
            target_return: None,
        }
    }

    pub fn with_weight_limits(mut self, min: f64, max: f64) -> Self {
        self.min_weight = min;
        self.max_weight = max;
        self
    }

    pub fn with_target_return(mut self, target: f64) -> Self {
        self.target_return = Some(target);
        self
    }

    /// 使用简化梯度投影法进行均值方差优化
    pub fn optimize(
        &self,
        expected_returns: &[f64],
        cov_matrix: &[f64],
        symbols: &[String],
        objective: OptimizationObjective,
    ) -> PortfolioResult {
        let n = expected_returns.len();
        assert_eq!(cov_matrix.len(), n * n, "协方差矩阵维度不匹配");
        assert_eq!(symbols.len(), n, "资产数量不匹配");

        let weights = match objective {
            OptimizationObjective::MaxSharpe => {
                self.maximize_sharpe(expected_returns, cov_matrix, n)
            }
            OptimizationObjective::MinVolatility => self.minimize_volatility(cov_matrix, n),
            OptimizationObjective::MinVolatilityForReturn => {
                let target = self.target_return.unwrap_or(0.10);
                self.min_volatility_for_return(expected_returns, cov_matrix, n, target)
            }
            OptimizationObjective::RiskParity => self.risk_parity(cov_matrix, n),
        };

        let (ret, vol) = Self::portfolio_stats(&weights, expected_returns, cov_matrix, n);
        let sharpe = if vol > 1e-10 {
            (ret - self.risk_free_rate) / vol
        } else {
            0.0
        };

        let method = match objective {
            OptimizationObjective::MaxSharpe => "Max Sharpe Ratio",
            OptimizationObjective::MinVolatility => "Minimum Volatility",
            OptimizationObjective::MinVolatilityForReturn => "Min Vol for Target Return",
            OptimizationObjective::RiskParity => "Risk Parity",
        };

        PortfolioResult {
            weights: symbols
                .iter()
                .zip(weights.iter())
                .map(|(s, &w)| AssetWeight {
                    symbol: s.clone(),
                    weight: w,
                })
                .collect(),
            expected_return: ret,
            expected_volatility: vol,
            sharpe_ratio: sharpe,
            method: method.to_string(),
        }
    }

    /// 最大化夏普比率（迭代投影梯度法）
    fn maximize_sharpe(&self, returns: &[f64], cov: &[f64], n: usize) -> Vec<f64> {
        let mut best_sharpe = f64::NEG_INFINITY;
        let mut best_w = vec![1.0 / n as f64; n];

        // 多起点优化
        for start in 0..10 {
            let mut w = if start == 0 {
                vec![1.0 / n as f64; n]
            } else {
                let mut rng_w = vec![0.0; n];
                let mut sum = 0.0;
                for (ri, r) in rng_w.iter_mut().enumerate() {
                    *r = Self::simple_random(start, ri as u64);
                    sum += *r;
                }
                for r in &mut rng_w {
                    *r /= sum;
                }
                rng_w
            };

            let mut lr = 0.01;
            for iter in 0..500 {
                let port_ret: f64 = w.iter().zip(returns.iter()).map(|(a, b)| a * b).sum();
                let port_var = Self::variance(&w, cov, n);
                let port_vol = port_var.sqrt();

                if port_vol < 1e-10 {
                    break;
                }

                let sharpe = (port_ret - self.risk_free_rate) / port_vol;

                // 计算梯度
                let mut grad = vec![0.0; n];
                for i in 0..n {
                    let cov_i: f64 = w
                        .iter()
                        .enumerate()
                        .map(|(j, &wj)| wj * cov[i * n + j])
                        .sum();
                    grad[i] =
                        (returns[i] - self.risk_free_rate) / port_vol - sharpe * cov_i / port_var;
                }

                // 梯度上升
                for i in 0..n {
                    w[i] += lr * grad[i];
                }

                w = self.project_weights(w);

                if iter % 100 == 99 {
                    lr *= 0.5;
                }

                let new_sharpe = self.compute_sharpe(&w, returns, cov, n);
                if new_sharpe > best_sharpe {
                    best_sharpe = new_sharpe;
                    best_w = w.clone();
                }
            }
        }

        best_w
    }

    /// 最小化波动率
    fn minimize_volatility(&self, cov: &[f64], n: usize) -> Vec<f64> {
        let mut w = vec![1.0 / n as f64; n];
        let mut lr = 0.01;

        for iter in 0..1000 {
            let mut grad = vec![0.0; n];
            for i in 0..n {
                grad[i] = 2.0
                    * w.iter()
                        .enumerate()
                        .map(|(j, &wj)| wj * cov[i * n + j])
                        .sum::<f64>();
            }

            for i in 0..n {
                w[i] -= lr * grad[i];
            }

            w = self.project_weights(w);

            if iter % 100 == 99 {
                lr *= 0.5;
            }
        }

        w
    }

    /// 给定目标收益下的最小波动率
    fn min_volatility_for_return(
        &self,
        returns: &[f64],
        cov: &[f64],
        n: usize,
        target: f64,
    ) -> Vec<f64> {
        let mut w = vec![1.0 / n as f64; n];
        let mut lr = 0.01;
        let lambda = 10.0;

        for iter in 0..1000 {
            let port_ret: f64 = w.iter().zip(returns.iter()).map(|(a, b)| a * b).sum();

            let mut grad = vec![0.0; n];
            for i in 0..n {
                let cov_i: f64 = w
                    .iter()
                    .enumerate()
                    .map(|(j, &wj)| wj * cov[i * n + j])
                    .sum();
                grad[i] = 2.0 * cov_i + lambda * (port_ret - target) * returns[i];
            }

            for i in 0..n {
                w[i] -= lr * grad[i];
            }

            w = self.project_weights(w);

            if iter % 100 == 99 {
                lr *= 0.5;
            }
        }

        w
    }

    /// 风险平价（等风险贡献）
    fn risk_parity(&self, cov: &[f64], n: usize) -> Vec<f64> {
        let mut w = vec![1.0 / n as f64; n];
        let mut lr = 0.001;

        for iter in 0..2000 {
            let port_var = Self::variance(&w, cov, n);
            let port_vol = port_var.sqrt();

            if port_vol < 1e-10 {
                break;
            }

            let mut marginal_risk = vec![0.0; n];
            for i in 0..n {
                marginal_risk[i] = w
                    .iter()
                    .enumerate()
                    .map(|(j, &wj)| wj * cov[i * n + j])
                    .sum::<f64>()
                    / port_vol;
            }

            let risk_contrib: Vec<f64> = w
                .iter()
                .zip(marginal_risk.iter())
                .map(|(&wi, &mri)| wi * mri)
                .collect();

            let target_risk = port_vol / n as f64;

            let mut grad = vec![0.0; n];
            for i in 0..n {
                grad[i] = risk_contrib[i] - target_risk;
            }

            for i in 0..n {
                w[i] -= lr * grad[i];
            }

            w = self.project_weights(w);

            if iter % 200 == 199 {
                lr *= 0.7;
            }

            let max_diff = risk_contrib
                .iter()
                .map(|&rc| (rc - target_risk).abs())
                .fold(0.0, f64::max);
            if max_diff < 1e-6 {
                break;
            }
        }

        w
    }

    fn portfolio_stats(weights: &[f64], returns: &[f64], cov: &[f64], n: usize) -> (f64, f64) {
        let port_ret: f64 = weights.iter().zip(returns.iter()).map(|(a, b)| a * b).sum();
        let port_vol = Self::variance(weights, cov, n).sqrt();
        (port_ret, port_vol)
    }

    fn compute_sharpe(&self, weights: &[f64], returns: &[f64], cov: &[f64], n: usize) -> f64 {
        let (ret, vol) = Self::portfolio_stats(weights, returns, cov, n);
        if vol < 1e-10 {
            0.0
        } else {
            (ret - self.risk_free_rate) / vol
        }
    }

    fn variance(weights: &[f64], cov: &[f64], n: usize) -> f64 {
        let mut var = 0.0;
        for i in 0..n {
            for j in 0..n {
                var += weights[i] * weights[j] * cov[i * n + j];
            }
        }
        var.max(0.0)
    }

    /// 投影权重到约束空间
    fn project_weights(&self, mut w: Vec<f64>) -> Vec<f64> {
        for wi in &mut w {
            *wi = wi.clamp(self.min_weight, self.max_weight);
        }

        let sum: f64 = w.iter().sum();
        if sum > 1e-10 {
            for wi in &mut w {
                *wi /= sum;
            }
        } else {
            let n = w.len();
            for wi in &mut w {
                *wi = 1.0 / n as f64;
            }
        }

        for _ in 0..5 {
            let mut needs_fix = false;
            for wi in &w {
                if *wi < self.min_weight - 1e-10 || *wi > self.max_weight + 1e-10 {
                    needs_fix = true;
                    break;
                }
            }
            if !needs_fix {
                break;
            }
            for wi in &mut w {
                *wi = wi.clamp(self.min_weight, self.max_weight);
            }
            let sum: f64 = w.iter().sum();
            if sum > 1e-10 {
                for wi in &mut w {
                    *wi /= sum;
                }
            }
        }

        w
    }

    fn simple_random(seed: usize, idx: u64) -> f64 {
        let mut x = (seed as u64)
            .wrapping_mul(6364136223846793005)
            .wrapping_add(idx);
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51afd7ed558ccd);
        x ^= x >> 33;
        (x as f64) / (u64::MAX as f64)
    }
}

/// Black-Litterman 模型
/// 结合市场均衡收益与主观观点，生成后验预期收益
pub struct BlackLittermanModel {
    pub risk_aversion: f64,
    pub tau: f64,
}

impl BlackLittermanModel {
    pub fn new(risk_aversion: f64, tau: f64) -> Self {
        Self { risk_aversion, tau }
    }

    /// 计算市场均衡收益（隐含收益）
    /// pi = delta * Sigma * w_market
    pub fn implied_returns(&self, cov_matrix: &[f64], market_weights: &[f64]) -> Vec<f64> {
        let n = market_weights.len();
        let mut pi = vec![0.0; n];

        for i in 0..n {
            let sum: f64 = (0..n)
                .map(|j| cov_matrix[i * n + j] * market_weights[j])
                .sum();
            pi[i] = self.risk_aversion * sum;
        }

        pi
    }

    /// 融合主观观点
    pub fn blend_views(
        &self,
        p: &[Vec<f64>],
        q: &[f64],
        omega: &[Vec<f64>],
        cov_matrix: &[f64],
        market_weights: &[f64],
    ) -> Vec<f64> {
        let n = market_weights.len();
        let k = q.len();

        assert_eq!(p.len(), k, "观点矩阵行数应与观点数量一致");
        for row in p {
            assert_eq!(row.len(), n, "观点矩阵列数应与资产数量一致");
        }

        let pi = self.implied_returns(cov_matrix, market_weights);
        let tau_sigma: Vec<f64> = cov_matrix.iter().map(|&x| self.tau * x).collect();

        let mut posterior = pi.clone();

        for view_idx in 0..k {
            let p_row = &p[view_idx];
            let q_view = q[view_idx];
            let omega_view = omega[view_idx][view_idx];

            let p_sigma_p: f64 = (0..n)
                .map(|i| {
                    (0..n)
                        .map(|j| p_row[i] * tau_sigma[i * n + j] * p_row[j])
                        .sum::<f64>()
                })
                .sum();

            let view_weight = (p_sigma_p + omega_view).max(1e-10);

            let prior_view: f64 = p_row.iter().zip(posterior.iter()).map(|(p, r)| p * r).sum();

            for i in 0..n {
                let adjustment: f64 = (0..n).map(|j| tau_sigma[i * n + j] * p_row[j]).sum::<f64>();
                posterior[i] += adjustment * (q_view - prior_view) / view_weight;
            }
        }

        posterior
    }

    /// 从协方差矩阵计算市场权重（逆优化）
    pub fn implied_weights(&self, expected_returns: &[f64], cov_matrix: &[f64]) -> Vec<f64> {
        let n = expected_returns.len();
        let mut w = vec![0.0; n];

        for i in 0..n {
            let diag = cov_matrix[i * n + i].max(1e-10);
            w[i] = expected_returns[i] / (self.risk_aversion * diag);
        }

        let total: f64 = w.iter().sum();
        if total.abs() > 1e-10 {
            for wi in &mut w {
                *wi = wi.max(0.0) / total;
            }
            let new_sum: f64 = w.iter().sum();
            if new_sum > 1e-10 {
                for wi in &mut w {
                    *wi /= new_sum;
                }
            }
        }

        w
    }
}

/// 有效前沿计算器
pub struct EfficientFrontier;

impl EfficientFrontier {
    /// 计算有效前沿上的多个点
    pub fn compute(
        optimizer: &MarkowitzOptimizer,
        expected_returns: &[f64],
        cov_matrix: &[f64],
        symbols: &[String],
        num_points: usize,
    ) -> Vec<(f64, f64, Vec<AssetWeight>)> {
        let min_vol_result = optimizer.optimize(
            expected_returns,
            cov_matrix,
            symbols,
            OptimizationObjective::MinVolatility,
        );

        let max_return = expected_returns
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        let min_ret = min_vol_result.expected_return;
        let step = (max_return - min_ret) / (num_points as f64 - 1.0).max(1.0);

        let mut frontier = Vec::with_capacity(num_points);

        frontier.push((
            min_vol_result.expected_return,
            min_vol_result.expected_volatility,
            min_vol_result.weights,
        ));

        for i in 1..num_points {
            let target_ret = min_ret + step * i as f64;
            let opt = MarkowitzOptimizer::new(optimizer.risk_free_rate)
                .with_weight_limits(optimizer.min_weight, optimizer.max_weight)
                .with_target_return(target_ret);

            let result = opt.optimize(
                expected_returns,
                cov_matrix,
                symbols,
                OptimizationObjective::MinVolatilityForReturn,
            );

            frontier.push((
                result.expected_return,
                result.expected_volatility,
                result.weights,
            ));
        }

        frontier
    }
}

/// 组合分析工具
pub struct PortfolioAnalyzer;

impl PortfolioAnalyzer {
    /// 计算组合的各资产风险贡献
    pub fn risk_contribution(weights: &[f64], cov_matrix: &[f64]) -> (Vec<f64>, Vec<f64>, f64) {
        let n = weights.len();
        let port_var: f64 = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| weights[i] * weights[j] * cov_matrix[i * n + j])
                    .sum::<f64>()
            })
            .sum();
        let port_vol = port_var.sqrt();

        if port_vol < 1e-10 {
            return (vec![0.0; n], vec![0.0; n], 0.0);
        }

        let mut marginal_risk = vec![0.0; n];
        for i in 0..n {
            marginal_risk[i] = (0..n)
                .map(|j| cov_matrix[i * n + j] * weights[j])
                .sum::<f64>()
                / port_vol;
        }

        let risk_contrib: Vec<f64> = weights
            .iter()
            .zip(marginal_risk.iter())
            .map(|(&w, &m)| w * m)
            .collect();

        let pct_contrib: Vec<f64> = risk_contrib
            .iter()
            .map(|&rc| if port_vol > 1e-10 { rc / port_vol } else { 0.0 })
            .collect();

        (risk_contrib, pct_contrib, port_vol)
    }

    /// 计算组合的 Diversification Ratio
    pub fn diversification_ratio(
        weights: &[f64],
        individual_vols: &[f64],
        cov_matrix: &[f64],
    ) -> f64 {
        let n = weights.len();
        let weighted_avg_vol: f64 = weights
            .iter()
            .zip(individual_vols.iter())
            .map(|(&w, &v)| w * v)
            .sum();

        let port_var: f64 = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| weights[i] * weights[j] * cov_matrix[i * n + j])
                    .sum::<f64>()
            })
            .sum();
        let port_vol = port_var.sqrt();

        if port_vol < 1e-10 {
            return 1.0;
        }

        weighted_avg_vol / port_vol
    }

    /// 生成组合优化报告
    pub fn generate_report(result: &PortfolioResult) -> String {
        let mut report = String::new();
        report.push_str("## 组合优化报告\n\n");
        report.push_str(&format!("**优化方法**: {}\n\n", result.method));
        report.push_str("| 指标 | 值 |\n");
        report.push_str("|------|-----|\n");
        report.push_str(&format!(
            "| 预期年化收益率 | {:.2}% |\n",
            result.expected_return * 100.0
        ));
        report.push_str(&format!(
            "| 预期年化波动率 | {:.2}% |\n",
            result.expected_volatility * 100.0
        ));
        report.push_str(&format!("| 夏普比率 | {:.3} |\n", result.sharpe_ratio));
        report.push_str("\n### 资产权重\n\n");
        report.push_str("| 资产 | 权重 |\n");
        report.push_str("|------|------|\n");
        for aw in &result.weights {
            report.push_str(&format!("| {} | {:.2}% |\n", aw.symbol, aw.weight * 100.0));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> (Vec<f64>, Vec<f64>, Vec<String>) {
        let symbols = vec![
            "沪深300".to_string(),
            "国债指数".to_string(),
            "商品指数".to_string(),
        ];
        let returns = vec![0.10, 0.04, 0.07];
        let cov = vec![0.04, 0.005, 0.012, 0.005, 0.01, 0.003, 0.012, 0.003, 0.03];

        (returns, cov, symbols)
    }

    #[test]
    fn test_max_sharpe() {
        let (returns, cov, symbols) = sample_data();
        let optimizer = MarkowitzOptimizer::new(0.03).with_weight_limits(0.05, 0.60);

        let result = optimizer.optimize(&returns, &cov, &symbols, OptimizationObjective::MaxSharpe);

        assert!(result.sharpe_ratio > 0.0, "夏普比率应为正");
        let weight_sum: f64 = result.weights.iter().map(|w| w.weight).sum();
        assert!((weight_sum - 1.0).abs() < 0.01, "权重总和应为1");

        for w in &result.weights {
            assert!(w.weight >= 0.05 - 0.01 && w.weight <= 0.60 + 0.01);
        }

        println!(
            "\nMax Sharpe 组合: SR={:.3}, 收益={:.1}%, 波动={:.1}%",
            result.sharpe_ratio,
            result.expected_return * 100.0,
            result.expected_volatility * 100.0
        );
        for w in &result.weights {
            println!("  {}: {:.1}%", w.symbol, w.weight * 100.0);
        }
    }

    #[test]
    fn test_min_volatility() {
        let (returns, cov, symbols) = sample_data();
        let optimizer = MarkowitzOptimizer::new(0.03);

        let result = optimizer.optimize(
            &returns,
            &cov,
            &symbols,
            OptimizationObjective::MinVolatility,
        );

        let ew_vol = {
            let n = 3;
            let ew = vec![1.0 / 3.0; 3];
            let var: f64 = (0..n)
                .map(|i| (0..n).map(|j| ew[i] * ew[j] * cov[i * n + j]).sum::<f64>())
                .sum();
            var.sqrt()
        };

        assert!(
            result.expected_volatility <= ew_vol + 0.01,
            "最小波动率应小于等于等权重波动率"
        );

        println!(
            "\nMin Vol 组合: 波动={:.1}% (等权={:.1}%)",
            result.expected_volatility * 100.0,
            ew_vol * 100.0
        );
    }

    #[test]
    fn test_risk_parity() {
        let (returns, cov, symbols) = sample_data();
        let optimizer = MarkowitzOptimizer::new(0.03);

        let result =
            optimizer.optimize(&returns, &cov, &symbols, OptimizationObjective::RiskParity);

        let weights_vec: Vec<f64> = result.weights.iter().map(|w| w.weight).collect();
        let (_risk_contrib, pct_contrib, _) =
            PortfolioAnalyzer::risk_contribution(&weights_vec, &cov);

        let avg_contrib = pct_contrib.iter().sum::<f64>() / pct_contrib.len() as f64;
        for pc in &pct_contrib {
            assert!(
                (pc - avg_contrib).abs() < 0.15,
                "风险贡献应接近平均值的±15%"
            );
        }

        println!(
            "\nRisk Parity 组合: 波动={:.1}%, 夏普={:.3}",
            result.expected_volatility * 100.0,
            result.sharpe_ratio
        );
        for (i, w) in result.weights.iter().enumerate() {
            println!(
                "  {}: {:.1}% (风险贡献: {:.1}%)",
                w.symbol,
                w.weight * 100.0,
                pct_contrib[i] * 100.0
            );
        }
    }

    #[test]
    fn test_black_litterman() {
        let (_returns, cov, symbols) = sample_data();
        let n = symbols.len();

        let market_weights = vec![1.0 / n as f64; n];

        let bl = BlackLittermanModel::new(3.0, 0.05);

        let implied = bl.implied_returns(&cov, &market_weights);
        assert_eq!(implied.len(), n);

        // 主观观点：认为股票收益高于均衡
        let p = vec![vec![1.0, 0.0, 0.0]];
        let q = vec![0.15];
        let omega = vec![vec![0.002]];

        let posterior = bl.blend_views(&p, &q, &omega, &cov, &market_weights);
        assert_eq!(posterior.len(), n);

        assert!(posterior[0] > implied[0], "股票的后验收益应高于先验");

        println!("\nBlack-Litterman:");
        println!(
            "  先验收益: {:?}",
            implied.iter().map(|x| x * 100.0).collect::<Vec<_>>()
        );
        println!(
            "  后验收益: {:?}",
            posterior.iter().map(|x| x * 100.0).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_efficient_frontier() {
        let (returns, cov, symbols) = sample_data();
        let optimizer = MarkowitzOptimizer::new(0.03).with_weight_limits(0.05, 0.60);

        let frontier = EfficientFrontier::compute(&optimizer, &returns, &cov, &symbols, 5);
        assert_eq!(frontier.len(), 5);

        // 有效前沿上，收益增加时波动率也应增加（或至少不显著减少）
        for i in 1..frontier.len() {
            assert!(frontier[i].0 > frontier[i - 1].0 - 0.001, "收益应递增");
        }

        println!("\n有效前沿:");
        for (i, (ret, vol, _)) in frontier.iter().enumerate() {
            println!(
                "  #{}: 收益={:.2}%, 波动={:.2}%, 夏普={:.3}",
                i + 1,
                ret * 100.0,
                vol * 100.0,
                (ret - 0.03) / vol
            );
        }
    }

    #[test]
    fn test_portfolio_analyzer() {
        let (_returns, cov, symbols) = sample_data();
        let n = symbols.len();
        let weights = vec![0.5, 0.3, 0.2];
        let individual_vols = vec![0.20, 0.10, 0.173]; // sqrt of diagonal

        let (_rc, pct, port_vol) = PortfolioAnalyzer::risk_contribution(&weights, &cov);
        assert!((pct.iter().sum::<f64>() - 1.0).abs() < 0.01);
        assert!(port_vol > 0.0);

        let dr = PortfolioAnalyzer::diversification_ratio(&weights, &individual_vols, &cov);
        assert!(dr >= 1.0, "分散化比率应>=1");

        let optimizer = MarkowitzOptimizer::new(0.03);
        let result = optimizer.optimize(
            &vec![0.10, 0.04, 0.07],
            &cov,
            &symbols,
            OptimizationObjective::MaxSharpe,
        );
        let report = PortfolioAnalyzer::generate_report(&result);
        assert!(report.contains("组合优化报告"));
        assert!(report.contains("沪深300"));
    }

    #[test]
    fn test_implied_weights() {
        let (_returns, cov, _symbols) = sample_data();
        let expected = vec![0.10, 0.04, 0.07];

        let bl = BlackLittermanModel::new(3.0, 0.05);
        let weights = bl.implied_weights(&expected, &cov);

        assert_eq!(weights.len(), 3);
        let sum: f64 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 0.01, "权重总和应为1");

        println!("\nImplied Weights: {:?}", weights);
    }

    #[test]
    fn test_target_return_optimization() {
        let (returns, cov, symbols) = sample_data();

        let opt1 = MarkowitzOptimizer::new(0.03)
            .with_weight_limits(0.05, 0.60)
            .with_target_return(0.06);
        let result1 = opt1.optimize(
            &returns,
            &cov,
            &symbols,
            OptimizationObjective::MinVolatilityForReturn,
        );

        let opt2 = MarkowitzOptimizer::new(0.03)
            .with_weight_limits(0.05, 0.60)
            .with_target_return(0.08);
        let result2 = opt2.optimize(
            &returns,
            &cov,
            &symbols,
            OptimizationObjective::MinVolatilityForReturn,
        );

        // 更高目标收益的组合应有更高波动率
        assert!(
            result2.expected_volatility >= result1.expected_volatility - 0.01,
            "更高收益目标应有更高或相等波动率"
        );
    }
}
