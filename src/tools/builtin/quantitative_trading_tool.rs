use crate::tools::registry::{Tool, ToolParameter};
use crate::trading::backtest::BacktestEngine;
use crate::trading::data::{Candle, DataSource};
use crate::trading::report::BacktestResult;
use crate::trading::strategy::{
    self, BollingerBandsStrategy, KeltnerChannelsStrategy, KeltnerMode, MacdMode, MacdStrategy,
    ParabolicSarStrategy, RsiMeanReversion, SmaCrossover, SmaCrossoverWithRsi, Strategy,
    SuperTrendStrategy, TripleEmaStrategy, TurtleTradingStrategy, VwapRsiStrategy,
};
use chrono::DateTime;
use serde_json::Value;
use std::collections::HashMap;

pub struct QuantitativeTradingTool;

#[async_trait::async_trait]
impl Tool for QuantitativeTradingTool {
    fn name(&self) -> &str {
        "quantitative_trading"
    }

    fn description(&self) -> &str {
        "量化交易功能：回测策略比较、技术指标计算、真实市场数据获取。支持9+种策略（SMA Crossover、MACD、RSI、布林带、海龟交易法则、三均线、VWAP+RSI、组合策略等）。操作: backtest（策略回测）、indicators（技术指标）、strategies（策略列表）、fetch_data（从API获取真实K线数据）、mock_data（模拟数据）。数据来源支持Binance API、CSV文件、模拟数据。"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "action".into(),
                description: "操作类型: backtest, indicators, strategies, fetch_data, mock_data".into(),
                required: true,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "data_source".into(),
                description: "数据来源: 'binance'（从Binance API获取）、csv文件路径、'mock'（默认模拟数据）".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "symbol".into(),
                description: "交易对/品种（仅data_source=binance时有效）。如: BTCUSDT, ETHUSDT, BNBUSDT".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "interval".into(),
                description: "K线周期（仅data_source=binance时有效）。可选: 1m,5m,15m,30m,1h,4h,1d,1w,1M（默认1d）".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "limit".into(),
                description: "获取K线数量（仅data_source=binance时有效），最大1000（默认100）".into(),
                required: false,
                parameter_type: "number".into(),
            },
            ToolParameter {
                name: "strategy_names".into(),
                description: "指定要回测的策略名称（逗号分隔）。默认全部。可选: SMA_Crossover, SMA_RSI, RSI_MeanRev, MACD_Crossover, MACD_Histogram, MACD_Divergence, Turtle_Trading, Triple_EMA, Bollinger_MeanRev, Bollinger_Squeeze, VWAP_RSI, Ensemble".into(),
                required: false,
                parameter_type: "string".into(),
            },
            ToolParameter {
                name: "mock_count".into(),
                description: "模拟数据K线数量（默认1000，仅data_source=mock时有效）".into(),
                required: false,
                parameter_type: "number".into(),
            },
            ToolParameter {
                name: "mock_start_price".into(),
                description: "模拟数据起始价格（默认100.0）".into(),
                required: false,
                parameter_type: "number".into(),
            },
            ToolParameter {
                name: "initial_capital".into(),
                description: "初始资金（默认10000.0，仅backtest有效）".into(),
                required: false,
                parameter_type: "number".into(),
            },
            ToolParameter {
                name: "commission_rate".into(),
                description: "手续费率（默认0.001，仅backtest有效）".into(),
                required: false,
                parameter_type: "number".into(),
            },
            ToolParameter {
                name: "slippage".into(),
                description: "滑点（默认0.001，仅backtest有效）".into(),
                required: false,
                parameter_type: "number".into(),
            },
            ToolParameter {
                name: "export_json".into(),
                description: "是否导出结果到JSON文件（true/false）".into(),
                required: false,
                parameter_type: "boolean".into(),
            },
            ToolParameter {
                name: "export_csv".into(),
                description: "是否将获取的K线数据导出为CSV文件（true/false，仅fetch_data有效）".into(),
                required: false,
                parameter_type: "boolean".into(),
            },
        ]
    }

    async fn execute(&self, params: &HashMap<String, Value>) -> Result<Value, String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("backtest");

        match action {
            "backtest" => handle_backtest_sync(params),
            "indicators" => handle_indicators_sync(params),
            "strategies" => handle_strategies(),
            "fetch_data" => handle_fetch_data(params),
            "mock_data" => handle_mock_data(params),
            _ => Err(format!("未知操作: {}. 支持: backtest, indicators, strategies, fetch_data, mock_data", action)),
        }
    }
}

// ============================================================
// 辅助函数
// ============================================================

fn parse_bool(params: &HashMap<String, Value>, key: &str, default: bool) -> bool {
    params.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn parse_f64(params: &HashMap<String, Value>, key: &str, default: f64) -> f64 {
    params.get(key).and_then(|v| v.as_f64()).unwrap_or(default)
}

fn parse_usize(params: &HashMap<String, Value>, key: &str, default: usize) -> usize {
    params
        .get(key)
        .and_then(|v| v.as_f64())
        .map(|f| f as usize)
        .unwrap_or(default)
}

fn parse_str<'a>(params: &'a HashMap<String, Value>, key: &str, default: &'a str) -> &'a str {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or(default)
}

// ============================================================
// 数据加载（支持 binance API、CSV、mock）
// ============================================================

fn load_candles(params: &HashMap<String, Value>) -> Result<Vec<Candle>, String> {
    let data_source = parse_str(params, "data_source", "mock");

    match data_source {
        "mock" | "" => {
            let count = parse_usize(params, "mock_count", 1000);
            let start_price = parse_f64(params, "mock_start_price", 100.0);
            Ok(DataSource::generate_mock(count, start_price))
        }
        "binance" => {
            // Use blocking reqwest to fetch from Binance API
            fetch_binance_klines_blocking(params)
        }
        csv_path => {
            DataSource::from_csv(csv_path).map_err(|e| format!("加载CSV失败: {}", e))
        }
    }
}

// ============================================================
// Binance API 数据获取
// ============================================================

/// Binance K线API: GET /api/v3/klines?symbol=BTCUSDT&interval=1d&limit=100
/// 返回: [openTime, open, high, low, close, volume, ...]
fn fetch_binance_klines_blocking(params: &HashMap<String, Value>) -> Result<Vec<Candle>, String> {
    let symbol = parse_str(params, "symbol", "BTCUSDT");
    let interval = parse_str(params, "interval", "1d");
    let limit = parse_usize(params, "limit", 100).min(1000);

    let url = format!(
        "https://api.binance.com/api/v3/klines?symbol={}&interval={}&limit={}",
        symbol, interval, limit
    );

    let resp = reqwest::blocking::get(&url)
        .map_err(|e| format!("请求Binance API失败: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(format!("Binance API返回错误 ({}): {}", status, body));
    }

    let data: Vec<Vec<serde_json::Value>> = resp
        .json()
        .map_err(|e| format!("解析Binance数据失败: {}", e))?;

    let mut candles = Vec::with_capacity(data.len());

    for kline in &data {
        if kline.len() < 6 {
            continue;
        }

        let timestamp_ms = kline[0].as_f64().unwrap_or(0.0) as i64;
        let timestamp = DateTime::from_timestamp_millis(timestamp_ms)
            .unwrap_or_default();

        let open = kline[1].as_str().and_then(|s: &str| s.parse::<f64>().ok()).unwrap_or(0.0);
        let high = kline[2].as_str().and_then(|s: &str| s.parse::<f64>().ok()).unwrap_or(0.0);
        let low = kline[3].as_str().and_then(|s: &str| s.parse::<f64>().ok()).unwrap_or(0.0);
        let close = kline[4].as_str().and_then(|s: &str| s.parse::<f64>().ok()).unwrap_or(0.0);
        let volume = kline[5].as_str().and_then(|s: &str| s.parse::<f64>().ok()).unwrap_or(0.0);

        candles.push(Candle::new(timestamp, open, high, low, close, volume));
    }

    if candles.is_empty() {
        return Err(format!("未获取到任何K线数据 (symbol={}, interval={})", symbol, interval));
    }

    Ok(candles)
}

// ============================================================
// 策略名称映射
// ============================================================

fn get_strategy_by_name(name: &str) -> Option<Box<dyn Strategy>> {
    match name {
        "SMA_Crossover" => Some(Box::new(SmaCrossover::new(5, 20))),
        "SMA_RSI" => Some(Box::new(SmaCrossoverWithRsi::new(5, 20, 14, 30.0, 70.0))),
        "RSI_MeanRev" => Some(Box::new(RsiMeanReversion::new(14, 30.0, 70.0))),
        "MACD_Crossover" => Some(Box::new(MacdStrategy::new(12, 26, 9, MacdMode::Crossover))),
        "MACD_Histogram" => Some(Box::new(MacdStrategy::new(12, 26, 9, MacdMode::CrossoverWithHistogram))),
        "MACD_Divergence" => Some(Box::new(MacdStrategy::new(12, 26, 9, MacdMode::CrossoverWithDivergence))),
        "Turtle_Trading" => Some(Box::new(TurtleTradingStrategy::new(20, 10, 20, 2.0))),
        "Triple_EMA" => Some(Box::new(TripleEmaStrategy::new(5, 13, 34))),
        "Bollinger_MeanRev" => Some(Box::new(BollingerBandsStrategy::new(20, 2.0, 0.95, 0.95, false))),
        "Bollinger_Squeeze" => Some(Box::new(BollingerBandsStrategy::new(20, 2.0, 0.95, 0.95, true))),
        "VWAP_RSI" => Some(Box::new(VwapRsiStrategy::new(14, 30.0, 70.0, 1.0))),
        "SuperTrend" => Some(Box::new(SuperTrendStrategy::new(10, 3.0))),
        "Keltner_Breakout" => Some(Box::new(KeltnerChannelsStrategy::new(20, 14, 2.0, KeltnerMode::Breakout))),
        "Keltner_Reversion" => Some(Box::new(KeltnerChannelsStrategy::new(20, 14, 2.0, KeltnerMode::Reversion))),
        "Parabolic_SAR" => Some(Box::new(ParabolicSarStrategy::new(0.02, 0.2))),
        "Ensemble" => Some(strategy::create_ensemble_strategy()),
        _ => None,
    }
}

fn get_strategy_key(name: &str) -> &str {
    match name {
        "SMA Crossover" => "SMA_Crossover",
        "SMA Crossover + RSI Filter" => "SMA_RSI",
        "RSI Mean Reversion" => "RSI_MeanRev",
        "MACD Crossover" => "MACD_Crossover",
        "MACD + Histogram" => "MACD_Histogram",
        "MACD + Divergence" => "MACD_Divergence",
        "Turtle Trading (Donchian)" => "Turtle_Trading",
        "Triple EMA Trend" => "Triple_EMA",
        "Bollinger Bands MeanRev" => "Bollinger_MeanRev",
        "Bollinger Bands + Squeeze" => "Bollinger_Squeeze",
        "VWAP + RSI Reversion" => "VWAP_RSI",
        "SuperTrend" => "SuperTrend",
        "Keltner Breakout" => "Keltner_Breakout",
        "Keltner Reversion" => "Keltner_Reversion",
        "Parabolic SAR" => "Parabolic_SAR",
        _ => "",
    }
}

// ============================================================
// 操作1: 策略列表
// ============================================================

fn handle_strategies() -> Result<Value, String> {
    let strategies = strategy::create_default_strategies();
    let mut list = Vec::new();
    for s in strategies {
        list.push(serde_json::json!({
            "name": s.name(),
            "key": get_strategy_key(s.name()),
        }));
    }
    list.push(serde_json::json!({
        "name": "Ensemble (3 strategies)",
        "key": "Ensemble",
    }));
    Ok(serde_json::json!({
        "total": list.len(),
        "strategies": list,
        "description": "可用的交易策略列表。使用对应的key在backtest的strategy_names参数中指定。"
    }))
}

// ============================================================
// 操作2: 回测（策略锦标赛）
// ============================================================

fn handle_backtest_sync(params: &HashMap<String, Value>) -> Result<Value, String> {
    let candles = load_candles(params)?;
    let initial_capital = parse_f64(params, "initial_capital", 10_000.0);
    let commission_rate = parse_f64(params, "commission_rate", 0.001);
    let slippage = parse_f64(params, "slippage", 0.001);
    let export_json = parse_bool(params, "export_json", false);

    let strategy_names = params
        .get("strategy_names")
        .and_then(|v| v.as_str())
        .map(|s| s.split(',').map(|n| n.trim().to_string()).collect::<Vec<_>>());

    let mut strats: Vec<Box<dyn Strategy>> = Vec::new();
    if let Some(names) = strategy_names {
        for name in names {
            if !name.is_empty() {
                match get_strategy_by_name(&name) {
                    Some(s) => strats.push(s),
                    None => return Err(format!("未知策略: '{}'。使用 'strategies' 操作查看可用策略列表", name)),
                }
            }
        }
    } else {
        strats = strategy::create_default_strategies();
        strats.push(strategy::create_ensemble_strategy());
    }

    if strats.is_empty() {
        return Err("至少需要指定一个策略".into());
    }

    // 运行全部策略回测
    let mut results: Vec<(String, serde_json::Value)> = Vec::new();
    for strategy in &strats {
        let mut engine = BacktestEngine::new(initial_capital, commission_rate, slippage);
        match engine.run(&candles, strategy.as_ref()) {
            Ok(trades) => {
                let report = BacktestResult::new(&engine, &candles, &trades);
                results.push((
                    strategy.name().to_string(),
                    serde_json::to_value(&report.engine).unwrap_or(serde_json::json!({})),
                ));
            }
            Err(e) => {
                results.push((
                    strategy.name().to_string(),
                    serde_json::json!({ "error": format!("回测失败: {}", e) }),
                ));
            }
        }
    }

    // 按收益率排序
    results.sort_by(|a, b| {
        let ra = a.1.get("total_return_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let rb = b.1.get("total_return_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
        rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut ranking = Vec::new();
    for (i, (name, data)) in results.iter().enumerate() {
        ranking.push(serde_json::json!({
            "rank": i + 1,
            "name": name,
            "data": data,
        }));
    }

    let winner = ranking.first().map(|r| serde_json::json!({
        "name": r["name"],
        "total_return_pct": r["data"]["total_return_pct"],
        "sharpe_ratio": r["data"]["sharpe_ratio"],
    }));

    let output = serde_json::json!({
        "status": "success",
        "data_source": parse_str(params, "data_source", "mock"),
        "bars_count": candles.len(),
        "initial_capital": initial_capital,
        "strategies_tested": strats.len(),
        "ranking": ranking,
        "winner": winner,
    });

    if export_json {
        if let Ok(json_str) = serde_json::to_string_pretty(&output) {
            let path = format!("strategy_ranking_{}.json", chrono::Utc::now().timestamp());
            let _ = std::fs::write(&path, &json_str);
            return Ok(serde_json::json!({
                "status": "success",
                "export_path": path,
                "result": output,
            }));
        }
    }

    Ok(output)
}

// ============================================================
// 操作3: 技术指标计算
// ============================================================

fn handle_indicators_sync(params: &HashMap<String, Value>) -> Result<Value, String> {
    let candles = load_candles(params)?;
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
    let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();

    let sma_5 = crate::trading::indicators::sma(&closes, 5);
    let sma_20 = crate::trading::indicators::sma(&closes, 20);
    let sma_60 = crate::trading::indicators::sma(&closes, 60);
    let ema_12 = crate::trading::indicators::ema(&closes, 12);
    let ema_26 = crate::trading::indicators::ema(&closes, 26);
    let rsi_14 = crate::trading::indicators::rsi(&closes, 14);
    let macd = crate::trading::indicators::macd(&closes, 12, 26, 9);
    let bb = crate::trading::indicators::bollinger_bands(&closes, 20, 2.0);
    let atr_val = crate::trading::indicators::atr(&highs, &lows, &closes, 14);

    let last_idx = if !closes.is_empty() { closes.len() - 1 } else { 0 };
    let get_last = |v: &[f64]| {
        if last_idx < v.len() && !v[last_idx].is_nan() {
            Some(v[last_idx])
        } else {
            None
        }
    };

    Ok(serde_json::json!({
        "status": "success",
        "data_source": parse_str(params, "data_source", "mock"),
        "bars_count": candles.len(),
        "price": {
            "open": candles[last_idx].open,
            "high": candles[last_idx].high,
            "low": candles[last_idx].low,
            "close": candles[last_idx].close,
            "volume": candles[last_idx].volume,
            "high_max": highs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            "low_min": lows.iter().cloned().fold(f64::INFINITY, f64::min),
        },
        "moving_averages": {
            "sma_5": get_last(&sma_5),
            "sma_20": get_last(&sma_20),
            "sma_60": get_last(&sma_60),
            "ema_12": get_last(&ema_12),
            "ema_26": get_last(&ema_26),
        },
        "rsi_14": get_last(&rsi_14),
        "macd": {
            "macd_line": get_last(&macd.macd_line),
            "signal_line": get_last(&macd.signal_line),
            "histogram": get_last(&macd.histogram),
        },
        "bollinger_bands": {
            "upper": get_last(&bb.upper),
            "middle": get_last(&bb.middle),
            "lower": get_last(&bb.lower),
        },
        "atr_14": get_last(&atr_val),
    }))
}

// ============================================================
// 操作4: 从API获取真实数据
// ============================================================

fn handle_fetch_data(params: &HashMap<String, Value>) -> Result<Value, String> {
    let symbol = parse_str(params, "symbol", "BTCUSDT");
    let interval = parse_str(params, "interval", "1d");
    let _limit = parse_usize(params, "limit", 100).min(1000);
    let export_csv = parse_bool(params, "export_csv", false);

    let candles = fetch_binance_klines_blocking(params)?;
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let first = &candles[0];
    let last = &candles[candles.len() - 1];

    // 计算基础指标
    let sma_20 = crate::trading::indicators::sma(&closes, 20.min(closes.len()));
    let sma_50 = crate::trading::indicators::sma(&closes, 50.min(closes.len()));
    let rsi_14 = crate::trading::indicators::rsi(&closes, 14.min(closes.len()));

    let last_sma20 = closes.len().min(20);
    let last_sma50 = closes.len().min(50);
    let last_rsi = closes.len().min(14);

    let mut result = serde_json::json!({
        "status": "success",
        "source": "Binance API",
        "symbol": symbol,
        "interval": interval,
        "count": candles.len(),
        "date_range": {
            "from": first.timestamp.to_rfc3339(),
            "to": last.timestamp.to_rfc3339(),
        },
        "price_range": {
            "first_close": first.close,
            "last_close": last.close,
            "change_pct": if first.close > 0.0 {
                ((last.close - first.close) / first.close * 100.0 * 100.0).round() / 100.0
            } else { 0.0 },
            "high": candles.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max),
            "low": candles.iter().map(|c| c.low).fold(f64::INFINITY, f64::min),
        },
        "latest_candle": {
            "timestamp": last.timestamp.to_rfc3339(),
            "open": last.open,
            "high": last.high,
            "low": last.low,
            "close": last.close,
            "volume": last.volume,
        },
        "indicators": {
            "sma_20": get_last_value(&sma_20, last_sma20),
            "sma_50": get_last_value(&sma_50, last_sma50),
            "rsi_14": get_last_value(&rsi_14, last_rsi),
        },
        "usage": "将此数据用于回测: 设置 data_source='binance' + symbol + interval + limit"
    });

    // 导出CSV
    if export_csv {
        let filename = format!("{}_{}_{}.csv", symbol, interval, chrono::Utc::now().format("%Y%m%d_%H%M%S"));
        if let Ok(mut wtr) = csv::Writer::from_path(&filename) {
            let _ = wtr.write_record(["timestamp", "open", "high", "low", "close", "volume"]);
            for c in &candles {
                let _ = wtr.write_record(&[
                    c.timestamp.to_rfc3339(),
                    c.open.to_string(),
                    c.high.to_string(),
                    c.low.to_string(),
                    c.close.to_string(),
                    c.volume.to_string(),
                ]);
            }
            if let Ok(()) = wtr.flush() {
                result["export_csv"] = serde_json::json!(filename);
            }
        }
    }

    Ok(result)
}

fn get_last_value(values: &[f64], lookback: usize) -> Option<f64> {
    let idx = if !values.is_empty() { values.len() - 1 } else { return None; };
    if idx < values.len() && !values[idx].is_nan() {
        Some(values[idx])
    } else if lookback > 0 && lookback < values.len() && !values[lookback - 1].is_nan() {
        Some(values[lookback - 1])
    } else {
        None
    }
}

// ============================================================
// 操作5: 生成模拟数据
// ============================================================

fn handle_mock_data(params: &HashMap<String, Value>) -> Result<Value, String> {
    let count = parse_usize(params, "mock_count", 100);
    let start_price = parse_f64(params, "mock_start_price", 100.0);
    let candles = DataSource::generate_mock(count, start_price);
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

    Ok(serde_json::json!({
        "status": "success",
        "count": count,
        "start_price": start_price,
        "end_price": closes.last().unwrap_or(&0.0),
        "first_candle": {
            "timestamp": candles[0].timestamp.to_rfc3339(),
            "open": candles[0].open,
            "high": candles[0].high,
            "low": candles[0].low,
            "close": candles[0].close,
            "volume": candles[0].volume,
        },
        "last_candle": {
            "timestamp": candles[count - 1].timestamp.to_rfc3339(),
            "open": candles[count - 1].open,
            "high": candles[count - 1].high,
            "low": candles[count - 1].low,
            "close": candles[count - 1].close,
            "volume": candles[count - 1].volume,
        },
    }))
}

pub fn register_all(registry: &mut crate::tools::ToolRegistry) {
    registry.register(Box::new(QuantitativeTradingTool));
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolRegistry;

    #[test]
    fn test_tool_metadata() {
        let tool = QuantitativeTradingTool;
        assert_eq!(tool.name(), "quantitative_trading");
        assert!(tool.description().contains("回测"));
        assert!(!tool.parameters().is_empty());
    }

    #[test]
    fn test_strategies_action() {
        let result = handle_strategies().unwrap();
        let total = result["total"].as_u64().unwrap();
        assert!(total >= 12);
    }

    #[test]
    fn test_mock_data_action() {
        let mut params = HashMap::new();
        params.insert("action".into(), Value::String("mock_data".into()));
        params.insert("mock_count".into(), Value::Number(50.into()));
        params.insert("mock_start_price".into(), serde_json::json!(100.0));
        let result = handle_mock_data(&params).unwrap();
        assert_eq!(result["count"].as_u64().unwrap(), 50);
    }

    #[test]
    fn test_backtest_with_mock() {
        let mut params = HashMap::new();
        params.insert("action".into(), Value::String("backtest".into()));
        params.insert("mock_count".into(), Value::Number(200.into()));
        params.insert("strategy_names".into(), Value::String("SMA_Crossover,MACD_Crossover".into()));
        let result = handle_backtest_sync(&params).unwrap();
        assert_eq!(result["strategies_tested"].as_u64().unwrap(), 2);
    }

    #[test]
    fn test_register() {
        let mut registry = ToolRegistry::new();
        register_all(&mut registry);
        let tool = registry.get("quantitative_trading");
        assert!(tool.is_some());
    }

    #[test]
    fn test_invalid_strategy_error() {
        let mut params = HashMap::new();
        params.insert("action".into(), Value::String("backtest".into()));
        params.insert("strategy_names".into(), Value::String("Invalid_Strat".into()));
        let result = handle_backtest_sync(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_fetch_data_description() {
        // Test that fetch_data is mentioned in the action list
        let tool = QuantitativeTradingTool;
        assert!(tool.description().contains("fetch_data"));
    }

    #[test]
    fn test_indicators_with_mock() {
        let mut params = HashMap::new();
        params.insert("action".into(), Value::String("indicators".into()));
        params.insert("data_source".into(), Value::String("mock".into()));
        params.insert("mock_count".into(), Value::Number(100.into()));
        let result = handle_indicators_sync(&params).unwrap();
        assert!(result.get("rsi_14").is_some());
        assert!(result.get("macd").is_some());
    }

    #[test]
    fn test_new_action_names_in_execute() {
        let tool = QuantitativeTradingTool;
        let mut params = HashMap::new();
        params.insert("action".into(), Value::String("invalid".into()));
        let err = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(tool.execute(&params))
            .unwrap_err();
        assert!(err.contains("fetch_data"));
    }
}
