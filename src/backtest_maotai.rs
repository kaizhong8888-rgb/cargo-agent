use serde_json::Value;
use std::fs;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone)]
struct DailyBar {
    date: String,
    open: f64,
    close: f64,
    high: f64,
    low: f64,
    volume: f64,
    amount: f64,
    amplitude: f64,
    pct_change: f64,
    change: f64,
    turnover: f64,
}

#[derive(Debug, Clone)]
struct Trade {
    date: String,
    price: f64,
    shares: f64,
    fee: f64,
}

#[derive(Debug, Clone)]
struct BacktestResult {
    strategy_name: String,
    total_return: f64,
    annualized_return: f64,
    max_drawdown: f64,
    sharpe_ratio: f64,
    win_rate: f64,
    trade_count: usize,
    buy_hold_return: f64,
}

// ============================================================================
// Data Parsing
// ============================================================================

fn parse_data(json_str: &str) -> Vec<DailyBar> {
    // Extract the klines array from the JSON response
    let data: Value = serde_json::from_str(json_str).expect("Failed to parse JSON");
    let klines = data["data"]["klines"]
        .as_array()
        .expect("No klines array found");

    let mut bars = Vec::new();
    for line in klines {
        let s = line.as_str().unwrap();
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() < 10 {
            continue;
        }

        let bar = DailyBar {
            date: parts[0].to_string(),
            open: parts[1].parse().unwrap_or(0.0),
            close: parts[2].parse().unwrap_or(0.0),
            high: parts[3].parse().unwrap_or(0.0),
            low: parts[4].parse().unwrap_or(0.0),
            volume: parts[5].parse().unwrap_or(0.0),
            amount: parts[6].parse().unwrap_or(0.0),
            amplitude: parts[7].parse().unwrap_or(0.0),
            pct_change: parts[8].parse().unwrap_or(0.0),
            change: parts[9].parse().unwrap_or(0.0),
            turnover: parts[10].parse().unwrap_or(0.0),
        };
        bars.push(bar);
    }
    bars
}

// ============================================================================
// Technical Indicators
// ============================================================================

fn sma(data: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut result = Vec::with_capacity(data.len());
    for i in 0..data.len() {
        if i + 1 < period {
            result.push(None);
        } else {
            let sum: f64 = data[i + 1 - period..=i].iter().sum();
            result.push(Some(sum / period as f64));
        }
    }
    result
}

fn ema(data: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut result = Vec::with_capacity(data.len());
    let multiplier = 2.0 / (period as f64 + 1.0);

    for i in 0..data.len() {
        if i == 0 {
            result.push(Some(data[0]));
        } else if let Some(prev) = result[i - 1] {
            result.push(Some((data[i] - prev) * multiplier + prev));
        } else {
            result.push(Some(data[i]));
        }
    }
    result
}

fn rsi(closes: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut result = vec![None; closes.len()];
    if closes.len() < period + 1 {
        return result;
    }

    let mut gains = 0.0;
    let mut losses = 0.0;

    // Initial average
    for i in 1..=period {
        let diff = closes[i] - closes[i - 1];
        if diff > 0.0 {
            gains += diff;
        } else {
            losses -= diff;
        }
    }

    let mut avg_gain = gains / period as f64;
    let mut avg_loss = losses / period as f64;

    if avg_loss == 0.0 {
        result[period] = Some(100.0);
    } else {
        let rs = avg_gain / avg_loss;
        result[period] = Some(100.0 - 100.0 / (1.0 + rs));
    }

    for i in (period + 1)..closes.len() {
        let diff = closes[i] - closes[i - 1];
        let gain = if diff > 0.0 { diff } else { 0.0 };
        let loss = if diff < 0.0 { -diff } else { 0.0 };

        avg_gain = (avg_gain * (period as f64 - 1.0) + gain) / period as f64;
        avg_loss = (avg_loss * (period as f64 - 1.0) + loss) / period as f64;

        if avg_loss == 0.0 {
            result[i] = Some(100.0);
        } else {
            let rs = avg_gain / avg_loss;
            result[i] = Some(100.0 - 100.0 / (1.0 + rs));
        }
    }

    result
}

fn macd(closes: &[f64]) -> (Vec<Option<f64>>, Vec<Option<f64>>, Vec<Option<f64>>) {
    let ema12 = ema(closes, 12);
    let ema26 = ema(closes, 26);

    let mut dif = Vec::with_capacity(closes.len());
    for i in 0..closes.len() {
        match (ema12[i], ema26[i]) {
            (Some(e12), Some(e26)) => dif.push(Some(e12 - e26)),
            _ => dif.push(None),
        }
    }

    // DEA is EMA of DIF (period 9)
    let dif_values: Vec<f64> = dif.iter().filter_map(|x| *x).collect();
    let dea_ema = ema(&dif_values, 9);

    let mut dea = Vec::with_capacity(closes.len());
    let mut dif_idx = 0;
    for i in 0..closes.len() {
        if dif[i].is_some() {
            if dif_idx < dea_ema.len() {
                dea.push(Some(dea_ema[dif_idx]));
                dif_idx += 1;
            } else {
                dea.push(None);
            }
        } else {
            dea.push(None);
        }
    }

    let mut histogram = Vec::with_capacity(closes.len());
    for i in 0..closes.len() {
        match (dif[i], dea[i]) {
            (Some(d), Some(e)) => histogram.push(Some(2.0 * (d - e))),
            _ => histogram.push(None),
        }
    }

    (dif, dea, histogram)
}

fn bollinger_bands(closes: &[f64], period: usize, std_dev_mult: f64) -> (Vec<Option<f64>>, Vec<Option<f64>>, Vec<Option<f64>>) {
    let mut upper = vec![None; closes.len()];
    let mut middle = vec![None; closes.len()];
    let mut lower = vec![None; closes.len()];

    for i in (period - 1)..closes.len() {
        let slice = &closes[i + 1 - period..=i];
        let mean: f64 = slice.iter().sum::<f64>() / period as f64;
        let variance: f64 = slice.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / period as f64;
        let std_dev = variance.sqrt();

        middle[i] = Some(mean);
        upper[i] = Some(mean + std_dev_mult * std_dev);
        lower[i] = Some(mean - std_dev_mult * std_dev);
    }

    (upper, middle, lower)
}

// ============================================================================
// Backtest Engine
// ============================================================================

fn run_backtest(
    strategy_name: &str,
    bars: &[DailyBar],
    signals: &[Option<i8>], // 1=buy, -1=sell, 0=hold
    initial_capital: f64,
    commission_rate: f64,
) -> BacktestResult {
    let mut capital = initial_capital;
    let mut position = 0.0f64; // shares held
    let mut portfolio_values = Vec::new();
    let mut trades = Vec::new();
    let mut wins = 0;
    let mut total_trades = 0;

    let buy_hold_shares = initial_capital / bars[0].close * (1.0 - commission_rate);
    let buy_hold_final = buy_hold_shares * bars.last().unwrap().close;
    let buy_hold_return = (buy_hold_final - initial_capital) / initial_capital;

    for i in 0..bars.len() {
        let signal = signals[i].unwrap_or(0);
        let price = bars[i].close;

        if signal == 1 && capital > price * 100.0 {
            // Buy: invest 95% of capital
            let invest_amount = capital * 0.95;
            let shares = invest_amount / price * (1.0 - commission_rate);
            let fee = invest_amount * commission_rate;
            capital -= invest_amount;
            position += shares;
            trades.push(Trade {
                date: bars[i].date.clone(),
                price,
                shares,
                fee,
            });
        } else if signal == -1 && position > 0.0 {
            // Sell all
            let sell_amount = position * price * (1.0 - commission_rate);
            let fee = position * price * commission_rate;
            let buy_price = if !trades.is_empty() {
                trades.last().unwrap().price
            } else {
                price
            };
            if price > buy_price {
                wins += 1;
            }
            total_trades += 1;
            capital += sell_amount;
            position = 0.0;
            trades.push(Trade {
                date: bars[i].date.clone(),
                price,
                shares: 0.0,
                fee,
            });
        }

        let portfolio_value = capital + position * price;
        portfolio_values.push(portfolio_value);
    }

    // Close position at end
    if position > 0.0 {
        let final_price = bars.last().unwrap().close;
        capital += position * final_price * (1.0 - commission_rate);
        position = 0.0;
    }

    let final_value = portfolio_values.last().copied().unwrap_or(initial_capital);
    let total_return = (final_value - initial_capital) / initial_capital;

    // Annualized return
    let years = bars.len() as f64 / 250.0;
    let annualized_return = (1.0 + total_return).powf(1.0 / years) - 1.0;

    // Max drawdown
    let mut peak = portfolio_values[0];
    let mut max_dd = 0.0;
    for &v in &portfolio_values {
        if v > peak {
            peak = v;
        }
        let dd = (peak - v) / peak;
        if dd > max_dd {
            max_dd = dd;
        }
    }

    // Sharpe ratio (simplified, assuming risk-free rate 3%)
    let daily_returns: Vec<f64> = portfolio_values
        .windows(2)
        .map(|w| (w[1] - w[0]) / w[0])
        .collect();
    let avg_daily = daily_returns.iter().sum::<f64>() / daily_returns.len() as f64;
    let daily_std = (daily_returns
        .iter()
        .map(|r| (r - avg_daily).powi(2))
        .sum::<f64>()
        / daily_returns.len() as f64)
        .sqrt();

    let sharpe = if daily_std > 0.0 {
        (avg_daily - 0.03 / 250.0) / daily_std * (250.0_f64).sqrt()
    } else {
        0.0
    };

    let win_rate = if total_trades > 0 {
        wins as f64 / total_trades as f64
    } else {
        0.0
    };

    BacktestResult {
        strategy_name: strategy_name.to_string(),
        total_return,
        annualized_return,
        max_drawdown: max_dd,
        sharpe_ratio: sharpe,
        win_rate,
        trade_count: total_trades,
        buy_hold_return,
    }
}

// ============================================================================
// Strategies
// ============================================================================

fn strategy_sma_crossover(bars: &[DailyBar], fast: usize, slow: usize) -> Vec<Option<i8>> {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let sma_fast = sma(&closes, fast);
    let sma_slow = sma(&closes, slow);

    let mut signals = vec![None; bars.len()];
    for i in 1..bars.len() {
        if let (Some(f), Some(s), Some(pf), Some(ps)) =
            (sma_fast[i], sma_slow[i], sma_fast[i - 1], sma_slow[i - 1])
        {
            if pf <= ps && f > s {
                signals[i] = Some(1); // Golden cross
            } else if pf >= ps && f < s {
                signals[i] = Some(-1); // Death cross
            }
        }
    }
    signals
}

fn strategy_ema_crossover(bars: &[DailyBar], fast: usize, slow: usize) -> Vec<Option<i8>> {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let ema_fast = ema(&closes, fast);
    let ema_slow = ema(&closes, slow);

    let mut signals = vec![None; bars.len()];
    for i in 1..bars.len() {
        if let (Some(f), Some(s), Some(pf), Some(ps)) =
            (ema_fast[i], ema_slow[i], ema_fast[i - 1], ema_slow[i - 1])
        {
            if pf <= ps && f > s {
                signals[i] = Some(1);
            } else if pf >= ps && f < s {
                signals[i] = Some(-1);
            }
        }
    }
    signals
}

fn strategy_rsi(bars: &[DailyBar], period: usize, oversold: f64, overbought: f64) -> Vec<Option<i8>> {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let rsi_values = rsi(&closes, period);

    let mut signals = vec![None; bars.len()];
    for i in 1..bars.len() {
        if let (Some(r), Some(rp)) = (rsi_values[i], rsi_values[i - 1]) {
            if rp <= oversold && r > oversold {
                signals[i] = Some(1);
            } else if rp >= overbought && r < overbought {
                signals[i] = Some(-1);
            }
        }
    }
    signals
}

fn strategy_macd(bars: &[DailyBar]) -> Vec<Option<i8>> {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let (dif, dea, _) = macd(&closes);

    let mut signals = vec![None; bars.len()];
    for i in 1..bars.len() {
        if let (Some(d), Some(e), Some(pd), Some(pe)) = (dif[i], dea[i], dif[i - 1], dea[i - 1]) {
            if pd <= pe && d > e {
                signals[i] = Some(1); // DIF crosses above DEA
            } else if pd >= pe && d < e {
                signals[i] = Some(-1); // DIF crosses below DEA
            }
        }
    }
    signals
}

fn strategy_bollinger(bars: &[DailyBar], period: usize, std_mult: f64) -> Vec<Option<i8>> {
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let (upper, middle, lower) = bollinger_bands(&closes, period, std_mult);

    let mut signals = vec![None; bars.len()];
    for i in 1..bars.len() {
        if let (Some(u), Some(l), Some(m)) = (upper[i], lower[i], middle[i]) {
            // Buy when price crosses above lower band
            if bars[i - 1].close <= lower[i - 1].unwrap_or(f64::MAX) && bars[i].close > l {
                signals[i] = Some(1);
            }
            // Sell when price crosses below upper band or below middle
            else if bars[i - 1].close >= upper[i - 1].unwrap_or(0.0) && bars[i].close < u {
                signals[i] = Some(-1);
            } else if bars[i].close < m && bars[i - 1].close >= middle[i - 1].unwrap_or(0.0) {
                signals[i] = Some(-1);
            }
        }
    }
    signals
}

fn strategy_turtle(bars: &[DailyBar], entry_period: usize, exit_period: usize) -> Vec<Option<i8>> {
    let mut signals = vec![None; bars.len()];

    for i in entry_period..bars.len() {
        let entry_high = bars[i - entry_period..i]
            .iter()
            .map(|b| b.high)
            .fold(f64::MIN, f64::max);
        let exit_low = bars[i - exit_period..i]
            .iter()
            .map(|b| b.low)
            .fold(f64::MAX, f64::min);

        // Breakout buy
        if bars[i].close > entry_high {
            signals[i] = Some(1);
        }
        // Breakdown sell
        else if bars[i].close < exit_low {
            signals[i] = Some(-1);
        }
    }
    signals
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    // Read data from stdin or file
    let json_str = fs::read_to_string("maotai_data.json").expect("Failed to read data file");
    let bars = parse_data(&json_str);

    println!("Loaded {} bars of data ({} to {})", bars.len(), bars.first().unwrap().date, bars.last().unwrap().date);
    println!("Price range: {:.2} - {:.2}\n", bars.iter().map(|b| b.low).fold(f64::MAX, f64::min), bars.iter().map(|b| b.high).fold(f64::MIN, f64::max));

    let initial_capital = 1_000_000.0;
    let commission_rate = 0.001; // 0.1%

    let mut results = Vec::new();

    // Strategy 1: SMA 5/20
    let signals = strategy_sma_crossover(&bars, 5, 20);
    results.push(run_backtest("SMA(5/20)", &bars, &signals, initial_capital, commission_rate));

    // Strategy 2: SMA 10/60
    let signals = strategy_sma_crossover(&bars, 10, 60);
    results.push(run_backtest("SMA(10/60)", &bars, &signals, initial_capital, commission_rate));

    // Strategy 3: SMA 20/120
    let signals = strategy_sma_crossover(&bars, 20, 120);
    results.push(run_backtest("SMA(20/120)", &bars, &signals, initial_capital, commission_rate));

    // Strategy 4: EMA 12/26
    let signals = strategy_ema_crossover(&bars, 12, 26);
    results.push(run_backtest("EMA(12/26)", &bars, &signals, initial_capital, commission_rate));

    // Strategy 5: RSI 14 (30/70)
    let signals = strategy_rsi(&bars, 14, 30.0, 70.0);
    results.push(run_backtest("RSI(14) 30/70", &bars, &signals, initial_capital, commission_rate));

    // Strategy 6: RSI 14 (20/80)
    let signals = strategy_rsi(&bars, 14, 20.0, 80.0);
    results.push(run_backtest("RSI(14) 20/80", &bars, &signals, initial_capital, commission_rate));

    // Strategy 7: MACD
    let signals = strategy_macd(&bars);
    results.push(run_backtest("MACD", &bars, &signals, initial_capital, commission_rate));

    // Strategy 8: Bollinger Bands (20, 2)
    let signals = strategy_bollinger(&bars, 20, 2.0);
    results.push(run_backtest("Bollinger(20,2)", &bars, &signals, initial_capital, commission_rate));

    // Strategy 9: Turtle (20/10)
    let signals = strategy_turtle(&bars, 20, 10);
    results.push(run_backtest("Turtle(20/10)", &bars, &signals, initial_capital, commission_rate));

    // Strategy 10: Turtle (55/20)
    let signals = strategy_turtle(&bars, 55, 20);
    results.push(run_backtest("Turtle(55/20)", &bars, &signals, initial_capital, commission_rate));

    // Print results
    println!("{:-<100}", "");
    println!("{:<16} {:>10} {:>10} {:>10} {:>10} {:>8} {:>10} {:>10}",
        "策略", "总收益率", "年化收益", "最大回撤", "夏普比率", "胜率", "交易次数", "买入持有");
    println!("{:-<100}", "");

    for r in &results {
        println!("{:<16} {:>9.2}% {:>9.2}% {:>9.2}% {:>10.3} {:>7.1}% {:>10} {:>9.2}%",
            r.strategy_name,
            r.total_return * 100.0,
            r.annualized_return * 100.0,
            r.max_drawdown * 100.0,
            r.sharpe_ratio,
            r.win_rate * 100.0,
            r.trade_count,
            r.buy_hold_return * 100.0,
        );
    }
    println!("{:-<100}", "");

    // Find best by Sharpe ratio
    let best_sharpe = results.iter().max_by(|a, b| a.sharpe_ratio.partial_cmp(&b.sharpe_ratio).unwrap()).unwrap();
    println!("\n🏆 最佳策略 (夏普比率最高): {} - 夏普: {:.3}, 年化: {:.2}%, 最大回撤: {:.2}%",
        best_sharpe.strategy_name, best_sharpe.sharpe_ratio, best_sharpe.annualized_return * 100.0, best_sharpe.max_drawdown * 100.0);

    // Find best by total return
    let best_return = results.iter().max_by(|a, b| a.total_return.partial_cmp(&b.total_return).unwrap()).unwrap();
    println!("🏆 最佳策略 (总收益最高): {} - 总收益: {:.2}%, 年化: {:.2}%, 最大回撤: {:.2}%",
        best_return.strategy_name, best_return.total_return * 100.0, best_return.annualized_return * 100.0, best_return.max_drawdown * 100.0);
}
