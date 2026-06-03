use super::fundamental::Fundamentals;

/// 因子类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FactorType {
    // 估值因子（越低越好）
    PeTtm,
    Pb,
    PsTtm,
    POcf,
    EvEbitda,
    DividendYield,

    // 盈利因子（越高越好）
    Roe,
    Roa,
    GrossMargin,
    NetMargin,
    Roic,

    // 成长因子（越高越好）
    RevenueGrowthYoy,
    NetProfitGrowthYoy,
    RevenueCagr3y,
    ProfitCagr3y,

    // 财务健康因子
    DebtToAssets,     // 越低越好
    CurrentRatio,     // 越高越好
    QuickRatio,       // 越高越好
    InterestCoverage, // 越高越好
    FreeCashflow,     // 越高越好
}

impl FactorType {
    /// 因子得分方向：true = 越高越好, false = 越低越好
    #[inline]
    pub fn is_higher_better(&self) -> bool {
        !matches!(
            self,
            FactorType::PeTtm
                | FactorType::Pb
                | FactorType::PsTtm
                | FactorType::POcf
                | FactorType::EvEbitda
                | FactorType::DebtToAssets
        )
    }

    /// 提取因子对应的原始值
    #[inline]
    pub fn extract(&self, f: &Fundamentals) -> f64 {
        match self {
            FactorType::PeTtm => f.pe_ttm,
            FactorType::Pb => f.pb,
            FactorType::PsTtm => f.ps_ttm,
            FactorType::POcf => f.p_ocf,
            FactorType::EvEbitda => f.ev_ebitda,
            FactorType::DividendYield => f.dividend_yield,
            FactorType::Roe => f.roe,
            FactorType::Roa => f.roa,
            FactorType::GrossMargin => f.gross_margin,
            FactorType::NetMargin => f.net_margin,
            FactorType::Roic => f.roic,
            FactorType::RevenueGrowthYoy => f.revenue_growth_yoy,
            FactorType::NetProfitGrowthYoy => f.net_profit_growth_yoy,
            FactorType::RevenueCagr3y => f.revenue_cagr_3y,
            FactorType::ProfitCagr3y => f.profit_cagr_3y,
            FactorType::DebtToAssets => f.debt_to_assets,
            FactorType::CurrentRatio => f.current_ratio,
            FactorType::QuickRatio => f.quick_ratio,
            FactorType::InterestCoverage => f.interest_coverage,
            FactorType::FreeCashflow => f.free_cashflow,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            FactorType::PeTtm => "PE(TTM)",
            FactorType::Pb => "PB",
            FactorType::PsTtm => "PS(TTM)",
            FactorType::POcf => "P/OCF",
            FactorType::EvEbitda => "EV/EBITDA",
            FactorType::DividendYield => "股息率",
            FactorType::Roe => "ROE",
            FactorType::Roa => "ROA",
            FactorType::GrossMargin => "毛利率",
            FactorType::NetMargin => "净利率",
            FactorType::Roic => "ROIC",
            FactorType::RevenueGrowthYoy => "营收同比增速",
            FactorType::NetProfitGrowthYoy => "净利润同比增速",
            FactorType::RevenueCagr3y => "营收3年CAGR",
            FactorType::ProfitCagr3y => "净利润3年CAGR",
            FactorType::DebtToAssets => "资产负债率",
            FactorType::CurrentRatio => "流动比率",
            FactorType::QuickRatio => "速动比率",
            FactorType::InterestCoverage => "利息保障倍数",
            FactorType::FreeCashflow => "自由现金流",
        }
    }
}

/// 因子权重配置
#[derive(Debug, Clone)]
pub struct FactorWeight {
    pub factor: FactorType,
    /// 权重（0-1之间，所有权重之和应为1.0）
    pub weight: f64,
}

/// 评分方法
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScoringMethod {
    /// 横截面排名百分位 (0-100)
    PercentileRank,
    /// Z-Score 标准化
    ZScore,
    /// Min-Max 归一化 (0-100)
    MinMax,
}

/// 多因子评分模型
#[derive(Debug, Clone)]
pub struct FactorModel {
    /// 因子权重列表
    weights: Vec<FactorWeight>,
    /// 评分方法
    scoring: ScoringMethod,
    /// 模型名称
    name: String,
}

impl FactorModel {
    /// 创建自定义多因子模型
    pub fn try_new(
        weights: Vec<FactorWeight>,
        scoring: ScoringMethod,
        name: &str,
    ) -> anyhow::Result<Self> {
        let total: f64 = weights.iter().map(|w| w.weight).sum();
        if (total - 1.0).abs() >= 0.01 {
            return Err(anyhow::anyhow!("权重之和必须接近 1.0，当前: {}", total));
        }
        Ok(Self {
            weights,
            scoring,
            name: name.to_string(),
        })
    }

    /// "低估值+高成长"默认模型（GARP 策略）
    pub fn value_growth() -> Self {
        let weights = vec![
            // 估值因子 (40%) - 越低越好
            FactorWeight {
                factor: FactorType::PeTtm,
                weight: 0.12,
            },
            FactorWeight {
                factor: FactorType::Pb,
                weight: 0.08,
            },
            FactorWeight {
                factor: FactorType::EvEbitda,
                weight: 0.08,
            },
            FactorWeight {
                factor: FactorType::PsTtm,
                weight: 0.06,
            },
            FactorWeight {
                factor: FactorType::POcf,
                weight: 0.06,
            },
            // 盈利因子 (25%) - 越高越好
            FactorWeight {
                factor: FactorType::Roe,
                weight: 0.12,
            },
            FactorWeight {
                factor: FactorType::Roic,
                weight: 0.08,
            },
            FactorWeight {
                factor: FactorType::NetMargin,
                weight: 0.05,
            },
            // 成长因子 (25%) - 越高越好
            FactorWeight {
                factor: FactorType::RevenueGrowthYoy,
                weight: 0.08,
            },
            FactorWeight {
                factor: FactorType::NetProfitGrowthYoy,
                weight: 0.08,
            },
            FactorWeight {
                factor: FactorType::ProfitCagr3y,
                weight: 0.05,
            },
            FactorWeight {
                factor: FactorType::RevenueCagr3y,
                weight: 0.04,
            },
            // 财务健康 (10%)
            FactorWeight {
                factor: FactorType::DebtToAssets,
                weight: 0.04,
            },
            FactorWeight {
                factor: FactorType::InterestCoverage,
                weight: 0.03,
            },
            FactorWeight {
                factor: FactorType::FreeCashflow,
                weight: 0.03,
            },
        ];
        Self::try_new(
            weights,
            ScoringMethod::PercentileRank,
            "低估值+高成长 (GARP)",
        )
        .unwrap()
    }

    /// 纯价值策略模型
    pub fn pure_value() -> Self {
        let weights = vec![
            FactorWeight {
                factor: FactorType::PeTtm,
                weight: 0.25,
            },
            FactorWeight {
                factor: FactorType::Pb,
                weight: 0.20,
            },
            FactorWeight {
                factor: FactorType::PsTtm,
                weight: 0.15,
            },
            FactorWeight {
                factor: FactorType::POcf,
                weight: 0.15,
            },
            FactorWeight {
                factor: FactorType::EvEbitda,
                weight: 0.15,
            },
            FactorWeight {
                factor: FactorType::DividendYield,
                weight: 0.10,
            },
        ];
        Self::try_new(weights, ScoringMethod::PercentileRank, "纯价值策略").unwrap()
    }

    /// 纯成长策略模型
    pub fn pure_growth() -> Self {
        let weights = vec![
            FactorWeight {
                factor: FactorType::RevenueGrowthYoy,
                weight: 0.25,
            },
            FactorWeight {
                factor: FactorType::NetProfitGrowthYoy,
                weight: 0.25,
            },
            FactorWeight {
                factor: FactorType::RevenueCagr3y,
                weight: 0.15,
            },
            FactorWeight {
                factor: FactorType::ProfitCagr3y,
                weight: 0.15,
            },
            FactorWeight {
                factor: FactorType::Roe,
                weight: 0.10,
            },
            FactorWeight {
                factor: FactorType::Roic,
                weight: 0.10,
            },
        ];
        Self::try_new(weights, ScoringMethod::PercentileRank, "纯成长策略").unwrap()
    }

    /// 高ROE质量策略模型
    pub fn high_quality() -> Self {
        let weights = vec![
            FactorWeight {
                factor: FactorType::Roe,
                weight: 0.25,
            },
            FactorWeight {
                factor: FactorType::Roic,
                weight: 0.20,
            },
            FactorWeight {
                factor: FactorType::NetMargin,
                weight: 0.10,
            },
            FactorWeight {
                factor: FactorType::GrossMargin,
                weight: 0.10,
            },
            FactorWeight {
                factor: FactorType::Roa,
                weight: 0.10,
            },
            FactorWeight {
                factor: FactorType::InterestCoverage,
                weight: 0.10,
            },
            FactorWeight {
                factor: FactorType::FreeCashflow,
                weight: 0.10,
            },
            FactorWeight {
                factor: FactorType::DebtToAssets,
                weight: 0.05,
            },
        ];
        Self::try_new(weights, ScoringMethod::PercentileRank, "高质量 (高ROE)").unwrap()
    }

    /// 计算所有股票的多因子综合得分
    pub fn score_all(&self, fundamentals: &[Fundamentals]) -> Vec<StockScore> {
        if fundamentals.is_empty() {
            return vec![];
        }

        // 过滤有效数据
        let valid: Vec<&Fundamentals> = fundamentals.iter().filter(|f| f.is_valid()).collect();
        if valid.is_empty() {
            return vec![];
        }

        let n = valid.len();

        // 为每个因子计算横截面统计
        let mut factor_values: Vec<(FactorType, Vec<(usize, f64)>)> =
            Vec::with_capacity(self.weights.len());

        for wf in &self.weights {
            let mut vals: Vec<(usize, f64)> = Vec::with_capacity(n);
            for (i, f) in valid.iter().enumerate() {
                let v = wf.factor.extract(f);
                if v.is_finite() && !v.is_nan() {
                    vals.push((i, v));
                }
            }
            if !vals.is_empty() {
                factor_values.push((wf.factor, vals));
            }
        }

        // 计算每个股票的因子得分
        let mut stock_scores: Vec<StockScore> = Vec::with_capacity(n);

        for (i, f) in valid.iter().enumerate() {
            let mut total_score = 0.0;
            let mut factor_details: Vec<FactorScore> = Vec::with_capacity(factor_values.len());

            for (factor_type, vals) in &factor_values {
                // 找到当前股票的因子值
                let raw_value = vals.iter().find(|(idx, _)| *idx == i).map(|(_, v)| *v);

                let score = if let Some(rv) = raw_value {
                    self.compute_factor_score(factor_type, rv, vals)
                } else {
                    f64::NAN
                };

                if !score.is_nan() {
                    let weight = self
                        .weights
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
                symbol: f.symbol.clone(),
                total_score,
                factor_details,
            });
        }

        // 按总分降序排列
        stock_scores.sort_by(|a, b| {
            b.total_score
                .partial_cmp(&a.total_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        stock_scores
    }

    /// 计算单个因子的标准化得分 (0-100)
    fn compute_factor_score(
        &self,
        factor: &FactorType,
        value: f64,
        all_values: &[(usize, f64)],
    ) -> f64 {
        match self.scoring {
            ScoringMethod::PercentileRank => {
                let better_count = if factor.is_higher_better() {
                    all_values.iter().filter(|(_, v)| *v <= value).count()
                } else {
                    all_values.iter().filter(|(_, v)| *v >= value).count()
                };
                (better_count as f64 / all_values.len() as f64) * 100.0
            }
            ScoringMethod::ZScore => {
                let mean: f64 =
                    all_values.iter().map(|(_, v)| v).sum::<f64>() / all_values.len() as f64;
                let variance: f64 = all_values
                    .iter()
                    .map(|(_, v)| (v - mean).powi(2))
                    .sum::<f64>()
                    / all_values.len() as f64;
                let std_dev = variance.sqrt();

                if std_dev < 1e-10 {
                    return 50.0; // 无差异
                }

                let z = if factor.is_higher_better() {
                    (value - mean) / std_dev
                } else {
                    (mean - value) / std_dev
                };

                // 将 Z-score 映射到 0-100 (假设 Z ∈ [-3, 3])
                ((z + 3.0) / 6.0 * 100.0).clamp(0.0, 100.0)
            }
            ScoringMethod::MinMax => {
                let min = all_values
                    .iter()
                    .map(|(_, v)| *v)
                    .fold(f64::INFINITY, f64::min);
                let max = all_values
                    .iter()
                    .map(|(_, v)| *v)
                    .fold(f64::NEG_INFINITY, f64::max);

                if (max - min).abs() < 1e-10 {
                    return 50.0;
                }

                let normalized = (value - min) / (max - min);
                if factor.is_higher_better() {
                    normalized * 100.0
                } else {
                    (1.0 - normalized) * 100.0
                }
            }
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn weights(&self) -> &[FactorWeight] {
        &self.weights
    }
}

/// 单只股票的综合评分
#[derive(Debug, Clone)]
pub struct StockScore {
    pub symbol: String,
    /// 综合得分 (0-100)
    pub total_score: f64,
    /// 各因子得分详情
    pub factor_details: Vec<FactorScore>,
}

/// 单个因子的得分
#[derive(Debug, Clone)]
pub struct FactorScore {
    pub factor: FactorType,
    /// 原始值
    pub raw_value: f64,
    /// 标准化得分 (0-100)
    pub score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_growth_model() {
        let model = FactorModel::value_growth();
        let data = Fundamentals::generate_mock(30, 42);
        let scores = model.score_all(&data);

        assert_eq!(scores.len(), 30);
        // 检查排序：第一名得分应高于最后一名
        assert!(scores[0].total_score >= scores[scores.len() - 1].total_score);

        // 打印前3名
        for s in scores.iter().take(3) {
            println!("  {} → 总分: {:.1}", s.symbol, s.total_score);
        }
    }

    #[test]
    fn test_all_preset_models() {
        let data = Fundamentals::generate_mock(20, 99);
        let models = [
            FactorModel::value_growth(),
            FactorModel::pure_value(),
            FactorModel::pure_growth(),
            FactorModel::high_quality(),
        ];

        for model in &models {
            let scores = model.score_all(&data);
            assert_eq!(scores.len(), 20);
            println!(
                "  {} → 最高分: {:.1} ({})",
                model.name(),
                scores[0].total_score,
                scores[0].symbol
            );
        }
    }

    #[test]
    fn test_scoring_methods() {
        let data = Fundamentals::generate_mock(20, 77);
        let methods = [
            ScoringMethod::PercentileRank,
            ScoringMethod::ZScore,
            ScoringMethod::MinMax,
        ];

        for method in &methods {
            let weights = vec![
                FactorWeight {
                    factor: FactorType::PeTtm,
                    weight: 0.5,
                },
                FactorWeight {
                    factor: FactorType::Roe,
                    weight: 0.5,
                },
            ];
            let model = FactorModel::try_new(weights, *method, "Test Model").unwrap();
            let scores = model.score_all(&data);
            assert_eq!(scores.len(), 20);
            println!("  {:?} → 最高分: {:.1}", method, scores[0].total_score);
        }
    }

    #[test]
    fn test_factor_direction() {
        // PE 越低越好
        assert!(!FactorType::PeTtm.is_higher_better());
        // ROE 越高越好
        assert!(FactorType::Roe.is_higher_better());
        // 资产负债率越低越好
        assert!(!FactorType::DebtToAssets.is_higher_better());
        // 流动比率越高越好
        assert!(FactorType::CurrentRatio.is_higher_better());
    }

    #[test]
    fn test_invalid_weights() {
        let weights = vec![
            FactorWeight {
                factor: FactorType::PeTtm,
                weight: 0.5,
            },
            FactorWeight {
                factor: FactorType::Roe,
                weight: 0.5,
            },
            FactorWeight {
                factor: FactorType::Roa,
                weight: 0.5,
            }, // 总和 1.5，应返回错误
        ];
        let result = FactorModel::try_new(weights, ScoringMethod::PercentileRank, "Bad");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("权重之和"));
    }
}
