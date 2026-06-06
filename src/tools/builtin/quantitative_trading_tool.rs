//! Agent tool wrapping the built-in quantitative trading library.

use crate::tools::registry::{Tool, ToolParameter, ToolRegistry};
use crate::trading::backtest::{BacktestEngine, PositionSizing, StopLoss};
use crate::trading::data::Candle;
use crate::trading::indicators;
use crate::trading::report::BacktestResult;
use crate::trading::strategy::{
    BollingerBandsStrategy, MacdMode, MacdStrategy, RsiMeanReversion, SmaCrossover, Strategy,
    TripleEmaStrategy, VwapRsiStrategy,
};
use crate::trading::strategy_comparison::StrategyComparator;
use crate::trading::tencent::{fetch_klines, normalize_symbol, TencentInterval};
use chrono::{Duration, Utc};
use rand::Rng;
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(QuantitativeTradingTool));
}

struct QuantitativeTradingTool;

const DEFAULT_STRATEGIES: &[&str] = &[
    "sma_crossover",
    "rsi",
    "macd",
    "triple_ema",
    "bollinger",
    "vwap_rsi",
];

#[async_trait::async_trait]
impl Tool for QuantitativeTradingTool {
    fn name(&self) -> &str {
        "quantitative_trading"
    }

    fn description(&self) -> &str {
        "Quantitative trading: backtest strategies, compare/rank strategies, compute indicators, \
         fetch market candles (Tencent), or generate synthetic OHLCV for testing. \
         Actions: list_strategies, fetch_data, backtest, compare, indicator."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".into(),
                parameter_type: "string".into(),
                description: "list_strategies | fetch_data | backtest | compare | indicator".into(),
                required: true,
            },
            ToolParameter {
                name: "strategy".into(),
                parameter_type: "string".into(),
                description: "Strategy id for backtest (see list_strategies)".into(),
                required: false,
            },
            ToolParameter {
                name: "strategies".into(),
                parameter_type: "string".into(),
                description: "Comma-separated strategy ids for compare (default: built-in set)"
                    .into(),
                required: false,
            },
            ToolParameter {
                name: "symbol".into(),
                parameter_type: "string".into(),
                description: "Market symbol, e.g. sh000001, sz000001, us_aapl".into(),
                required: false,
            },
            ToolParameter {
                name: "limit".into(),
                parameter_type: "number".into(),
                description: "Number of candles to fetch (max 320, default 200)".into(),
                required: false,
            },
            ToolParameter {
                name: "candles".into(),
                parameter_type: "string".into(),
                description: "JSON array of OHLCV candles for backtest/compare when not fetching"
                    .into(),
                required: false,
            },
            ToolParameter {
                name: "synthetic_bars".into(),
                parameter_type: "number".into(),
                description: "Generate synthetic candles when no symbol/candles (default 500)"
                    .into(),
                required: false,
            },
            ToolParameter {
                name: "initial_capital".into(),
                parameter_type: "number".into(),
                description: "Starting capital (default 100000)".into(),
                required: false,
            },
            ToolParameter {
                name: "commission_rate".into(),
                parameter_type: "number".into(),
                description: "Commission rate per trade (default 0.001)".into(),
                required: false,
            },
            ToolParameter {
                name: "slippage".into(),
                parameter_type: "number".into(),
                description: "Slippage rate (default 0.001)".into(),
                required: false,
            },
            ToolParameter {
                name: "ranking_method".into(),
                parameter_type: "string".into(),
                description: "compare ranking: composite | sharpe | calmar (default composite)"
                    .into(),
                required: false,
            },
            ToolParameter {
                name: "indicator".into(),
                parameter_type: "string".into(),
                description: "indicator action: sma | ema | rsi | macd".into(),
                required: false,
            },
            ToolParameter {
                name: "period".into(),
                parameter_type: "number".into(),
                description: "Indicator period (default 14)".into(),
                required: false,
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = param_str(params, "action")?;
        match action.as_str() {
            "list_strategies" => Ok(json!({
                "strategies": DEFAULT_STRATEGIES,
                "descriptions": {
                    "sma_crossover": "SMA fast/slow crossover (10/30)",
                    "rsi": "RSI mean reversion (14)",
                    "macd": "MACD crossover (12/26/9)",
                    "triple_ema": "Triple EMA (5/13/34)",
                    "bollinger": "Bollinger bands mean reversion",
                    "vwap_rsi": "VWAP deviation + RSI",
                }
            })),
            "fetch_data" => fetch_data(params).await,
            "backtest" => backtest(params).await,
            "compare" => compare(params).await,
            "indicator" => compute_indicator(params),
            _ => Err(format!(
                "Unknown action '{action}'. Use list_strategies, fetch_data, backtest, compare, or indicator."
            )),
        }
    }
}

async fn fetch_data(params: &HashMap<String, Value>) -> Result<Value, String> {
    let symbol = param_str(params, "symbol")?;
    let limit = param_usize(params, "limit").unwrap_or(200).min(320);
    let symbol = normalize_symbol(&symbol);
    let symbol_for_fetch = symbol.clone();
    let candles = tokio::task::spawn_blocking(move || {
        fetch_klines(&symbol_for_fetch, TencentInterval::Day, limit)
    })
    .await
    .map_err(|e| format!("fetch task failed: {e}"))??;

    Ok(json!({
        "symbol": symbol,
        "count": candles.len(),
        "candles": candles_to_json(&candles),
    }))
}

async fn backtest(params: &HashMap<String, Value>) -> Result<Value, String> {
    let strategy_name = param_str(params, "strategy")?;
    let candles = load_candles(params).await?;
    let initial_capital = param_f64(params, "initial_capital").unwrap_or(100_000.0);
    let commission_rate = param_f64(params, "commission_rate").unwrap_or(0.001);
    let slippage = param_f64(params, "slippage").unwrap_or(0.001);

    let strategy = build_strategy(&strategy_name)?;
    let result = run_backtest(
        strategy.as_ref(),
        &candles,
        initial_capital,
        commission_rate,
        slippage,
    );
    Ok(backtest_result_json(&strategy_name, &result))
}

async fn compare(params: &HashMap<String, Value>) -> Result<Value, String> {
    let candles = load_candles(params).await?;
    let initial_capital = param_f64(params, "initial_capital").unwrap_or(100_000.0);
    let commission_rate = param_f64(params, "commission_rate").unwrap_or(0.001);
    let slippage = param_f64(params, "slippage").unwrap_or(0.001);
    let ranking = param_str(params, "ranking_method").unwrap_or_else(|_| "composite".into());

    let names: Vec<String> = match param_str(params, "strategies") {
        Ok(s) => s
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect(),
        Err(_) => DEFAULT_STRATEGIES.iter().map(|s| s.to_string()).collect(),
    };
    if names.is_empty() {
        return Err("No strategies specified for compare".to_string());
    }

    let mut strategies: Vec<Box<dyn Strategy>> = Vec::new();
    for name in &names {
        strategies.push(build_strategy(name)?);
    }

    let ranking = match ranking.as_str() {
        "sharpe" => StrategyComparator::rank_by_sharpe(
            &strategies,
            &candles,
            initial_capital,
            commission_rate,
            slippage,
        ),
        "calmar" => StrategyComparator::rank_by_calmar(
            &strategies,
            &candles,
            initial_capital,
            commission_rate,
            slippage,
        ),
        _ => StrategyComparator::compare_and_rank(
            &strategies,
            &candles,
            initial_capital,
            commission_rate,
            slippage,
        ),
    };

    let report = StrategyComparator::generate_comparison_report(&ranking);
    Ok(json!({
        "ranking_method": ranking.ranking_method,
        "total_strategies": ranking.total_strategies,
        "entries": ranking.entries,
        "benchmark": ranking.benchmark,
        "report_markdown": report,
    }))
}

fn compute_indicator(params: &HashMap<String, Value>) -> Result<Value, String> {
    let ind = param_str(params, "indicator")?;
    let period = param_usize(params, "period").unwrap_or(14).max(2);
    let closes = parse_closes_from_candles_param(params)?;

    let (series, meta) = match ind.as_str() {
        "sma" => (
            indicators::sma(&closes, period),
            json!({ "type": "sma", "period": period }),
        ),
        "ema" => (
            indicators::ema(&closes, period),
            json!({ "type": "ema", "period": period }),
        ),
        "rsi" => (
            indicators::rsi(&closes, period),
            json!({ "type": "rsi", "period": period }),
        ),
        "macd" => {
            let out = indicators::macd(&closes, 12, 26, 9);
            return Ok(json!({
                "meta": { "type": "macd", "fast": 12, "slow": 26, "signal": 9 },
                "macd": tail_series(&out.macd_line, 30),
                "signal": tail_series(&out.signal_line, 30),
                "histogram": tail_series(&out.histogram, 30),
            }));
        }
        other => {
            return Err(format!(
                "Unknown indicator '{other}'. Use sma, ema, rsi, or macd."
            ))
        }
    };

    Ok(json!({
        "meta": meta,
        "values": tail_series(&series, 30),
    }))
}

async fn load_candles(params: &HashMap<String, Value>) -> Result<Vec<Candle>, String> {
    if let Ok(raw) = param_str(params, "candles") {
        return parse_candles_json(&raw);
    }
    if param_str(params, "symbol").is_ok() {
        let v = fetch_data(params).await?;
        let arr = v
            .get("candles")
            .ok_or_else(|| "fetch_data missing candles".to_string())?;
        let raw = serde_json::to_string(arr).map_err(|e| format!("serialize candles: {e}"))?;
        return parse_candles_json(&raw);
    }
    let bars = param_usize(params, "synthetic_bars")
        .unwrap_or(500)
        .clamp(50, 5000);
    Ok(generate_synthetic_candles(bars, 100.0))
}

fn run_backtest(
    strategy: &dyn Strategy,
    candles: &[Candle],
    initial_capital: f64,
    commission_rate: f64,
    slippage: f64,
) -> BacktestResult {
    let mut engine = BacktestEngine::new(initial_capital, commission_rate, slippage)
        .with_position_sizing(PositionSizing::FixedFractional(0.5))
        .with_stop_loss(StopLoss::AtrTrailing(3.0))
        .with_max_drawdown_stop(25.0);
    match engine.run(candles, strategy) {
        Ok(trades) => BacktestResult::new(&engine, candles, &trades),
        Err(_) => BacktestResult::from_trades(&[], initial_capital),
    }
}

fn build_strategy(name: &str) -> Result<Box<dyn Strategy>, String> {
    let s: Box<dyn Strategy> = match name {
        "sma_crossover" => Box::new(SmaCrossover::new(10, 30)),
        "rsi" => Box::new(RsiMeanReversion::new(14, 30.0, 70.0)),
        "macd" => Box::new(MacdStrategy::new(12, 26, 9, MacdMode::Crossover)),
        "triple_ema" => Box::new(TripleEmaStrategy::new(5, 13, 34)),
        "bollinger" => Box::new(BollingerBandsStrategy::new(20, 2.0, 0.05, 0.05, false)),
        "vwap_rsi" => Box::new(VwapRsiStrategy::new(14, 30.0, 70.0, 1.0)),
        other => {
            return Err(format!(
                "Unknown strategy '{other}'. Call action=list_strategies for valid ids."
            ));
        }
    };
    Ok(s)
}

fn backtest_result_json(strategy: &str, result: &BacktestResult) -> Value {
    let e = &result.engine;
    json!({
        "strategy": strategy,
        "total_bars": result.total_bars,
        "metrics": {
            "initial_capital": e.initial_capital,
            "final_equity": e.final_equity,
            "total_return_pct": e.total_return_pct,
            "sharpe_ratio": e.sharpe_ratio,
            "calmar_ratio": e.calmar_ratio,
            "sortino_ratio": e.sortino_ratio,
            "max_drawdown_pct": e.max_drawdown_pct,
            "win_rate": e.win_rate,
            "profit_factor": e.profit_factor,
            "total_trades": e.total_trades,
        }
    })
}

fn candles_to_json(candles: &[Candle]) -> Value {
    serde_json::to_value(
        candles
            .iter()
            .map(|c| {
                json!({
                    "timestamp": c.timestamp.to_rfc3339(),
                    "open": c.open,
                    "high": c.high,
                    "low": c.low,
                    "close": c.close,
                    "volume": c.volume,
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or(json!([]))
}

fn parse_candles_json(raw: &str) -> Result<Vec<Candle>, String> {
    #[derive(serde::Deserialize)]
    struct RawCandle {
        timestamp: Option<String>,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
    }

    let rows: Vec<RawCandle> =
        serde_json::from_str(raw).map_err(|e| format!("parse candles JSON: {e}"))?;
    let start = Utc::now() - Duration::days(rows.len() as i64);
    Ok(rows
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            let ts = r
                .timestamp
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|| start + Duration::days(i as i64));
            Candle::new(ts, r.open, r.high, r.low, r.close, r.volume)
        })
        .collect())
}

fn parse_closes_from_candles_param(params: &HashMap<String, Value>) -> Result<Vec<f64>, String> {
    if let Ok(raw) = param_str(params, "candles") {
        let candles = parse_candles_json(&raw)?;
        return Ok(candles.iter().map(|c| c.close).collect());
    }
    let bars = param_usize(params, "synthetic_bars").unwrap_or(200);
    let candles = generate_synthetic_candles(bars, 100.0);
    Ok(candles.iter().map(|c| c.close).collect())
}

fn generate_synthetic_candles(count: usize, start_price: f64) -> Vec<Candle> {
    let mut rng = rand::thread_rng();
    let mut candles = Vec::with_capacity(count);
    let start = Utc::now() - Duration::days(count as i64);
    let mut price = start_price;
    for i in 0..count {
        let change = rng.gen_range(-0.02..0.02);
        let open = price;
        let close = (price * (1.0 + change)).max(0.1);
        let high = open.max(close) * (1.0 + rng.gen_range(0.0..0.005));
        let low = open.min(close) * (1.0 - rng.gen_range(0.0..0.005));
        let volume = rng.gen_range(10_000.0..1_000_000.0);
        candles.push(Candle::new(
            start + Duration::days(i as i64),
            open,
            high,
            low,
            close,
            volume,
        ));
        price = close;
    }
    candles
}

fn tail_series(data: &[f64], n: usize) -> Vec<f64> {
    let start = data.len().saturating_sub(n);
    data[start..].to_vec()
}

fn param_str(params: &HashMap<String, Value>, key: &str) -> Result<String, String> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Missing required parameter: {key}"))
}

fn param_f64(params: &HashMap<String, Value>, key: &str) -> Option<f64> {
    params.get(key).and_then(|v| v.as_f64())
}

fn param_usize(params: &HashMap<String, Value>, key: &str) -> Option<usize> {
    params.get(key).and_then(|v| v.as_u64()).map(|n| n as usize)
}
