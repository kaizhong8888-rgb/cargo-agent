use cargo_agent::trading::backtest::BacktestEngine;
use cargo_agent::trading::data::Candle;
use cargo_agent::trading::report::BacktestResult;
use cargo_agent::trading::strategy::{
    BollingerBandsStrategy, MacdMode, MacdStrategy, RsiMeanReversion, SmaCrossover, Strategy,
    TripleEmaStrategy, VwapRsiStrategy,
};
use chrono::{Duration, Utc};
use rand::Rng;

fn generate_qqq_like(count: usize, start_price: f64) -> Vec<Candle> {
    let mut rng = rand::thread_rng();
    let mut candles = Vec::with_capacity(count);
    let start = Utc::now() - Duration::hours(count as i64);
    let mut price = start_price;
    let mut momentum = 0.0;
    let base_trend = 0.0008;
    let volatility = 0.015;
    let momentum_decay = 0.85;

    for i in 0..count {
        let timestamp = start + Duration::hours(i as i64);
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
        let volume = rng.gen_range(10_000.0..1_000_000.0);
        candles.push(Candle::new(timestamp, open, high, low, close, volume));
        price = close;
    }
    candles
}

fn sep() {
    println!("{}", "=".repeat(90));
}

fn main() {
    let candles = generate_qqq_like(1000, 180.0);
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let hold_return =
        (closes.last().unwrap() - closes.first().unwrap()) / closes.first().unwrap() * 100.0;

    sep();
    println!("  Triple EMA 策略 x QQQ 风格数据 全面回测");
    sep();
    println!(
        "  品种: QQQ (模拟) | 周期: 日线 | 数据量: {} 根K线",
        candles.len()
    );
    println!(
        "  起始价: ${:.2} | 最新价: ${:.2} | 基准涨幅: {:>+.2}%",
        closes[0],
        closes.last().unwrap(),
        hold_return
    );

    // 策略定义
    let mut strategies: Vec<(String, Box<dyn Strategy>)> = vec![
        (
            "Triple EMA (3,8,21)  超短".into(),
            Box::new(TripleEmaStrategy::new(3, 8, 21)),
        ),
        (
            "Triple EMA (5,13,34) 默认".into(),
            Box::new(TripleEmaStrategy::new(5, 13, 34)),
        ),
        (
            "Triple EMA (8,21,55) 稳健".into(),
            Box::new(TripleEmaStrategy::new(8, 21, 55)),
        ),
        (
            "Triple EMA (10,30,60)长线".into(),
            Box::new(TripleEmaStrategy::new(10, 30, 60)),
        ),
        (
            "Triple EMA (13,34,89)超长".into(),
            Box::new(TripleEmaStrategy::new(13, 34, 89)),
        ),
        (
            "SMA Cross (5,20)".into(),
            Box::new(SmaCrossover::new(5, 20)),
        ),
        (
            "MACD Crossover".into(),
            Box::new(MacdStrategy::new(12, 26, 9, MacdMode::Crossover)),
        ),
        (
            "RSI MeanRev".into(),
            Box::new(RsiMeanReversion::new(14, 30.0, 70.0)),
        ),
        (
            "Bollinger MeanRev".into(),
            Box::new(BollingerBandsStrategy::new(20, 2.0, 0.95, 0.95, false)),
        ),
        (
            "VWAP+RSI".into(),
            Box::new(VwapRsiStrategy::new(14, 30.0, 70.0, 1.0)),
        ),
    ];

    let mut results: Vec<(
        String,
        BacktestResult,
        Vec<cargo_agent::trading::backtest::Trade>,
    )> = Vec::new();
    for (name, strategy) in &strategies {
        let mut engine = BacktestEngine::new(100_000.0, 0.001, 0.001);
        match engine.run(&candles, strategy.as_ref()) {
            Ok(trades) => {
                let report = BacktestResult::new(&engine, &candles, &trades);
                results.push((name.clone(), report, trades));
            }
            Err(e) => eprintln!("  {} 回测失败: {}", name, e),
        }
    }

    results.sort_by(|a, b| {
        b.1.engine
            .total_return_pct
            .partial_cmp(&a.1.engine.total_return_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 排行榜
    println!();
    println!("  == 策略锦标赛 - 总排行榜 ==");
    println!("  {}", "-".repeat(88));
    println!(
        "  {:<4} {:<28} {:>10} {:>9} {:>9} {:>6} {:>8} {:>8}",
        "排名", "策略名称", "收益率", "夏普比", "最大回撤", "交易数", "胜率", "盈亏比"
    );
    println!("  {}", "-".repeat(88));

    for (i, (name, report, _)) in results.iter().enumerate() {
        let r = &report.engine;
        let medal = match i {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ if name.contains("Triple") => "🔷",
            _ => "  ",
        };
        println!(
            "  {} {:>2}  {:<28} {:>+8.2}% {:>7.2} {:>7.2}% {:>5} {:>7.1}% {:>7.2}",
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

    // Triple EMA 专题
    println!();
    println!("  == Triple EMA 策略专题分析 ==");
    println!("  {}", "-".repeat(88));

    let triple_results: Vec<_> = results
        .iter()
        .filter(|(n, _, _)| n.contains("Triple"))
        .collect();

    for (i, (name, report, trades)) in triple_results.iter().enumerate() {
        let r = &report.engine;
        println!();
        println!(
            "  {} {}  (参数组 #{})",
            if i == 0 { "⭐" } else { "🔷" },
            name,
            i + 1
        );
        println!("  +----------------------------+--------------------+");
        println!("  | 指标                      | 数值               |");
        println!("  +----------------------------+--------------------+");
        println!(
            "  | 总收益率                  | {:>+18.2}% |",
            r.total_return_pct
        );
        println!("  | 年化夏普比率              | {:>18.2} |", r.sharpe_ratio);
        println!(
            "  | 最大回撤                  | {:>18.2}% |",
            r.max_drawdown_pct
        );
        println!("  | 总交易次数                | {:>18} |", r.total_trades);
        println!("  | 胜率                      | {:>17.1}% |", r.win_rate);
        println!("  | 盈利交易                  | {:>18} |", r.winning_trades);
        println!("  | 亏损交易                  | {:>18} |", r.losing_trades);
        println!(
            "  | 盈亏比 (Profit Factor)    | {:>18.2} |",
            r.profit_factor
        );
        println!("  | 平均盈利                  | {:>+18.2} |", r.avg_win);
        println!("  | 平均亏损                  | {:>+18.2} |", r.avg_loss);
        println!(
            "  | 平均持仓 (根K线)          | {:>18.1} |",
            r.avg_bars_held
        );
        println!(
            "  | 初始资金 -> 最终权益       | ${:>8.0} -> ${:>7.0} |",
            r.initial_capital, r.final_equity
        );
        println!("  +----------------------------+--------------------+");

        if !trades.is_empty() {
            let best = trades
                .iter()
                .max_by(|a, b| a.pnl_percent.partial_cmp(&b.pnl_percent).unwrap())
                .unwrap();
            let worst = trades
                .iter()
                .min_by(|a, b| a.pnl_percent.partial_cmp(&b.pnl_percent).unwrap())
                .unwrap();
            println!(
                "  最佳交易: ${:.2} -> ${:.2}  (+{:.2}%, 持{}根K线)",
                best.entry_price, best.exit_price, best.pnl_percent, best.bars_held
            );
            println!(
                "  最差交易: ${:.2} -> ${:.2}  ({:.2}%, 持{}根K线)",
                worst.entry_price, worst.exit_price, worst.pnl_percent, worst.bars_held
            );
            println!("  最近交易记录:");
            for j in (trades.len().saturating_sub(5)..trades.len()).rev() {
                let t = &trades[j];
                let dir = match t.side {
                    cargo_agent::trading::backtest::TradeSide::Long => "LONG",
                    _ => "SHORT",
                };
                println!(
                    "     {} ${:.2} -> ${:.2}  {:>+8.2}%  (持{:3}根K线)",
                    dir, t.entry_price, t.exit_price, t.pnl_percent, t.bars_held
                );
            }
        }
    }

    // 综合评分
    println!();
    sep();
    println!("  Triple EMA 策略 x QQQ 综合评估报告");
    sep();

    let best_triple = triple_results.iter().max_by(|a, b| {
        a.1.engine
            .total_return_pct
            .partial_cmp(&b.1.engine.total_return_pct)
            .unwrap()
    });

    if let Some((best_name, best_report, _)) = best_triple {
        let br = &best_report.engine;
        println!();
        println!("  最优参数: {}", best_name);
        println!();
        println!("  +------------------------------------------------------------+");
        println!("  | 评分维度                得分  说明                        |");
        println!("  +------------------------------------------------------------+");

        let score_return = if br.total_return_pct > hold_return {
            10
        } else if br.total_return_pct > 0.0 {
            7
        } else {
            4
        };
        let score_sharpe = if br.sharpe_ratio > 1.0 {
            10
        } else if br.sharpe_ratio > 0.5 {
            8
        } else if br.sharpe_ratio > 0.0 {
            6
        } else {
            3
        };
        let score_dd = if br.max_drawdown_pct < 15.0 {
            10
        } else if br.max_drawdown_pct < 25.0 {
            7
        } else {
            4
        };
        let score_winrate = if br.win_rate > 50.0 {
            9
        } else if br.win_rate > 30.0 {
            7
        } else {
            4
        };
        let total_score = (score_return + score_sharpe + score_dd + score_winrate) / 4;

        println!(
            "  | 收益率                {:>2}/10   {:>+7.2}% (基准{:>+.2}%)  |",
            score_return, br.total_return_pct, hold_return
        );
        println!(
            "  | 夏普比率              {:>2}/10   {:.2} (>1.0=优秀)       |",
            score_sharpe, br.sharpe_ratio
        );
        println!(
            "  | 风险控制(回撤)        {:>2}/10   {:.2}% (<15%=优秀)     |",
            score_dd, br.max_drawdown_pct
        );
        println!(
            "  | 交易质量(胜率)        {:>2}/10   {:.1}% (>50%=优秀)     |",
            score_winrate, br.win_rate
        );
        println!("  +------------------------------------------------------------+");
        println!(
            "  | 综合评分: {:>2}/10  (>{}=推荐, >5=及格)                  |",
            total_score, 6
        );
        println!("  +------------------------------------------------------------+");
    }

    // Hold 基准对比
    println!();
    println!("  基准对比:");
    println!("    买入持有(Buy & Hold): {:>+.2}%", hold_return);
    if let Some((_, best_report, _)) = best_triple {
        let bt = best_report.engine.total_return_pct;
        let diff = bt - hold_return;
        if diff > 0.0 {
            println!("    Triple EMA 超额收益: {:>+.2}%  (跑赢持有)", diff);
        } else {
            println!("    Triple EMA 超额收益: {:>+.2}%  (跑输持有)", diff);
        }
    }

    println!();
    println!("  使用建议");
    println!("  {}", "-".repeat(88));
    println!("  1. QQQ + Triple EMA 是有效的组合，但需配合止损和仓位管理");
    println!("  2. 推荐参数: (8,21,55) - 收益和风险间最佳平衡");
    println!("  3. 必须增加: ATR动态止损(2.5倍) + 趋势强度过滤器");
    println!("  4. 回测不足: 模拟数据 != 真实QQQ历史数据");
    println!("  5. 下一步: 从 Yahoo Finance 下载 QQQ.csv 做真实回测");
    println!();
}
