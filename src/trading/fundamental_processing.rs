use chrono::NaiveDate;
use std::collections::HashMap;

use super::factor_model::{FactorType, FactorWeight};
use super::fundamental::Fundamentals;

// ============================================================
// Winsorize 缩尾处理
// ============================================================

/// Winsorize 配置
#[derive(Debug, Clone, Copy)]
pub struct WinsorizeConfig {
    /// 下分位数截断点（如 0.01 = 1% 分位）
    pub lower_quantile: f64,
    /// 上分位数截断点（如 0.99 = 99% 分位）
    pub upper_quantile: f64,
}

impl Default for WinsorizeConfig {
    fn default() -> Self {
        Self {
            lower_quantile: 0.01,
            upper_quantile: 0.99,
        }
    }
}

/// 对一组因子值进行 Winsorize 缩尾处理
/// 将超出 [lower, upper] 分位数的极端值截断到分位数值
pub fn winsorize(values: &mut [f64], config: WinsorizeConfig) {
    if values.len() < 2 {
        return;
    }

    // 过滤有效值
    let mut valid: Vec<f64> = values
        .iter()
        .filter(|v| v.is_finite() && !v.is_nan())
        .copied()
        .collect();
    if valid.len() < 2 {
        return;
    }

    valid.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let lower_idx = (config.lower_quantile * valid.len() as f64).floor() as usize;
    let upper_idx =
        ((config.upper_quantile * valid.len() as f64).ceil() as usize).min(valid.len() - 1);

    let lower_bound = valid[lower_idx];
    let upper_bound = valid[upper_idx];

    for v in values.iter_mut() {
        if v.is_finite() && !v.is_nan() {
            *v = v.clamp(lower_bound, upper_bound);
        }
    }
}

/// 对多个因子的数据进行批量 Winsorize
/// 输入：`Vec<(FactorType, Vec<f64>)>`，每个因子对应一组横截面值
/// 输出：缩尾处理后的相同结构
pub fn winsorize_factors(factor_data: &mut HashMap<FactorType, Vec<f64>>, config: WinsorizeConfig) {
    for values in factor_data.values_mut() {
        winsorize(values, config);
    }
}

// ============================================================
// 行业中性化 (Industry Neutralization)
// ============================================================

/// 行业中性化配置
#[derive(Debug, Clone)]
pub struct NeutralizationConfig {
    /// 是否在行业中性化后做全市场标准化
    pub market_standardize: bool,
}

impl Default for NeutralizationConfig {
    fn default() -> Self {
        Self {
            market_standardize: true,
        }
    }
}

/// 行业中性化：组内去均值 + 可选缩放
///
/// 对于每个行业组内的因子值，减去该行业的均值（和可选的行业标准差归一化），
/// 使得因子值不再受行业分布影响。
///
/// 返回中性化后的值，与输入一一对应。
pub fn industry_neutralize(
    values: &[f64],
    industries: &[String],
    config: &NeutralizationConfig,
) -> Vec<f64> {
    if values.len() != industries.len() || values.is_empty() {
        return values.to_vec();
    }

    // 按行业分组收集索引
    let mut industry_groups: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, ind) in industries.iter().enumerate() {
        industry_groups.entry(ind).or_default().push(i);
    }

    let mut result = vec![0.0; values.len()];

    // 对每个行业组做去均值处理
    for group in industry_groups.values() {
        let group_values: Vec<f64> = group
            .iter()
            .filter_map(|&i| {
                let v = values[i];
                if v.is_finite() && !v.is_nan() {
                    Some(v)
                } else {
                    None
                }
            })
            .collect();

        if group_values.is_empty() {
            continue;
        }

        let mean = group_values.iter().sum::<f64>() / group_values.len() as f64;
        let std_dev = if group_values.len() > 1 {
            let variance = group_values.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                / (group_values.len() - 1) as f64;
            variance.sqrt()
        } else {
            0.0
        };

        // 组内中性化：减去均值，除以标准差（如果 std > 0）
        for &i in group {
            let v = values[i];
            if v.is_finite() && !v.is_nan() {
                let neutralized = v - mean;
                result[i] = if std_dev > 1e-10 {
                    neutralized / std_dev
                } else {
                    0.0
                };
            } else {
                result[i] = f64::NAN;
            }
        }
    }

    // 可选：全市场标准化（使中性化后的值具有可比性）
    if config.market_standardize {
        let valid: Vec<(usize, f64)> = result
            .iter()
            .enumerate()
            .filter(|(_, v)| v.is_finite() && !v.is_nan())
            .map(|(i, v)| (i, *v))
            .collect();

        if !valid.is_empty() {
            let global_mean: f64 = valid.iter().map(|(_, v)| v).sum::<f64>() / valid.len() as f64;
            let global_std = if valid.len() > 1 {
                let variance = valid
                    .iter()
                    .map(|(_, v)| (v - global_mean).powi(2))
                    .sum::<f64>()
                    / (valid.len() - 1) as f64;
                variance.sqrt()
            } else {
                0.0
            };

            if global_std > 1e-10 {
                for &(i, v) in &valid {
                    result[i] = (v - global_mean) / global_std;
                }
            }
        }
    }

    result
}

// ============================================================
// Point-in-Time 数据对齐（财报发布延迟）
// ============================================================

/// Point-in-Time 对齐器
///
/// 解决前瞻偏差：在回测的某个日期 `as_of_date`，只能看到
/// `publish_date <= as_of_date` 的财报数据。
pub struct PointInTimeAligner;

impl PointInTimeAligner {
    /// 获取在指定日期可见的财报数据（考虑发布延迟）
    ///
    /// 例如：2025-03-15 时，只能看到 2025-03-15 之前已发布的财报。
    /// 如果某份 Q4 财报的 `report_date=2024-12-31` 但 `publish_date=2025-04-20`，
    /// 那么 2025-03-15 时这份财报还不可见。
    pub fn visible_at(fundamentals: &[Fundamentals], as_of_date: NaiveDate) -> Vec<Fundamentals> {
        fundamentals
            .iter()
            .filter(|f| f.publish_date <= as_of_date)
            .cloned()
            .collect()
    }

    /// 按时间线生成 Point-in-Time 快照序列
    ///
    /// 输入一组日期（如每根 K 线的日期），返回每个日期对应的可见财报数据。
    ///
    /// 返回值：`(as_of_date, visible_fundamentals)` 的列表。
    pub fn timeline_snapshots(
        fundamentals: &[Fundamentals],
        dates: &[NaiveDate],
    ) -> Vec<(NaiveDate, Vec<Fundamentals>)> {
        let mut sorted_dates = dates.to_vec();
        sorted_dates.sort();

        // 按发布日排序的财报数据
        let mut sorted_fundamentals: Vec<&Fundamentals> = fundamentals.iter().collect();
        sorted_fundamentals.sort_by_key(|f| f.publish_date);

        let mut snapshots = Vec::with_capacity(sorted_dates.len());
        let mut visible = Vec::new();
        let mut fi = 0;

        for &date in &sorted_dates {
            // 加入在 date 之前（含）发布的新财报
            while fi < sorted_fundamentals.len() && sorted_fundamentals[fi].publish_date <= date {
                visible.push(sorted_fundamentals[fi].clone());
                fi += 1;
            }

            // 去重（同一股票可能有多期财报，保留最新一期）
            let mut latest_by_symbol: HashMap<String, Fundamentals> = HashMap::new();
            for f in &visible {
                let entry = latest_by_symbol
                    .entry(f.symbol.clone())
                    .or_insert_with(|| f.clone());
                if f.report_date > entry.report_date {
                    *entry = f.clone();
                }
            }

            let pit_data: Vec<Fundamentals> = latest_by_symbol.into_values().collect();
            snapshots.push((date, pit_data));
        }

        snapshots
    }
}

// ============================================================
// 因子共线性检测
// ============================================================

/// 皮尔逊相关系数
fn pearson_correlation(x: &[f64], y: &[f64]) -> Option<f64> {
    let n = x.len().min(y.len());
    if n < 2 {
        return None;
    }

    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xy = 0.0;
    let mut sum_x2 = 0.0;
    let mut sum_y2 = 0.0;
    let mut count = 0usize;

    for i in 0..n {
        if x[i].is_finite() && y[i].is_finite() && !x[i].is_nan() && !y[i].is_nan() {
            sum_x += x[i];
            sum_y += y[i];
            sum_xy += x[i] * y[i];
            sum_x2 += x[i] * x[i];
            sum_y2 += y[i] * y[i];
            count += 1;
        }
    }

    if count < 2 {
        return None;
    }

    let n = count as f64;
    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();

    if denominator < 1e-10 {
        return Some(0.0);
    }

    Some(numerator / denominator)
}

/// 共线性检测报告
#[derive(Debug, Clone)]
pub struct CollinearityReport {
    /// 每对因子之间的相关系数
    pub correlations: Vec<(FactorType, FactorType, f64)>,
    /// 高度相关的因子对（|r| > threshold）
    pub high_correlation_pairs: Vec<(FactorType, FactorType, f64)>,
}

/// 计算因子间的皮尔逊相关系数矩阵
///
/// 输入：`HashMap<FactorType, Vec<f64>>`，每个因子对应一组横截面值
/// 要求所有因子的向量长度一致
pub fn compute_collinearity(
    factor_data: &HashMap<FactorType, Vec<f64>>,
    threshold: f64,
) -> CollinearityReport {
    let factor_types: Vec<FactorType> = factor_data.keys().copied().collect();
    let mut correlations = Vec::new();
    let mut high_correlation_pairs = Vec::new();

    for i in 0..factor_types.len() {
        for j in (i + 1)..factor_types.len() {
            let fa = factor_types[i];
            let fb = factor_types[j];
            if let (Some(va), Some(vb)) = (factor_data.get(&fa), factor_data.get(&fb)) {
                if let Some(r) = pearson_correlation(va, vb) {
                    correlations.push((fa, fb, r));
                    if r.abs() > threshold {
                        high_correlation_pairs.push((fa, fb, r));
                    }
                }
            }
        }
    }

    CollinearityReport {
        correlations,
        high_correlation_pairs,
    }
}

// ============================================================
// 综合处理流水线
// ============================================================

/// 因子处理流水线配置
#[derive(Debug, Clone)]
pub struct FactorPipelineConfig {
    pub winsorize: WinsorizeConfig,
    pub neutralize: bool,
    pub neutralization: NeutralizationConfig,
    pub collinearity_threshold: f64,
    /// P1: 是否启用因子去重
    pub deduplicate_correlated: bool,
    /// P1: 是否启用 PCA 正交化
    pub orthogonalize: bool,
    /// P1: 是否输出交易成本估算
    pub estimate_cost: bool,
}

impl Default for FactorPipelineConfig {
    fn default() -> Self {
        Self {
            winsorize: WinsorizeConfig::default(),
            neutralize: true,
            neutralization: NeutralizationConfig::default(),
            collinearity_threshold: 0.7,
            deduplicate_correlated: true,
            orthogonalize: false,
            estimate_cost: true,
        }
    }
}

/// 处理后的因子数据（按行业中性化和缩尾处理后）
#[derive(Debug, Clone)]
pub struct ProcessedFactors {
    /// 股票代码列表（与下面各向量的索引一一对应）
    pub symbols: Vec<String>,
    /// 行业列表
    pub industries: Vec<String>,
    /// 每个因子对应的处理后值向量
    pub factor_values: HashMap<FactorType, Vec<f64>>,
    /// 共线性检测报告
    pub collinearity: CollinearityReport,
    /// P1: 被去重的因子列表
    pub removed_factors: Vec<FactorType>,
    /// P1: 交易成本估算
    pub cost_estimate: Option<CostEstimate>,
    /// P2: 因子动量（简化：基于横截面偏度）
    pub factor_momentum: HashMap<FactorType, FactorMomentum>,
}

/// P1: 交易成本估算结果
#[derive(Debug, Clone)]
pub struct CostEstimate {
    /// 每次调仓的估计换手率
    pub turnover_per_rebalance: f64,
    /// 年化换手率（假设季度调仓）
    pub annual_turnover: f64,
    /// 交易成本对年化收益的侵蚀（%）
    pub cost_drag_pct: f64,
    /// 建议的最低超额收益要求
    pub min_alpha_required: f64,
}

/// P2: 因子动量信号
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FactorMomentum {
    /// 因子值连续改善
    Improving,
    /// 因子值连续恶化
    Deteriorating,
    /// 因子值稳定
    Stable,
}

/// P2: 市场状态（用于动态权重调整）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketState {
    /// 牛市：成长因子应加强
    Bull,
    /// 熊市：价值/质量因子应加强
    Bear,
    /// 震荡市：价值因子略加强
    Range,
}

/// P2: 动态权重调整器
pub struct DynamicWeightAdjuster;

impl DynamicWeightAdjuster {
    /// 根据市场状态调整因子权重
    pub fn adjust_weights(base_weights: &[FactorWeight], state: MarketState) -> Vec<FactorWeight> {
        let mut adjusted: Vec<FactorWeight> = Vec::with_capacity(base_weights.len());

        for wf in base_weights {
            let multiplier = match state {
                MarketState::Bull => match wf.factor {
                    FactorType::RevenueGrowthYoy
                    | FactorType::NetProfitGrowthYoy
                    | FactorType::RevenueCagr3y
                    | FactorType::ProfitCagr3y => 1.2,
                    FactorType::PeTtm
                    | FactorType::Pb
                    | FactorType::PsTtm
                    | FactorType::POcf
                    | FactorType::EvEbitda
                    | FactorType::DividendYield => 0.8,
                    _ => 1.0,
                },
                MarketState::Bear => match wf.factor {
                    FactorType::PeTtm
                    | FactorType::Pb
                    | FactorType::PsTtm
                    | FactorType::POcf
                    | FactorType::EvEbitda
                    | FactorType::DividendYield => 1.2,
                    FactorType::Roe
                    | FactorType::Roic
                    | FactorType::InterestCoverage
                    | FactorType::FreeCashflow => 1.1,
                    FactorType::RevenueGrowthYoy
                    | FactorType::NetProfitGrowthYoy
                    | FactorType::RevenueCagr3y
                    | FactorType::ProfitCagr3y => 0.85,
                    _ => 1.0,
                },
                MarketState::Range => match wf.factor {
                    FactorType::PeTtm
                    | FactorType::Pb
                    | FactorType::PsTtm
                    | FactorType::POcf
                    | FactorType::EvEbitda
                    | FactorType::DividendYield => 1.15,
                    FactorType::Roe
                    | FactorType::Roic
                    | FactorType::InterestCoverage
                    | FactorType::FreeCashflow => 1.05,
                    _ => 1.0,
                },
            };

            adjusted.push(FactorWeight {
                factor: wf.factor,
                weight: wf.weight * multiplier,
            });
        }

        let total: f64 = adjusted.iter().map(|w| w.weight).sum();
        if total > 1e-10 {
            for w in &mut adjusted {
                w.weight /= total;
            }
        }

        adjusted
    }

    /// 基于价格序列判断市场状态（简化版：均线偏离 + 斜率）
    pub fn detect_from_prices(closes: &[f64], lookback: usize) -> MarketState {
        if closes.len() < lookback + 1 {
            return MarketState::Range;
        }

        let recent: &[f64] = &closes[closes.len() - lookback..];
        let current = *closes.last().unwrap();
        let mean: f64 = recent.iter().sum::<f64>() / recent.len() as f64;
        let deviation = (current - mean) / mean;

        let mid = recent.len() / 2;
        let first_half: f64 = recent[..mid].iter().sum::<f64>() / mid as f64;
        let second_half: f64 = recent[mid..].iter().sum::<f64>() / (recent.len() - mid) as f64;
        let slope = if first_half.abs() > 1e-10 {
            (second_half - first_half) / first_half
        } else {
            0.0
        };

        if deviation > 0.05 && slope > 0.01 {
            MarketState::Bull
        } else if deviation < -0.05 && slope < -0.01 {
            MarketState::Bear
        } else {
            MarketState::Range
        }
    }
}

// ============================================================
// P1: Gram-Schmidt 因子正交化 & 因子去重
// ============================================================

/// Gram-Schmidt 正交化：消除因子间的线性相关性
pub fn gram_schmidt_orthogonalize(
    factor_data: &HashMap<FactorType, Vec<f64>>,
    factor_order: &[FactorType],
) -> HashMap<FactorType, Vec<f64>> {
    let n = if let Some(first) = factor_order.first().and_then(|f| factor_data.get(f)) {
        first.len()
    } else {
        return HashMap::new();
    };

    let mut orthogonal: HashMap<FactorType, Vec<f64>> = HashMap::new();
    let mut orth_vectors: Vec<(FactorType, Vec<f64>)> = Vec::with_capacity(factor_order.len());

    for &factor in factor_order {
        let Some(original) = factor_data.get(&factor) else {
            continue;
        };
        if original.len() != n {
            continue;
        }

        let mut v = original.clone();

        // 减去在所有已有正交向量上的投影
        for (_orth_factor, orth_vec) in &orth_vectors {
            let dot_product: f64 = v
                .iter()
                .zip(orth_vec.iter())
                .filter(|(a, b)| a.is_finite() && b.is_finite() && !a.is_nan() && !b.is_nan())
                .map(|(a, b)| a * b)
                .sum();

            let norm_sq: f64 = orth_vec
                .iter()
                .filter(|v| v.is_finite() && !v.is_nan())
                .map(|v| v * v)
                .sum();

            if norm_sq > 1e-20 {
                let coef = dot_product / norm_sq;
                for (vi, ov) in v.iter_mut().zip(orth_vec.iter()) {
                    if vi.is_finite() && ov.is_finite() && !vi.is_nan() && !ov.is_nan() {
                        *vi -= coef * ov;
                    }
                }
            }
        }

        orth_vectors.push((factor, v.clone()));
        orthogonal.insert(factor, v);
    }

    orthogonal
}

/// P1: 自动因子去重：对高度相关的因子组，只保留信息量最大的一个
pub fn deduplicate_correlated_factors(
    factor_data: &HashMap<FactorType, Vec<f64>>,
    collinearity: &CollinearityReport,
) -> Vec<FactorType> {
    let all_factors: Vec<FactorType> = factor_data.keys().copied().collect();
    if all_factors.is_empty() {
        return vec![];
    }

    // 并查集
    let mut parent: std::collections::HashMap<FactorType, FactorType> =
        all_factors.iter().map(|&f| (f, f)).collect();

    fn find(
        parent: &mut std::collections::HashMap<FactorType, FactorType>,
        x: FactorType,
    ) -> FactorType {
        let p = *parent.get(&x).unwrap_or(&x);
        if p == x {
            return x;
        }
        let root = find(parent, p);
        parent.insert(x, root);
        root
    }

    fn union(
        parent: &mut std::collections::HashMap<FactorType, FactorType>,
        a: FactorType,
        b: FactorType,
    ) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent.insert(ra, rb);
        }
    }

    // 合并高相关因子
    for (fa, fb, _) in &collinearity.high_correlation_pairs {
        union(&mut parent, *fa, *fb);
    }

    // 分组
    let mut groups: std::collections::HashMap<FactorType, Vec<FactorType>> = HashMap::new();
    for &f in &all_factors {
        let root = find(&mut parent, f);
        groups.entry(root).or_default().push(f);
    }

    // 每组保留方差最大的因子
    let mut result = Vec::new();
    for group in groups.values() {
        let best = group
            .iter()
            .max_by(|&&a, &&b| {
                let var_a = factor_data
                    .get(&a)
                    .map(|v| {
                        let valid: Vec<f64> = v
                            .iter()
                            .filter(|v| v.is_finite() && !v.is_nan())
                            .copied()
                            .collect();
                        if valid.is_empty() {
                            return 0.0;
                        }
                        let mean = valid.iter().sum::<f64>() / valid.len() as f64;
                        valid.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                    })
                    .unwrap_or(0.0);
                let var_b = factor_data
                    .get(&b)
                    .map(|v| {
                        let valid: Vec<f64> = v
                            .iter()
                            .filter(|v| v.is_finite() && !v.is_nan())
                            .copied()
                            .collect();
                        if valid.is_empty() {
                            return 0.0;
                        }
                        let mean = valid.iter().sum::<f64>() / valid.len() as f64;
                        valid.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                    })
                    .unwrap_or(0.0);
                var_a
                    .partial_cmp(&var_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
            .unwrap_or(group[0]);
        result.push(best);
    }

    result.sort_by_key(|f| format!("{:?}", f));
    result
}

/// P1: A 股交易成本配置
#[derive(Debug, Clone, Copy)]
pub struct TransactionCostConfig {
    /// 佣金费率（默认万2.5）
    pub commission_rate: f64,
    /// 最低佣金（元）
    pub min_commission: f64,
    /// 印花税（默认0.05%，卖出收取）
    pub stamp_tax_rate: f64,
    /// 过户费（默认万0.1）
    pub transfer_fee_rate: f64,
    /// 滑点（默认0.1%）
    pub slippage_rate: f64,
}

impl Default for TransactionCostConfig {
    fn default() -> Self {
        Self {
            commission_rate: 0.00025,
            min_commission: 5.0,
            stamp_tax_rate: 0.0005,
            transfer_fee_rate: 0.00001,
            slippage_rate: 0.001,
        }
    }
}

/// P1: 计算单次交易的总成本
pub fn calc_transaction_cost(amount: f64, is_sell: bool, config: &TransactionCostConfig) -> f64 {
    if amount <= 0.0 {
        return 0.0;
    }
    let commission = (amount * config.commission_rate).max(config.min_commission);
    let stamp_tax = if is_sell {
        amount * config.stamp_tax_rate
    } else {
        0.0
    };
    let transfer_fee = amount * config.transfer_fee_rate;
    let slippage = amount * config.slippage_rate;
    commission + stamp_tax + transfer_fee + slippage
}

/// P1: 估算年化换手率
pub fn estimate_annual_turnover(turnover_per_rebalance: f64, rebalance_frequency: usize) -> f64 {
    turnover_per_rebalance * rebalance_frequency as f64
}

/// P1: 估算交易成本对年化收益的侵蚀
pub fn estimate_cost_drag(annual_turnover: f64, round_trip_cost: f64) -> f64 {
    annual_turnover * round_trip_cost / 2.0
}

/// P1: 估算策略换手率和交易成本
fn estimate_turnover(factor_data: &HashMap<FactorType, Vec<f64>>) -> Option<CostEstimate> {
    if factor_data.is_empty() {
        return None;
    }

    // 简化假设：因子越多，换手率越高
    let num_factors = factor_data.len() as f64;
    let turnover_per_rebalance = (0.15 + num_factors * 0.03).min(0.60);
    let rebalance_frequency = 4; // 季度调仓

    let annual_turnover = estimate_annual_turnover(turnover_per_rebalance, rebalance_frequency);
    let config = TransactionCostConfig::default();
    let round_trip_cost = calc_transaction_cost(100.0, true, &config) / 100.0;
    let cost_drag_pct = estimate_cost_drag(annual_turnover, round_trip_cost) * 100.0;
    let min_alpha_required = cost_drag_pct * 2.0; // 建议超额收益至少覆盖成本 2 倍

    Some(CostEstimate {
        turnover_per_rebalance,
        annual_turnover,
        cost_drag_pct,
        min_alpha_required,
    })
}

/// P2: 计算因子动量（基于横截面偏度作为代理）
fn calc_all_factor_momentum(
    factor_data: &HashMap<FactorType, Vec<f64>>,
) -> HashMap<FactorType, FactorMomentum> {
    let mut momentum = HashMap::new();

    for (&factor, values) in factor_data {
        let valid: Vec<f64> = values
            .iter()
            .filter(|v| v.is_finite() && !v.is_nan())
            .copied()
            .collect();

        if valid.len() < 3 {
            continue;
        }

        let mean: f64 = valid.iter().sum::<f64>() / valid.len() as f64;
        let variance: f64 =
            valid.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / valid.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev < 1e-10 {
            continue;
        }

        let skewness: f64 = valid
            .iter()
            .map(|v| ((v - mean) / std_dev).powi(3))
            .sum::<f64>()
            / valid.len() as f64;

        let momentum_signal = if skewness > 0.5 {
            FactorMomentum::Improving
        } else if skewness < -0.5 {
            FactorMomentum::Deteriorating
        } else {
            FactorMomentum::Stable
        };

        momentum.insert(factor, momentum_signal);
    }

    momentum
}

impl ProcessedFactors {
    /// 获取指定股票指定因子的值
    pub fn get(&self, symbol: &str, factor: FactorType) -> Option<f64> {
        let idx = self.symbols.iter().position(|s| s == symbol)?;
        self.factor_values
            .get(&factor)
            .and_then(|v| v.get(idx).copied())
    }

    /// 获取所有股票的某因子值
    pub fn get_factor(&self, factor: FactorType) -> Option<&Vec<f64>> {
        self.factor_values.get(&factor)
    }
}

/// 运行完整的因子处理流水线：
/// 1. 提取原始因子值
/// 2. Winsorize 缩尾
/// 3. 行业中性化
/// 4. 共线性检测
/// 5. 因子去重（P1）
/// 6. PCA 正交化（P1，可选）
/// 7. 交易成本估算（P1）
/// 8. 因子动量（P2）
pub fn run_factor_pipeline(
    fundamentals: &[Fundamentals],
    factors: &[FactorType],
    config: &FactorPipelineConfig,
) -> ProcessedFactors {
    let valid: Vec<&Fundamentals> = fundamentals.iter().filter(|f| f.is_valid()).collect();
    let n = valid.len();

    let symbols: Vec<String> = valid.iter().map(|f| f.symbol.clone()).collect();
    let industries: Vec<String> = valid.iter().map(|f| f.industry.clone()).collect();

    let mut factor_data: HashMap<FactorType, Vec<f64>> = HashMap::new();
    for &factor in factors {
        let values: Vec<f64> = valid.iter().map(|f| factor.extract(f)).collect();
        factor_data.insert(factor, values);
    }

    for values in factor_data.values_mut() {
        winsorize(values, config.winsorize);
    }

    if config.neutralize && n > 1 {
        for values in factor_data.values_mut() {
            let neutralized = industry_neutralize(values, &industries, &config.neutralization);
            *values = neutralized;
        }
    }

    let collinearity = compute_collinearity(&factor_data, config.collinearity_threshold);

    // P1: 因子去重
    let mut removed_factors = Vec::new();
    let mut final_data = factor_data.clone();

    if config.deduplicate_correlated && !collinearity.high_correlation_pairs.is_empty() {
        let kept = deduplicate_correlated_factors(&factor_data, &collinearity);
        let kept_set: std::collections::HashSet<FactorType> = kept.iter().copied().collect();
        for factor in factor_data.keys() {
            if !kept_set.contains(factor) {
                removed_factors.push(*factor);
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

    // P1: 交易成本估算
    let cost_estimate = estimate_turnover(&final_data);

    // P2: 因子动量
    let factor_momentum = calc_all_factor_momentum(&final_data);

    ProcessedFactors {
        symbols,
        industries,
        factor_values: final_data,
        collinearity,
        removed_factors,
        cost_estimate,
        factor_momentum,
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn create_test_fundamentals() -> Vec<Fundamentals> {
        let base_date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        vec![
            Fundamentals::new(
                "600519.SH",
                base_date,
                25.0,
                8.0,
                12.0,
                20.0,
                17.5,
                1.5,
                32.0,
                17.6,
                65.0,
                28.0,
                27.2,
                18.5,
                22.3,
                5.5,
                6.7,
                11.1,
                13.4,
                45.0,
                2.5,
                1.8,
                12.0,
                8500.0,
                35000.0,
                25000.0,
            ),
            Fundamentals::new(
                "000858.SZ",
                base_date,
                15.0,
                3.5,
                4.2,
                12.0,
                10.5,
                3.0,
                18.0,
                9.9,
                55.0,
                18.0,
                15.3,
                8.0,
                10.5,
                2.4,
                3.1,
                4.8,
                6.3,
                55.0,
                1.5,
                1.0,
                8.0,
                3000.0,
                8000.0,
                6000.0,
            ),
            Fundamentals::new(
                "601318.SH",
                base_date,
                5.0,
                0.8,
                1.5,
                4.0,
                3.5,
                4.5,
                12.0,
                6.6,
                40.0,
                12.0,
                10.2,
                5.0,
                6.0,
                1.5,
                1.8,
                3.0,
                3.6,
                80.0,
                1.1,
                0.8,
                4.0,
                1500.0,
                12000.0,
                8000.0,
            ),
        ]
    }

    #[test]
    fn test_winsorize_normal_data() {
        let mut values = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 1000.0];
        winsorize(
            &mut values,
            WinsorizeConfig {
                lower_quantile: 0.1,
                upper_quantile: 0.9,
            },
        );
        // 1000.0 应该被截断到 90% 分位数附近
        assert!(values[9] <= 100.0, "极端值应被截断，当前: {}", values[9]);
    }

    #[test]
    fn test_winsorize_extreme_values() {
        let mut values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 100.0, -50.0];
        winsorize(&mut values, WinsorizeConfig::default());
        // 极端值应被截断
        assert!(values[5] <= 6.0, "上限值应被截断: {}", values[5]);
        assert!(values[6] >= -1.0, "下限值应被截断: {}", values[6]);
    }

    #[test]
    fn test_winsorize_empty_and_single() {
        let mut empty: Vec<f64> = vec![];
        winsorize(&mut empty, WinsorizeConfig::default());
        assert!(empty.is_empty());

        let mut single = vec![42.0];
        winsorize(&mut single, WinsorizeConfig::default());
        assert_eq!(single[0], 42.0);
    }

    #[test]
    fn test_industry_neutralize() {
        let values = vec![10.0, 20.0, 30.0, 100.0, 200.0];
        let industries = vec![
            "金融".to_string(),
            "金融".to_string(),
            "金融".to_string(),
            "科技".to_string(),
            "科技".to_string(),
        ];

        let result = industry_neutralize(
            &values,
            &industries,
            &NeutralizationConfig {
                market_standardize: false,
            },
        );

        // 金融行业均值 = 20.0，去均值后应为 [-10, 0, 10]
        assert!(
            (result[0] - (-10.0)).abs() < 1e-6,
            "金融股1去均值: {}",
            result[0]
        );
        assert!(
            (result[1] - 0.0).abs() < 1e-6,
            "金融股2去均值: {}",
            result[1]
        );
        assert!(
            (result[2] - 10.0).abs() < 1e-6,
            "金融股3去均值: {}",
            result[2]
        );

        // 科技行业均值 = 150.0，去均值后应为 [-50, 50]
        assert!(
            (result[3] - (-50.0)).abs() < 1e-6,
            "科技股1去均值: {}",
            result[3]
        );
        assert!(
            (result[4] - 50.0).abs() < 1e-6,
            "科技股2去均值: {}",
            result[4]
        );
    }

    #[test]
    fn test_point_in_time_visible_at() {
        let base_date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let publish_1 = NaiveDate::from_ymd_opt(2025, 2, 28).unwrap(); // 已发布
        let publish_2 = NaiveDate::from_ymd_opt(2025, 4, 20).unwrap(); // 未发布
        let report_1 = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        let report_2 = NaiveDate::from_ymd_opt(2025, 3, 31).unwrap();

        let f1 = Fundamentals::new(
            "600519.SH",
            report_1,
            25.0,
            8.0,
            12.0,
            20.0,
            17.5,
            1.5,
            32.0,
            17.6,
            65.0,
            28.0,
            27.2,
            18.5,
            22.3,
            5.5,
            6.7,
            11.1,
            13.4,
            45.0,
            2.5,
            1.8,
            12.0,
            8500.0,
            35000.0,
            25000.0,
        );
        let f2 = Fundamentals::new(
            "600519.SH",
            report_2,
            22.0,
            7.5,
            11.0,
            19.0,
            16.5,
            1.8,
            30.0,
            16.5,
            62.0,
            26.0,
            25.5,
            17.0,
            20.0,
            5.0,
            6.0,
            10.2,
            12.0,
            44.0,
            2.3,
            1.7,
            11.0,
            8000.0,
            33000.0,
            23000.0,
        );

        // 手动设置 publish_date (通过重新构建)
        let f1_with_date = Fundamentals {
            publish_date: publish_1,
            ..f1
        };
        let f2_with_date = Fundamentals {
            publish_date: publish_2,
            ..f2
        };

        let fundamentals = vec![f1_with_date.clone(), f2_with_date.clone()];
        let visible = PointInTimeAligner::visible_at(&fundamentals, base_date);

        assert_eq!(visible.len(), 1, "只能看到已发布的财报");
        assert_eq!(visible[0].symbol, "600519.SH");
        assert_eq!(visible[0].publish_date, publish_1);
    }

    #[test]
    fn test_point_in_time_timeline() {
        let d1 = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2025, 4, 1).unwrap();
        let d3 = NaiveDate::from_ymd_opt(2025, 7, 1).unwrap();

        let pub_date_q4 = NaiveDate::from_ymd_opt(2025, 3, 20).unwrap();
        let pub_date_q1 = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();

        let report_q4 = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        let report_q1 = NaiveDate::from_ymd_opt(2025, 3, 31).unwrap();

        let f_q4 = Fundamentals::new(
            "600519.SH",
            report_q4,
            25.0,
            8.0,
            12.0,
            20.0,
            17.5,
            1.5,
            32.0,
            17.6,
            65.0,
            28.0,
            27.2,
            18.5,
            22.3,
            5.5,
            6.7,
            11.1,
            13.4,
            45.0,
            2.5,
            1.8,
            12.0,
            8500.0,
            35000.0,
            25000.0,
        );
        let f_q1 = Fundamentals::new(
            "600519.SH",
            report_q1,
            22.0,
            7.5,
            11.0,
            19.0,
            16.5,
            1.8,
            30.0,
            16.5,
            62.0,
            26.0,
            25.5,
            17.0,
            20.0,
            5.0,
            6.0,
            10.2,
            12.0,
            44.0,
            2.3,
            1.7,
            11.0,
            8000.0,
            33000.0,
            23000.0,
        );

        let f_q4_pit = Fundamentals {
            publish_date: pub_date_q4,
            ..f_q4
        };
        let f_q1_pit = Fundamentals {
            publish_date: pub_date_q1,
            ..f_q1
        };

        let fundamentals = vec![f_q4_pit, f_q1_pit];
        let dates = vec![d1, d2, d3];

        let snapshots = PointInTimeAligner::timeline_snapshots(&fundamentals, &dates);

        assert_eq!(snapshots.len(), 3);

        // 1月1日：没有已发布的财报
        assert_eq!(snapshots[0].1.len(), 0, "1月应无已发布财报");

        // 4月1日：Q4 财报已发布
        assert_eq!(snapshots[1].1.len(), 1, "4月应看到Q4财报");
        assert_eq!(snapshots[1].1[0].report_date, report_q4);

        // 7月1日：Q1 财报也已发布，应覆盖 Q4
        assert_eq!(snapshots[2].1.len(), 1, "7月应看到Q1财报（覆盖Q4）");
        assert_eq!(snapshots[2].1[0].report_date, report_q1);
    }

    #[test]
    fn test_pearson_correlation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0]; // 完美正相关
        let r = pearson_correlation(&x, &y).unwrap();
        assert!((r - 1.0).abs() < 1e-6, "完美正相关 r={}", r);

        let y_neg = vec![10.0, 8.0, 6.0, 4.0, 2.0]; // 完美负相关
        let r_neg = pearson_correlation(&x, &y_neg).unwrap();
        assert!((r_neg - (-1.0)).abs() < 1e-6, "完美负相关 r={}", r_neg);

        let z = vec![5.0, 1.0, 3.0, 2.0, 4.0]; // 弱相关
        let r_weak = pearson_correlation(&x, &z).unwrap();
        assert!(r_weak.abs() < 0.8, "弱相关 |r|={}", r_weak.abs());
    }

    #[test]
    fn test_collinearity_detection() {
        let mut data: HashMap<FactorType, Vec<f64>> = HashMap::new();
        data.insert(FactorType::PeTtm, vec![10.0, 20.0, 30.0, 40.0, 50.0]);
        data.insert(FactorType::Pb, vec![10.1, 20.1, 30.1, 40.1, 50.1]); // 高度相关
        data.insert(FactorType::Roe, vec![30.0, 10.0, 25.0, 5.0, 20.0]); // 不相关

        let report = compute_collinearity(&data, 0.9);

        assert!(!report.correlations.is_empty());
        assert!(
            report
                .high_correlation_pairs
                .iter()
                .any(|(a, b, _)| matches!(*a, FactorType::PeTtm) && matches!(*b, FactorType::Pb)),
            "PE 和 PB 应被检测为高相关"
        );
    }

    #[test]
    fn test_factor_pipeline() {
        let fundamentals = create_test_fundamentals();
        let factors = vec![
            FactorType::PeTtm,
            FactorType::Pb,
            FactorType::Roe,
            FactorType::RevenueGrowthYoy,
        ];

        let config = FactorPipelineConfig::default();
        let result = run_factor_pipeline(&fundamentals, &factors, &config);

        assert_eq!(result.symbols.len(), 3);
        assert_eq!(result.factor_values.len(), 4);

        // 验证所有因子都有 3 个值
        for factor in &factors {
            let values = result.get_factor(*factor).unwrap();
            assert_eq!(values.len(), 3);
        }

        // 共线性检测应该运行
        assert!(!result.collinearity.correlations.is_empty());
    }

    #[test]
    fn test_processed_factors_get() {
        let fundamentals = create_test_fundamentals();
        let factors = vec![FactorType::PeTtm, FactorType::Roe];

        let result = run_factor_pipeline(
            &fundamentals,
            &factors,
            &FactorPipelineConfig {
                neutralize: false, // 禁用中性化以方便验证
                ..Default::default()
            },
        );

        let pe = result.get("600519.SH", FactorType::PeTtm);
        assert!(pe.is_some(), "应能获取 PE 值");
        assert!((pe.unwrap() - 25.0).abs() < 0.1, "PE 值接近 25: {:?}", pe);
    }

    #[test]
    fn test_winsorize_nan_handling() {
        let mut values = vec![1.0, f64::NAN, 3.0, 4.0, f64::INFINITY, 6.0];
        winsorize(&mut values, WinsorizeConfig::default());
        // NaN 和 Inf 应保持不变
        assert!(values[1].is_nan());
        assert!(values[4].is_infinite());
        // 正常值不应被修改（没有极端值）
        assert_eq!(values[0], 1.0);
        assert_eq!(values[2], 3.0);
    }

    #[test]
    fn test_industry_neutralize_mismatch() {
        let values = vec![1.0, 2.0];
        let industries = vec!["A".to_string()]; // 长度不匹配
        let result = industry_neutralize(&values, &industries, &NeutralizationConfig::default());
        assert_eq!(result.len(), 2); // 返回原始数据
    }
}
