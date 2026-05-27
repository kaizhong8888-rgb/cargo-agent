mod backtest;
mod data;
mod indicators;
mod report;
mod strategy;

use anyhow::Result;
use data::DataSource;
use strategy::*;

fn main() -> Result<()> {
    println!("🚀 量化交易策略比较回测系统");
    println!("{}\n", "=".repeat(70));

    // ============================================================
    // 1. 加载数据
    // ============================================================
    println!("📊 [1/4] 加载数据...");

    let candles = match DataSource::from_csv("data/btc_usdt_1h.csv") {
        Ok(c) => {
            println!("   ✅ 从 CSV 加载了 {} 根 K 线\n", c.len());
            c
        }
        Err(_) => {
            println!("   ⚠️  CSV 文件不存在，生成模拟价格数据");
            let mock = DataSource::generate_mock(1000, 100.0);
            println!("   ✅ 生成了 {} 根模拟 K 线\n", mock.len());
            mock
        }
    };

    let initial_capital = 10_000.0;
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

    // ============================================================
    // 2. 计算技术指标概览
    // ============================================================
    println!("📈 [2/4] 技术指标概览...");
    let sma_20 = indicators::sma(&closes, 20);
    let rsi = indicators::rsi(&closes, 14);
    let macd = indicators::macd(&closes, 12, 26, 9);

    println!(
        "   📊 收盘价范围: ${:.2} ~ ${:.2}",
        closes.iter().cloned().fold(f64::INFINITY, f64::min),
        closes.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    );
    println!(
        "   📊 SMA(20)={:.2}, RSI(14)={:.2}",
        sma_20.last().copied().unwrap_or(0.0),
        rsi.last().copied().unwrap_or(0.0)
    );
    println!(
        "   📊 MACD={:.4}, Signal={:.4}, Histogram={:.4}\n",
        macd.macd_line.last().copied().unwrap_or(0.0),
        macd.signal_line.last().copied().unwrap_or(0.0),
        macd.histogram.last().copied().unwrap_or(0.0)
    );

    // ============================================================
    // 3. 🏆 策略锦标赛 — 运行所有策略回测
    // ============================================================
    println!("🔄 [3/4] 🏆 策略锦标赛 — 回测所有策略...\n");

    // 构建策略列表
    let mut strats: Vec<Box<dyn Strategy>> = vec![
        // --- 基础策略 ---
        Box::new(SmaCrossover::new(5, 20)),
        Box::new(SmaCrossoverWithRsi::new(5, 20, 14, 30.0, 70.0)),
        Box::new(RsiMeanReversion::new(14, 30.0, 70.0)),
        // --- MACD 家族 ---
        Box::new(MacdStrategy::new(12, 26, 9, MacdMode::Crossover)),
        Box::new(MacdStrategy::new(12, 26, 9, MacdMode::CrossoverWithHistogram)),
        Box::new(MacdStrategy::new(12, 26, 9, MacdMode::CrossoverWithDivergence)),
        // --- 趋势跟踪 ---
        Box::new(TurtleTradingStrategy::new(20, 10, 20, 2.0)),
        Box::new(TripleEmaStrategy::new(5, 13, 34)),
        // --- 均值回归 ---
        Box::new(BollingerBandsStrategy::new(20, 2.0, 0.95, 0.95, false)),
        Box::new(BollingerBandsStrategy::new(20, 2.0, 0.95, 0.95, true)),
        Box::new(VwapRsiStrategy::new(14, 30.0, 70.0, 1.0)),
    ];

    // --- 组合策略 ---
    let ensemble = EnsembleStrategy::new(vec![
        Box::new(MacdStrategy::new(12, 26, 9, MacdMode::Crossover)),
        Box::new(TripleEmaStrategy::new(5, 13, 34)),
        Box::new(VwapRsiStrategy::new(14, 30.0, 70.0, 1.0)),
    ]);
    strats.push(Box::new(ensemble));

    // 收集所有结果
    let mut results: Vec<(String, report::BacktestResultData)> = Vec::new();

    for strategy in strats {
        println!("  📌 正在回测: {} ...", strategy.name());
        let mut engine = backtest::BacktestEngine::new(initial_capital, 0.001, 0.001);
        match engine.run(&candles, strategy.as_ref()) {
            Ok(trades) => {
                let result = report::BacktestResult::new(&engine, &candles, &trades);
                result.print_summary();
                results.push((strategy.name().to_string(), result.engine));
            }
            Err(e) => {
                println!("     ❌ 回测失败: {}\n", e);
            }
        }
    }

    // ============================================================
    // 4. 🏆 冠军排行榜
    // ============================================================
    println!("\n{}\n", "=".repeat(70));
    println!("🏆 策略排行榜 (按收益率排序)");
    println!("{}\n", "=".repeat(70));

    results.sort_by(|a, b| {
        b.1.total_return_pct
            .partial_cmp(&a.1.total_return_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!(
        "  {:<3} {:<34} {:>10} {:>9} {:>10} {:>7} {:>7}",
        "#", "策略名称", "收益率", "夏普比率", "最大回撤", "交易", "胜率"
    );
    println!("  {}", "-".repeat(86));

    for (i, (name, data)) in results.iter().enumerate() {
        let medal = if i == 0 {
            "🥇"
        } else if i == 1 {
            "🥈"
        } else if i == 2 {
            "🥉"
        } else {
            "  "
        };
        let emoji = if data.total_return_pct > 15.0 {
            " 🔥"
        } else if data.total_return_pct > 0.0 {
            " ✅"
        } else {
            " ❌"
        };
        println!(
            "  {:<3} {:<34} {:>+8.2}% {:>8.4} {:>8.2}% {:>6} {:>6.1}%{}",
            medal,
            name,
            data.total_return_pct,
            data.sharpe_ratio,
            data.max_drawdown_pct,
            data.total_trades,
            data.win_rate,
            emoji,
        );
    }

    println!("\n{}", "=".repeat(70));
    println!("✅ 回测完成！共比较了 {} 个策略", results.len());
    if let Some((winner, data)) = results.first() {
        println!(
            "🥇 冠军策略: {} (收益率 {:.2}%, 夏普比率 {:.4})",
            winner, data.total_return_pct, data.sharpe_ratio
        );

        // 给出策略建议
        println!("\n💡 策略分析建议:");
        for (name, data) in results.iter() {
            if data.total_return_pct > 0.0 && data.sharpe_ratio > 0.5 && data.max_drawdown_pct < 20.0
            {
                println!("   ✅ {} — 收益率{:+.2}% 夏普{:.2} 回撤{:.1}% ⭐ 推荐",
                    name, data.total_return_pct, data.sharpe_ratio, data.max_drawdown_pct);
            } else if data.total_return_pct > 0.0 {
                println!("   📊 {} — 收益率{:+.2}% 但夏普{:.2} 回撤{:.1}% 需优化",
                    name, data.total_return_pct, data.sharpe_ratio, data.max_drawdown_pct);
            }
        }
    }

    // 导出排行榜
    if let Ok(json) = serde_json::to_string_pretty(&results) {
        std::fs::write("strategy_ranking.json", json)?;
        println!("\n📁 排行榜已导出到 strategy_ranking.json");
    }

    Ok(())
}
