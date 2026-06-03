use std::collections::HashMap;

use super::factor_model::{FactorModel, FactorScore, FactorType, StockScore};
use super::fundamental::Fundamentals;
use super::fundamental_processing::{
    compute_collinearity, deduplicate_correlated_factors, gram_schmidt_orthogonalize,
    industry_neutralize, winsorize, CollinearityReport, FactorPipelineConfig,
};

/// 扩展：带 Winsorize + 行业中性化 + 因子去重 + PCA 正交化的因子评分
impl FactorModel {
    /// 使用处理流水线计算因子得分
    pub fn score_all_processed(
        &self,
        fundamentals: &[Fundamentals],
        pipeline_config: &FactorPipelineConfig,
    ) -> (Vec<StockScore>, CollinearityReport) {
        let factors: Vec<FactorType> = self.weights().iter().map(|w| w.factor).collect();
        let (symbols, _industries, factor_data, collinearity) =
            run_pipeline(fundamentals, &factors, pipeline_config);

        if symbols.is_empty() {
            return (vec![], collinearity);
        }

        let n = symbols.len();

        let mut factor_values: Vec<(FactorType, Vec<(usize, f64)>)> =
            Vec::with_capacity(self.weights().len());

        for wf in self.weights() {
            if let Some(values) = factor_data.get(&wf.factor) {
                let vals: Vec<(usize, f64)> = values
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.is_finite() && !v.is_nan())
                    .map(|(i, v)| (i, *v))
                    .collect();
                if !vals.is_empty() {
                    factor_values.push((wf.factor, vals));
                }
            }
        }

        let mut stock_scores: Vec<StockScore> = Vec::with_capacity(n);

        for i in 0..n {
            let mut total_score = 0.0;
            let mut factor_details: Vec<FactorScore> = Vec::with_capacity(factor_values.len());

            for (factor_type, vals) in &factor_values {
                let raw_value = vals.iter().find(|(idx, _)| *idx == i).map(|(_, v)| *v);

                let score = if let Some(rv) = raw_value {
                    compute_factor_score(factor_type, rv, vals)
                } else {
                    f64::NAN
                };

                if !score.is_nan() {
                    let weight = self
                        .weights()
                        .iter()
                        .find(|w| w.factor == *factor_type)
                        .map(|w| w.weight)
                        .unwrap_or(0.0);
                    total_score += score * weight;

                    factor_details.push(FactorScore {
                        factor: *factor_type,
                        raw_value: raw_value.unwrap_or(0.0),
                        score,
                    });
                }
            }

            stock_scores.push(StockScore {
                symbol: symbols[i].clone(),
                total_score,
                factor_details,
            });
        }

        stock_scores.sort_by(|a, b| {
            b.total_score
                .partial_cmp(&a.total_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        (stock_scores, collinearity)
    }
}

/// 内部函数：运行因子处理流水线（含 P1 新功能）
fn run_pipeline(
    fundamentals: &[Fundamentals],
    factors: &[FactorType],
    config: &FactorPipelineConfig,
) -> (
    Vec<String>,
    Vec<String>,
    HashMap<FactorType, Vec<f64>>,
    CollinearityReport,
) {
    let valid: Vec<&Fundamentals> = fundamentals.iter().filter(|f| f.is_valid()).collect();
    let n = valid.len();

    let symbols: Vec<String> = valid.iter().map(|f| f.symbol.clone()).collect();
    let industries: Vec<String> = valid.iter().map(|f| f.industry.clone()).collect();

    let mut factor_data: HashMap<FactorType, Vec<f64>> = HashMap::new();
    for &factor in factors {
        let values: Vec<f64> = valid.iter().map(|f| factor.extract(f)).collect();
        factor_data.insert(factor, values);
    }

    // Winsorize 缩尾
    for values in factor_data.values_mut() {
        winsorize(values, config.winsorize);
    }

    // 行业中性化
    if config.neutralize && n > 1 {
        for values in factor_data.values_mut() {
            let neutralized = industry_neutralize(values, &industries, &config.neutralization);
            *values = neutralized;
        }
    }

    // 共线性检测
    let collinearity = compute_collinearity(&factor_data, config.collinearity_threshold);

    // P1: 因子去重
    let mut final_data = factor_data.clone();
    if config.deduplicate_correlated && !collinearity.high_correlation_pairs.is_empty() {
        let kept = deduplicate_correlated_factors(&factor_data, &collinearity);
        let kept_set: std::collections::HashSet<FactorType> = kept.iter().copied().collect();
        for factor in factor_data.keys() {
            if !kept_set.contains(factor) {
                final_data.remove(factor);
            }
        }
    }

    // P1: PCA 正交化（可选）
    if config.orthogonalize && !final_data.is_empty() {
        let order: Vec<FactorType> = final_data.keys().copied().collect();
        let orthogonal = gram_schmidt_orthogonalize(&final_data, &order);
        final_data = orthogonal;
    }

    (symbols, industries, final_data, collinearity)
}

/// 内部函数：计算单个因子得分
fn compute_factor_score(factor: &FactorType, value: f64, all_values: &[(usize, f64)]) -> f64 {
    // 默认使用百分位排名
    let better_count = if factor.is_higher_better() {
        all_values.iter().filter(|(_, v)| *v <= value).count()
    } else {
        all_values.iter().filter(|(_, v)| *v >= value).count()
    };
    (better_count as f64 / all_values.len() as f64) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_all_processed_with_dedup() {
        let model = FactorModel::value_growth();
        let data = Fundamentals::generate_mock(50, 42);

        let config = FactorPipelineConfig {
            deduplicate_correlated: true,
            ..Default::default()
        };
        let (scores, collinearity) = model.score_all_processed(&data, &config);

        assert!(!scores.is_empty());

        // 打印去重信息
        println!("\n因子去重效果:");
        if !collinearity.high_correlation_pairs.is_empty() {
            for (fa, fb, r) in &collinearity.high_correlation_pairs {
                println!(
                    "  {} ↔ {} (r={:.3}) — 其中一个被去除",
                    fa.display_name(),
                    fb.display_name(),
                    r
                );
            }
        }
        println!(
            "  保留因子数: {}",
            scores.first().map(|s| s.factor_details.len()).unwrap_or(0)
        );
    }

    #[test]
    fn test_score_all_processed_with_orthogonalize() {
        let model = FactorModel::value_growth();
        let data = Fundamentals::generate_mock(30, 99);

        let config = FactorPipelineConfig {
            orthogonalize: true,
            deduplicate_correlated: false, // 关闭去重，测试正交化
            ..Default::default()
        };
        let (scores, _) = model.score_all_processed(&data, &config);

        assert!(!scores.is_empty());
        assert_eq!(scores.len(), 30);

        println!("\nPCA 正交化效果（前5名）:");
        for (i, s) in scores.iter().take(5).enumerate() {
            println!("  #{} {} → {:.1}", i + 1, s.symbol, s.total_score);
        }
    }

    #[test]
    fn test_full_pipeline_comparison() {
        let data = Fundamentals::generate_mock(50, 77);
        let model = FactorModel::value_growth();

        // 原始
        let raw = model.score_all(&data);
        // P0: Winsorize + 行业中性化
        let p0 = model.score_all_processed(
            &data,
            &FactorPipelineConfig {
                deduplicate_correlated: false,
                orthogonalize: false,
                ..Default::default()
            },
        );
        // P0+P1: + 因子去重
        let p1 = model.score_all_processed(
            &data,
            &FactorPipelineConfig {
                deduplicate_correlated: true,
                orthogonalize: false,
                ..Default::default()
            },
        );

        println!("\n渐进优化对比:");
        println!(
            "  原始 Top-3: {:?}",
            raw.iter().take(3).map(|s| &s.symbol).collect::<Vec<_>>()
        );
        println!(
            "  P0   Top-3: {:?}",
            p0.0.iter().take(3).map(|s| &s.symbol).collect::<Vec<_>>()
        );
        println!(
            "  P0+P1 Top-3: {:?}",
            p1.0.iter().take(3).map(|s| &s.symbol).collect::<Vec<_>>()
        );
    }
}
