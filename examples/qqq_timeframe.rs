/// Triple EMA 多时间框架回测：日线 vs 周线 vs 月线
use cargo_agent::trading::backtest::BacktestEngine;
use cargo_agent::trading::data::Candle;
use cargo_agent::trading::report::BacktestResult;
use cargo_agent::trading::strategy::{Strategy, TripleEmaStrategy};
use chrono::{Duration, Utc};
use rand::Rng;

/// 生成 QQQ 风格的日线数据
fn generate_daily(count: usize, start_price: f64) -> Vec<Candle> {
    let mut rng = rand::thread_rng();
    let mut candles = Vec::with_capacity(count);
    let start = Utc::now() - Duration::hours(count as i64);
    let mut price = start_price;
    let mut momentum = 0.0;
    let base_trend = 0.0008;
    let volatility = 0.015;
    let momentum_decay = 0.85;
    for i in 0..count {
        let ts = start + Duration::hours(i as i64);
        let shock = rng.gen_range(-volatility..volatility);
        momentum = momentum * momentum_decay + shock * (1.0 - momentum_decay);
        let mean_revert = if price > start_price * 2.0 {
            -0.003
        } else if price < start_price * 0.8 {
            0.005
        } else {
            0.0
        };
        let change = base_trend + momentum * 0.6 + mean_revert;
        let open = price;
        let close = (price * (1.0 + change)).max(0.1);
        let high = open.max(close) * (1.0 + rng.gen_range(0.0..0.01));
        let low = open.min(close) * (1.0 - rng.gen_range(0.0..0.01));
        let vol = rng.gen_range(10_000.0..1_000_000.0);
        candles.push(Candle::new(ts, open, high, low, close, vol));
        price = close;
    }
    candles
}

/// 从日线数据聚合生成周线和月线
fn aggregate_candles(daily: &[Candle], bars_per_period: usize) -> Vec<Candle> {
    let mut result = Vec::new();
    for chunk in daily.chunks(bars_per_period) {
        if chunk.is_empty() {
            continue;
        }
        let ts = chunk[0].timestamp;
        let open = chunk[0].open;
        let close = chunk.last().unwrap().close;
        let high = chunk.iter().map(|c| c.high).fold(0.0_f64, f64::max);
        let low = chunk.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
        let volume = chunk.iter().map(|c| c.volume).sum();
        result.push(Candle::new(ts, open, high, low, close, volume));
    }
    result
}

fn sep() {
    println!("{}", "=".repeat(92));
}

fn run_backtest(name: &str, candles: &[Candle], params_list: &[(usize, usize, usize, &str)]) {
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    if closes.len() < 100 {
        println!("  {}: 数据太少 ({}根) 无法回测", name, closes.len());
        return;
    }

    let hold_return =
        (closes.last().unwrap() - closes.first().unwrap()) / closes.first().unwrap() * 100.0;
    println!("  {} 数据: {} 根K线", name, candles.len());
    println!(
        "  起始: ${:.2} -> 最新: ${:.2}  (基准涨幅: {:>+.2}%)",
        closes[0],
        closes.last().unwrap(),
        hold_return
    );
    println!();

    let mut results = Vec::new();
    for (fast, mid, slow, desc) in params_list {
        let strategy = TripleEmaStrategy::new(*fast, *mid, *slow);
        let mut engine = BacktestEngine::new(100_000.0, 0.001, 0.001);
        if let Ok(trades) = engine.run(candles, &strategy) {
            let report = BacktestResult::new(&engine, candles, &trades);
            results.push((
                format!("Triple EMA ({},{},{}) {}", fast, mid, slow, desc),
                report,
                trades,
            ));
        }
    }

    results.sort_by(|a, b| {
        b.1.engine
            .total_return_pct
            .partial_cmp(&a.1.engine.total_return_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!(
        "  {:<4} {:<34} {:>10} {:>9} {:>9} {:>6} {:>8} {:>8}",
        "排名", "参数", "收益率", "夏普比", "最大回撤", "交易数", "胜率", "盈亏比"
    );
    println!("  {}", "-".repeat(84));

    for (i, (name, report, _)) in results.iter().enumerate() {
        let r = &report.engine;
        let medal = match i {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "  ",
        };
        println!(
            "  {} {:>2}  {:<34} {:>+8.2}% {:>7.2} {:>7.2}% {:>5} {:>7.1}% {:>7.2}",
            medal,
            i + 1,
            name,
            r.total_return_pct,
            r.sharpe_ratio,
            r.max_drawdown_pct,
            r.total_trades,
            r.win_rate,
            r.profit_factor
        );
    }
    println!(
        "  {} 基准: Buy & Hold {:>+.2}%",
        if results.is_empty() { "" } else { "" },
        hold_return
    );
    println!();
}

fn main() {
    // 生成连续日线数据（约4年 = 1000个交易日）
    let daily_candles = generate_daily(1000, 180.0);

    // 聚合为周线（5天/周）和月线（21天/月）
    let weekly_candles = aggregate_candles(&daily_candles, 5);
    let monthly_candles = aggregate_candles(&daily_candles, 21);

    sep();
    println!("  Triple EMA 多时间框架适用性测试");
    println!("  数据源: 同一组日线数据聚合生成周线/月线");
    println!("  总日线数: 1000根 (~4年)");
    println!("  聚合周线: {} 根 (~4年)", weekly_candles.len());
    println!("  聚合月线: {} 根 (~4年)", monthly_candles.len());
    sep();

    // ===== 日线回测 =====
    println!();
    println!("  {}", "=".repeat(42));
    println!("  1️⃣  日线 (Daily) 回测");
    println!("  {}", "=".repeat(42));
    println!();
    let daily_params = vec![
        (3, 8, 21, "超短"),
        (5, 13, 34, "⭐默认"),
        (8, 21, 55, "稳健"),
        (10, 30, 60, "长线"),
        (13, 34, 89, "超长"),
        (20, 50, 200, "极长"),
    ];
    run_backtest("日线", &daily_candles, &daily_params);

    // ===== 周线回测 =====
    println!();
    println!("  {}", "=".repeat(42));
    println!("  2️⃣  周线 (Weekly) 回测");
    println!("  {}", "=".repeat(42));
    println!();
    let weekly_params = vec![
        (2, 5, 13, "短线周"),
        (3, 8, 21, "⭐默认"),
        (5, 13, 34, "稳健周"),
        (8, 21, 55, "长线周"),
        (10, 30, 60, "超长周"),
    ];
    run_backtest("周线", &weekly_candles, &weekly_params);

    // ===== 月线回测 =====
    println!();
    println!("  {}", "=".repeat(42));
    println!("  3️⃣  月线 (Monthly) 回测");
    println!("  {}", "=".repeat(42));
    println!();
    let monthly_params = vec![
        (2, 3, 8, "短线月"),
        (3, 5, 13, "⭐默认"),
        (5, 13, 34, "长线月"),
        (8, 21, 55, "超长月"),
    ];
    run_backtest("月线", &monthly_candles, &monthly_params);

    // ===== 总结 =====
    sep();
    println!("  多时间框架参数对照表");
    sep();
    println!();
    println!("  +------------+-------------------+----------------------+-------------------+");
    println!("  | 时间框架   | 超短 (灵敏)       | 默认 (推荐)          | 稳健 (长线)       |");
    println!("  +------------+-------------------+----------------------+-------------------+");
    println!("  | 日线 1D    | (3, 8, 21)        | (5, 13, 34) ⭐       | (8, 21, 55)       |");
    println!("  | 周线 1W    | (2, 5, 13)        | (3, 8, 21)  ⭐       | (5, 13, 34)       |");
    println!("  | 月线 1M    | (2, 3, 8)         | (3, 5, 13)   ⭐      | (5, 13, 34)       |");
    println!("  +------------+-------------------+----------------------+-------------------+");
    println!();

    // 对应关系解释
    println!("  参数的实际时间跨度（以交易日为基准）:");
    println!();
    println!("  ┌─────────────────────────────────────────────────────────────┐");
    println!("  │ 参数         │ 日线          │ 周线           │ 月线       │");
    println!("  ├─────────────────────────────────────────────────────────────┤");
    println!("  │ (3,8,21)     │ ~3天/~1.5周   │ ~3周/~1.5月    │ ~3月/季   │");
    println!("  │              │ /~1月         │ /~5月          │ /~2年     │");
    println!("  ├─────────────────────────────────────────────────────────────┤");
    println!("  │ (5,13,34)    │ ~1周/~2.5周   │ ~5周/~3月      │ ~5月/~13月│");
    println!("  │              │ /~6.5周       │ /~8月          │ /~3年     │");
    println!("  ├─────────────────────────────────────────────────────────────┤");
    println!("  │ (8,21,55)    │ ~1.5周/~1月   │ ~2月/~5月      │ ~8月/~21月│");
    println!("  │              │ /~3月         │ /~14月         │ /~5年     │");
    println!("  └─────────────────────────────────────────────────────────────┘");
    println!();

    // 核心结论
    println!("  核心结论");
    println!("  {}", "-".repeat(88));
    println!("  1. 不要把日线的 (5,13,34) 直接套用到周线/月线！");
    println!("     日线 (5,13,34) = 快线追踪 ~1周趋势");
    println!("     周线 (5,13,34) = 快线追踪 ~5周趋势 (完全不同!)");
    println!("     月线 (5,13,34) = 快线追踪 ~5月趋势 (巨慢!)");
    println!();
    println!("  2. 参数缩放法则：时间框架越大，周期数应该越小");
    println!("     日线默认 (5,13,34)");
    println!("     周线默认 (3,8,21)  ← 周期数缩小约 40%");
    println!("     月线默认 (3,5,13)  ← 周期数缩小约 60%");
    println!();
    println!("  3. 保持斐波那契比例比绝对值更重要");
    println!("     核心比例: 快:中:慢 ≈ 1:2.6:6.8");
    println!("     (5,13,34)   → 1:2.6:6.8 ✅");
    println!("     (3,8,21)    → 1:2.7:7.0 ✅");
    println!("     (3,5,13)    → 1:1.7:4.3 ✅ (月线可接受)");
    println!();
    println!("  4. 推荐的多时间框架组合");
    println!("     日线 (5,13,34) → 短线交易/入场时机");
    println!("     周线 (3,8,21)  → 中期趋势/持仓判断");
    println!("     月线 (3,5,13)  → 大趋势方向/资产配置");
    println!();
    println!("  5. 一句话总结");
    println!("     ⭐ 日线用 (5,13,34)，周线用 (3,8,21)，月线用 (3,5,13)");
    println!("     不要跨时间框架复制粘贴参数！");
    println!();
}
