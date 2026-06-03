use chrono::NaiveDate;

use super::data::Candle;
use super::factor_model::{FactorModel, StockScore};
use super::fundamental::Fundamentals;
use super::fundamental_processing::{FactorPipelineConfig, PointInTimeAligner};
use super::strategy::{Signal, Strategy};

/// 基本面多因子选股策略
///
/// 结合财报数据（PE, PB, ROE, 营收增速等），构建"低估值+高成长"的多因子选股策略。
/// 与价格动量信号结合，实现基本面+技术面的双轮驱动。
///
/// **P0 优化**：
/// - 使用 Point-in-Time 对齐，避免前瞻偏差（财报发布延迟）
/// - 每根 K 线重新计算因子得分（而非全局一次性计算）
/// - 支持 Winsorize 缩尾 + 行业中性化
pub struct FundamentalStrategy {
    /// 多因子评分模型
    model: FactorModel,
    /// 所有股票的基本面数据
    fundamentals: Vec<Fundamentals>,
    /// 选取得分前 N 名的股票
    top_n: usize,
    /// 价格动量确认周期（EMA天数）
    momentum_period: usize,
    /// 是否需要价格动量确认（true = 基本面+技术面共振才买入）
    require_momentum: bool,
    /// 当前持仓的股票符号（模拟单只标的场景）
    current_symbol: Option<String>,
    /// 策略名称
    name: String,
    /// 因子处理流水线配置
    pipeline_config: FactorPipelineConfig,
    /// 基本面质量阈值
    quality_threshold: f64,
}

impl FundamentalStrategy {
    /// 创建新的基本面选股策略
    pub fn new(
        model: FactorModel,
        fundamentals: Vec<Fundamentals>,
        top_n: usize,
        momentum_period: usize,
        require_momentum: bool,
    ) -> Self {
        assert!(top_n > 0, "top_n 必须大于 0");
        let name = format!(
            "多因子选股 ({} | Top-{} | {}动量确认)",
            model.name(),
            top_n,
            if require_momentum { "需" } else { "免" }
        );

        Self {
            model,
            fundamentals,
            top_n,
            momentum_period,
            require_momentum,
            current_symbol: None,
            name,
            pipeline_config: FactorPipelineConfig::default(),
            quality_threshold: 55.0,
        }
    }

    /// 设置因子处理流水线配置
    pub fn with_pipeline_config(mut self, config: FactorPipelineConfig) -> Self {
        self.pipeline_config = config;
        self
    }

    /// 设置基本面质量阈值
    pub fn with_quality_threshold(mut self, threshold: f64) -> Self {
        self.quality_threshold = threshold;
        self
    }

    /// 使用 GARP（低估值+高成长）默认配置
    pub fn garp(fundamentals: Vec<Fundamentals>, top_n: usize) -> Self {
        Self::new(FactorModel::value_growth(), fundamentals, top_n, 20, true)
    }

    /// 计算多因子排名，返回入选股票列表
    pub fn rank_stocks(&self) -> Vec<StockScore> {
        let (scores, _) = self
            .model
            .score_all_processed(&self.fundamentals, &self.pipeline_config);
        scores.into_iter().take(self.top_n).collect()
    }

    /// 设置当前跟踪的股票（用于单标的回测场景）
    pub fn set_current_symbol(&mut self, symbol: Option<String>) {
        self.current_symbol = symbol;
    }

    /// 获取流水线配置的可变引用
    pub fn pipeline_config_mut(&mut self) -> &mut FactorPipelineConfig {
        &mut self.pipeline_config
    }
}

impl Strategy for FundamentalStrategy {
    fn name(&self) -> &str {
        &self.name
    }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        if candles.is_empty() {
            return vec![];
        }

        // ===== P0 修复：使用 Point-in-Time 对齐 =====
        // 将 K 线的 DateTime<Utc> 转为 NaiveDate，然后用 PIT 对齐器获取该日期可见的财报数据
        let candle_dates: Vec<NaiveDate> =
            candles.iter().map(|c| c.timestamp.date_naive()).collect();
        let pit_snapshots =
            PointInTimeAligner::timeline_snapshots(&self.fundamentals, &candle_dates);

        let mut signals = vec![Signal::Hold; candles.len()];
        let mut in_position = false;

        for i in self.momentum_period..candles.len() {
            // 获取该时间点可见的财报数据
            let (_, visible_fundamentals) = &pit_snapshots[i];

            if visible_fundamentals.is_empty() {
                continue;
            }

            // 在可见数据上计算因子得分（每次重新计算，反映最新可得信息）
            let (scores, _) = self
                .model
                .score_all_processed(visible_fundamentals, &self.pipeline_config);

            if scores.is_empty() {
                continue;
            }

            // 获取入选股票
            let selected: Vec<&StockScore> = scores.iter().take(self.top_n).collect();

            // 提取入选股票的平均得分（用于判断市场整体质量）
            let avg_score: f64 =
                selected.iter().map(|s| s.total_score).sum::<f64>() / selected.len() as f64;

            // 计算价格动量指标
            let closes: Vec<f64> = candles[..=i].iter().map(|c| c.close).collect();
            let momentum = compute_momentum(&closes, self.momentum_period);
            let price_momentum = momentum.last().copied().unwrap_or(0.0);

            if !in_position {
                // 买入条件：
                // 1. 基本面质量好（入选股票平均得分超过阈值）
                // 2. 如果 require_momentum，则价格动量为正（上涨趋势）
                let quality_ok = avg_score > self.quality_threshold;
                let momentum_ok = !self.require_momentum || price_momentum > 0.0;

                if quality_ok && momentum_ok {
                    signals[i] = Signal::Buy;
                    in_position = true;
                }
            } else {
                // 卖出条件：
                // 1. 基本面质量下降（入选股票平均得分跌破阈值）
                // 2. 价格动量转负（下跌趋势）
                let quality_bad = avg_score < self.quality_threshold - 10.0;
                let momentum_bad = self.require_momentum && price_momentum < -1.0;

                if quality_bad || momentum_bad {
                    signals[i] = Signal::Sell;
                    in_position = false;
                }
            }
        }

        signals
    }
}

/// 计算价格动量（EMA 差值）
fn compute_momentum(closes: &[f64], period: usize) -> Vec<f64> {
    if closes.len() < period {
        return vec![0.0; closes.len()];
    }

    let mut ema = vec![f64::NAN; closes.len()];
    let multiplier = 2.0 / (period as f64 + 1.0);

    // 初始化 EMA
    let sum: f64 = closes[..period].iter().sum();
    ema[period - 1] = sum / period as f64;

    // 计算 EMA
    for i in period..closes.len() {
        ema[i] = (closes[i] - ema[i - 1]) * multiplier + ema[i - 1];
    }

    // 动量 = 价格 - EMA（正值 = 价格在均线上方 = 多头动量）
    let mut result = vec![0.0; closes.len()];
    for i in 0..closes.len() {
        if !ema[i].is_nan() {
            result[i] = closes[i] - ema[i];
        }
    }

    result
}

/// 纯基本面选股报告（不含价格动量）
/// 用于批量股票筛选场景
pub struct FundamentalScreener {
    model: FactorModel,
    fundamentals: Vec<Fundamentals>,
    pipeline_config: FactorPipelineConfig,
}

impl FundamentalScreener {
    pub fn new(model: FactorModel, fundamentals: Vec<Fundamentals>) -> Self {
        Self {
            model,
            fundamentals,
            pipeline_config: FactorPipelineConfig::default(),
        }
    }

    /// 设置流水线配置
    pub fn with_pipeline_config(mut self, config: FactorPipelineConfig) -> Self {
        self.pipeline_config = config;
        self
    }

    /// 运行筛选，返回排名前 N 的股票及详细因子得分
    pub fn screen(&self, top_n: usize) -> Vec<StockScore> {
        let (scores, _) = self
            .model
            .score_all_processed(&self.fundamentals, &self.pipeline_config);
        scores.into_iter().take(top_n).collect()
    }

    /// 获取模型名称
    pub fn model_name(&self) -> &str {
        self.model.name()
    }

    /// 打印筛选报告
    pub fn print_report(&self, top_n: usize) {
        let scores = self.screen(top_n);

        println!("\n╔══════════════════════════════════════════════════════════╗");
        println!("║           多因子选股报告: {:<22} ║", self.model.name());
        println!("╚══════════════════════════════════════════════════════════╝");
        println!(
            "\n{:<12} {:>8}  {:>8}  {:>8}  {:>8}  {:>8}  {:>8}",
            "股票代码", "总分", "PE", "PB", "ROE", "营收增速", "利润增速"
        );
        println!("{}", "─".repeat(72));

        for s in &scores {
            // 提取关键因子值
            use super::factor_model::FactorType;
            let pe = s
                .factor_details
                .iter()
                .find(|f| matches!(f.factor, FactorType::PeTtm))
                .map(|f| f.raw_value)
                .unwrap_or(f64::NAN);
            let pb = s
                .factor_details
                .iter()
                .find(|f| matches!(f.factor, FactorType::Pb))
                .map(|f| f.raw_value)
                .unwrap_or(f64::NAN);
            let roe = s
                .factor_details
                .iter()
                .find(|f| matches!(f.factor, FactorType::Roe))
                .map(|f| f.raw_value)
                .unwrap_or(f64::NAN);
            let rev_growth = s
                .factor_details
                .iter()
                .find(|f| matches!(f.factor, FactorType::RevenueGrowthYoy))
                .map(|f| f.raw_value)
                .unwrap_or(f64::NAN);
            let profit_growth = s
                .factor_details
                .iter()
                .find(|f| matches!(f.factor, FactorType::NetProfitGrowthYoy))
                .map(|f| f.raw_value)
                .unwrap_or(f64::NAN);

            println!(
                "{:<12} {:>7.1}  {:>7.1}  {:>7.2}  {:>7.1}% {:>7.1}% {:>7.1}%",
                s.symbol, s.total_score, pe, pb, roe, rev_growth, profit_growth
            );
        }
        println!();

        // 打印因子权重
        println!("📊 因子权重:");
        for w in self.model.weights() {
            println!(
                "   {:<15} {:.0}%",
                w.factor.display_name(),
                w.weight * 100.0
            );
        }
    }
}

/// 基本面因子分析（对比不同模型下的股票排名）
pub struct FactorAnalyzer {
    fundamentals: Vec<Fundamentals>,
    pipeline_config: FactorPipelineConfig,
}

impl FactorAnalyzer {
    pub fn new(fundamentals: Vec<Fundamentals>) -> Self {
        Self {
            fundamentals,
            pipeline_config: FactorPipelineConfig::default(),
        }
    }

    /// 设置流水线配置
    pub fn with_pipeline_config(mut self, config: FactorPipelineConfig) -> Self {
        self.pipeline_config = config;
        self
    }

    /// 运行多个模型并对比结果
    pub fn compare_models(&self, top_n: usize) {
        let models = [
            ("GARP (低估值+高成长)", FactorModel::value_growth()),
            ("纯价值", FactorModel::pure_value()),
            ("纯成长", FactorModel::pure_growth()),
            ("高质量 (高ROE)", FactorModel::high_quality()),
        ];

        println!("\n╔══════════════════════════════════════════════════════════╗");
        println!("║              多因子模型对比分析                          ║");
        println!("╚══════════════════════════════════════════════════════════╝");

        for (name, model) in &models {
            let (scores, _) = model.score_all_processed(&self.fundamentals, &self.pipeline_config);
            let top_scores: Vec<&StockScore> = scores.iter().take(top_n).collect();

            let avg_score = if !top_scores.is_empty() {
                top_scores.iter().map(|s| s.total_score).sum::<f64>() / top_scores.len() as f64
            } else {
                0.0
            };

            let top_symbol = top_scores
                .first()
                .map(|s| s.symbol.as_str())
                .unwrap_or("N/A");
            let top_score = top_scores.first().map(|s| s.total_score).unwrap_or(0.0);

            println!(
                "\n  📈 {:<20} → 入选 {} 只, 平均得分: {:.1}, 榜首: {} ({:.1})",
                name,
                scores.len(),
                avg_score,
                top_symbol,
                top_score
            );

            // 打印前 3 名
            for (rank, s) in top_scores.iter().take(3).enumerate() {
                println!(
                    "      #{} {:<12} → 总分: {:.1}",
                    rank + 1,
                    s.symbol,
                    s.total_score
                );
            }
        }

        // 计算不同模型之间的重合度
        self.compute_overlap(top_n, &models);
    }

    /// 计算不同模型选出的股票重合度
    fn compute_overlap(&self, top_n: usize, models: &[(&str, FactorModel)]) {
        println!("\n🔍 模型间选股重合度分析:");

        let model_symbols: Vec<(String, Vec<String>)> = models
            .iter()
            .map(|(name, model)| {
                let (scores, _) =
                    model.score_all_processed(&self.fundamentals, &self.pipeline_config);
                let symbols: Vec<String> = scores
                    .iter()
                    .take(top_n)
                    .map(|s| s.symbol.clone())
                    .collect();
                (name.to_string(), symbols)
            })
            .collect();

        for i in 0..model_symbols.len() {
            for j in (i + 1)..model_symbols.len() {
                let set_a: std::collections::HashSet<&str> =
                    model_symbols[i].1.iter().map(|s| s.as_str()).collect();
                let set_b: std::collections::HashSet<&str> =
                    model_symbols[j].1.iter().map(|s| s.as_str()).collect();
                let overlap: usize = set_a.intersection(&set_b).count();
                let overlap_pct = overlap as f64 / top_n as f64 * 100.0;

                println!(
                    "   {} ↔ {}: {}/{} ({:.0}%)",
                    model_symbols[i].0, model_symbols[j].0, overlap, top_n, overlap_pct
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> Vec<Fundamentals> {
        Fundamentals::generate_mock(30, 42)
    }

    fn create_test_candles() -> Vec<Candle> {
        super::super::data::DataSource::generate_mock(200, 100.0)
    }

    #[test]
    fn test_garp_strategy() {
        let data = create_test_data();
        let strategy = FundamentalStrategy::garp(data, 5);

        let ranking = strategy.rank_stocks();
        assert_eq!(ranking.len(), 5);
        println!("GARP Top-5:");
        for (i, s) in ranking.iter().enumerate() {
            println!("  #{} {} → {:.1}", i + 1, s.symbol, s.total_score);
        }
    }

    #[test]
    fn test_strategy_generate_signals() {
        let data = create_test_data();
        let candles = create_test_candles();
        let strategy = FundamentalStrategy::garp(data, 5);

        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());

        let buy_count = signals.iter().filter(|s| **s == Signal::Buy).count();
        let sell_count = signals.iter().filter(|s| **s == Signal::Sell).count();
        println!(
            "信号分布: Buy={}, Sell={}, Hold={}",
            buy_count,
            sell_count,
            signals.len() - buy_count - sell_count
        );
    }

    #[test]
    fn test_strategy_without_momentum() {
        let data = create_test_data();
        let candles = create_test_candles();
        let strategy = FundamentalStrategy::new(
            FactorModel::value_growth(),
            data,
            5,
            10,
            false, // 不需要动量确认
        );

        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_strategy_with_processed_scoring() {
        let data = create_test_data();
        let candles = create_test_candles();
        let strategy = FundamentalStrategy::new(FactorModel::value_growth(), data, 5, 10, false)
            .with_pipeline_config(FactorPipelineConfig {
                neutralize: true,
                ..Default::default()
            });

        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_screener() {
        let data = create_test_data();
        let screener = FundamentalScreener::new(FactorModel::value_growth(), data);
        screener.print_report(5);

        let top = screener.screen(5);
        assert_eq!(top.len(), 5);
        assert!(top[0].total_score >= top[4].total_score);
    }

    #[test]
    fn test_screener_with_pipeline() {
        let data = create_test_data();
        let screener = FundamentalScreener::new(FactorModel::value_growth(), data)
            .with_pipeline_config(FactorPipelineConfig {
                neutralize: true,
                ..Default::default()
            });

        let top = screener.screen(5);
        assert_eq!(top.len(), 5);
    }

    #[test]
    fn test_analyzer() {
        let data = create_test_data();
        let analyzer = FactorAnalyzer::new(data);
        analyzer.compare_models(5);
    }

    #[test]
    fn test_all_preset_models_on_strategy() {
        let data = create_test_data();
        let candles = create_test_candles();

        let models = [
            FactorModel::value_growth(),
            FactorModel::pure_value(),
            FactorModel::pure_growth(),
            FactorModel::high_quality(),
        ];

        for model in &models {
            let strategy = FundamentalStrategy::new(model.clone(), data.clone(), 3, 10, false);
            let signals = strategy.generate(&candles);
            assert_eq!(signals.len(), candles.len());
            let buy_count = signals.iter().filter(|s| **s == Signal::Buy).count();
            let sell_count = signals.iter().filter(|s| **s == Signal::Sell).count();
            println!(
                "  {} → Buy: {}, Sell: {}",
                model.name(),
                buy_count,
                sell_count
            );
        }
    }

    #[test]
    fn test_momentum_calculation() {
        let closes = (0..50).map(|i| 100.0 + i as f64 * 0.5).collect::<Vec<_>>();
        let momentum = compute_momentum(&closes, 10);

        // 上涨趋势中，动量应该大部分为正
        let positive_count = momentum.iter().filter(|m| **m > 0.0).count();
        assert!(
            positive_count > momentum.len() / 2,
            "上涨趋势中大部分动量应为正"
        );
    }

    #[test]
    fn test_point_in_time_scoring() {
        // 验证 PIT 对齐后评分正确性
        let data = create_test_data();
        let candles = create_test_candles();

        // 确保每根 K 线都有一份财报数据可用
        let strategy = FundamentalStrategy::new(FactorModel::value_growth(), data, 5, 10, false);

        let signals = strategy.generate(&candles);

        // 信号数量应与 K 线一致
        assert_eq!(signals.len(), candles.len());

        // 打印信号分布
        let buy_count = signals.iter().filter(|s| **s == Signal::Buy).count();
        let sell_count = signals.iter().filter(|s| **s == Signal::Sell).count();
        println!(
            "PIT 对齐信号分布: Buy={}, Sell={}, Hold={}",
            buy_count,
            sell_count,
            signals.len() - buy_count - sell_count
        );
    }
}
