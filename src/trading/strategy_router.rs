//! 策略路由模块
//! 根据市场状态自动选择/推荐最佳策略

use super::data::Candle;
use super::market_regime::{MarketRegime, MarketRegimeDetector};
use super::strategy::{
    BollingerBandsStrategy, MacdMode, MacdStrategy, RsiMeanReversion, SmaCrossoverWithRsi,
    Strategy, TripleEmaStrategy, TurtleTradingStrategy, VwapRsiStrategy,
};

/// 策略路由器
/// 根据市场状态自动推荐或创建合适的策略
pub struct StrategyRouter {
    detector: MarketRegimeDetector,
}

impl Default for StrategyRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl StrategyRouter {
    pub fn new() -> Self {
        Self {
            detector: MarketRegimeDetector::new(),
        }
    }

    /// 检测当前市场状态
    pub fn detect(&self, candles: &[Candle]) -> MarketRegime {
        self.detector.detect_regime(candles)
    }

    /// 根据市场状态推荐策略名称
    pub fn recommend(&self, regime: MarketRegime) -> Vec<&'static str> {
        match regime {
            MarketRegime::StrongUptrend | MarketRegime::StrongDowntrend => {
                vec![
                    "Triple EMA Trend (5,13,34)",
                    "Triple EMA Trend (10,30,60)",
                    "MACD Crossover",
                    "Turtle Trading (20,10,20,2.0)",
                ]
            }
            MarketRegime::WeakUptrend | MarketRegime::WeakDowntrend => {
                vec![
                    "SMA Crossover + RSI Filter",
                    "Bollinger Bands MeanRev",
                    "MACD + Histogram",
                ]
            }
            MarketRegime::Ranging => {
                vec![
                    "RSI Mean Reversion (14, 30, 70)",
                    "RSI Mean Reversion (14, 20, 80)",
                    "Bollinger Bands MeanRev",
                    "VWAP + RSI Reversion",
                ]
            }
            MarketRegime::LowVolatilitySqueeze => {
                vec!["Bollinger Bands + Squeeze", "Turtle Trading (20,10,20,2.0)"]
            }
            MarketRegime::HighVolatilityBreakout => {
                vec![
                    "Turtle Trading (55,20,20,2.0)",
                    "Bollinger Bands + Squeeze",
                    "Triple EMA Trend (5,13,34)",
                ]
            }
        }
    }

    /// 根据市场状态创建策略实例
    pub fn create_strategies(&self, regime: MarketRegime) -> Vec<Box<dyn Strategy>> {
        match regime {
            MarketRegime::StrongUptrend | MarketRegime::StrongDowntrend => {
                vec![
                    Box::new(TripleEmaStrategy::new(5, 13, 34)),
                    Box::new(TripleEmaStrategy::new(10, 30, 60)),
                    Box::new(MacdStrategy::new(12, 26, 9, MacdMode::Crossover)),
                    Box::new(TurtleTradingStrategy::new(20, 10, 20, 2.0)),
                ]
            }
            MarketRegime::WeakUptrend | MarketRegime::WeakDowntrend => {
                vec![
                    Box::new(SmaCrossoverWithRsi::new(5, 20, 14, 30.0, 70.0)),
                    Box::new(BollingerBandsStrategy::new(20, 2.0, 1.0, 1.0, false)),
                    Box::new(MacdStrategy::new(
                        12,
                        26,
                        9,
                        MacdMode::CrossoverWithHistogram,
                    )),
                ]
            }
            MarketRegime::Ranging => {
                vec![
                    Box::new(RsiMeanReversion::new(14, 30.0, 70.0)),
                    Box::new(RsiMeanReversion::new(14, 20.0, 80.0)),
                    Box::new(BollingerBandsStrategy::new(20, 2.0, 1.0, 1.0, false)),
                    Box::new(VwapRsiStrategy::new(14, 30.0, 70.0, 1.0)),
                ]
            }
            MarketRegime::LowVolatilitySqueeze => {
                vec![
                    Box::new(BollingerBandsStrategy::new(20, 2.0, 1.0, 1.0, true)),
                    Box::new(TurtleTradingStrategy::new(20, 10, 20, 2.0)),
                ]
            }
            MarketRegime::HighVolatilityBreakout => {
                vec![
                    Box::new(TurtleTradingStrategy::new(55, 20, 20, 2.0)),
                    Box::new(BollingerBandsStrategy::new(20, 2.0, 1.0, 1.0, true)),
                    Box::new(TripleEmaStrategy::new(5, 13, 34)),
                ]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::data::DataSource;
    use super::*;

    #[test]
    fn test_strategy_router_recommends() {
        let router = StrategyRouter::new();

        for regime in &[
            MarketRegime::StrongUptrend,
            MarketRegime::StrongDowntrend,
            MarketRegime::WeakUptrend,
            MarketRegime::WeakDowntrend,
            MarketRegime::Ranging,
            MarketRegime::LowVolatilitySqueeze,
            MarketRegime::HighVolatilityBreakout,
        ] {
            let recs = router.recommend(*regime);
            assert!(
                !recs.is_empty(),
                "Should recommend strategies for {:?}",
                regime
            );

            let strategies = router.create_strategies(*regime);
            assert!(
                !strategies.is_empty(),
                "Should create strategies for {:?}",
                regime
            );
        }
    }

    #[test]
    fn test_strategy_router_detect() {
        let router = StrategyRouter::new();
        let candles = DataSource::generate_mock(200, 100.0);
        let regime = router.detect(&candles);
        // Should not panic, and should return some regime
        let _recs = router.recommend(regime);
    }
}
