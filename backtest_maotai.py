#!/usr/bin/env python3
"""贵州茅台(600519) 多策略回测 - 基于东方财富前复权数据"""

import json
import math
import re
from typing import List, Tuple, Optional
from dataclasses import dataclass

@dataclass
class Bar:
    date: str
    open: float
    close: float
    high: float
    low: float
    volume: float
    amount: float

def parse_data(json_str: str) -> List[Bar]:
    """解析东方财富K线数据"""
    # 提取klines数组
    match = re.search(r'"klines":\[(.+?)\]', json_str, re.DOTALL)
    if not match:
        raise ValueError("No klines found")
    
    klines_str = match.group(1)
    # 提取所有引号内的字符串
    items = re.findall(r'"([^"]*)"', klines_str)
    
    bars = []
    for item in items:
        parts = item.split(',')
        if len(parts) < 7:
            continue
        try:
            bar = Bar(
                date=parts[0],
                open=float(parts[1]),
                close=float(parts[2]),
                high=float(parts[3]),
                low=float(parts[4]),
                volume=float(parts[5]),
                amount=float(parts[6])
            )
            bars.append(bar)
        except:
            continue
    
    return bars

def calc_returns(bars: List[Bar]) -> List[float]:
    """计算每日收益率"""
    returns = []
    for i in range(1, len(bars)):
        ret = (bars[i].close - bars[i-1].close) / abs(bars[i-1].close) if bars[i-1].close != 0 else 0
        returns.append(ret)
    return returns

def sma(closes: List[float], period: int) -> List[Optional[float]]:
    """简单移动平均"""
    result = []
    for i in range(len(closes)):
        if i < period - 1:
            result.append(None)
        else:
            avg = sum(closes[i-period+1:i+1]) / period
            result.append(avg)
    return result

def ema(closes: List[float], period: int) -> List[Optional[float]]:
    """指数移动平均"""
    if not closes:
        return []
    k = 2.0 / (period + 1)
    result = [closes[0]]
    for i in range(1, len(closes)):
        ema_val = closes[i] * k + result[-1] * (1 - k)
        result.append(ema_val)
    return result

def rsi(closes: List[float], period: int = 14) -> List[Optional[float]]:
    """RSI指标"""
    if len(closes) < period + 1:
        return [None] * len(closes)
    
    result = [None] * len(closes)
    gains = []
    losses = []
    
    for i in range(1, len(closes)):
        change = closes[i] - closes[i-1]
        gains.append(max(0, change))
        losses.append(max(0, -change))
    
    # 初始平均
    avg_gain = sum(gains[:period]) / period
    avg_loss = sum(losses[:period]) / period
    
    if avg_loss == 0:
        result[period] = 100.0
    else:
        rs = avg_gain / avg_loss
        result[period] = 100 - 100/(1+rs)
    
    for i in range(period, len(gains)):
        avg_gain = (avg_gain * (period - 1) + gains[i]) / period
        avg_loss = (avg_loss * (period - 1) + losses[i]) / period
        
        if avg_loss == 0:
            result[i+1] = 100.0
        else:
            rs = avg_gain / avg_loss
            result[i+1] = 100 - 100/(1+rs)
    
    return result

def macd(closes: List[float]) -> Tuple[List[Optional[float]], List[Optional[float]], List[Optional[float]]]:
    """MACD指标"""
    ema12 = ema(closes, 12)
    ema26 = ema(closes, 26)
    
    dif = []
    for i in range(len(closes)):
        if ema12[i] is not None and ema26[i] is not None:
            dif.append(ema12[i] - ema26[i])
        else:
            dif.append(None)
    
    # DEA是DIF的9日EMA
    dif_vals = [x for x in dif if x is not None]
    dea_ema = ema(dif_vals, 9) if dif_vals else []
    
    dea = []
    dif_idx = 0
    for i in range(len(closes)):
        if dif[i] is not None:
            if dif_idx < len(dea_ema):
                dea.append(dea_ema[dif_idx])
                dif_idx += 1
            else:
                dea.append(None)
        else:
            dea.append(None)
    
    histogram = []
    for i in range(len(closes)):
        if dif[i] is not None and dea[i] is not None:
            histogram.append(2 * (dif[i] - dea[i]))
        else:
            histogram.append(None)
    
    return dif, dea, histogram

def bollinger(closes: List[float], period: int = 20, std_dev: float = 2.0) -> Tuple[List[Optional[float]], List[Optional[float]], List[Optional[float]]]:
    """布林带"""
    upper = [None] * len(closes)
    middle = [None] * len(closes)
    lower = [None] * len(closes)
    
    for i in range(period-1, len(closes)):
        window = closes[i-period+1:i+1]
        mean = sum(window) / period
        variance = sum((x - mean)**2 for x in window) / period
        std = math.sqrt(variance)
        
        middle[i] = mean
        upper[i] = mean + std_dev * std
        lower[i] = mean - std_dev * std
    
    return upper, middle, lower

@dataclass
class BacktestResult:
    name: str
    total_return: float
    annualized_return: float
    max_drawdown: float
    sharpe_ratio: float
    win_rate: float
    trade_count: int
    final_value: float

def run_backtest(name: str, bars: List[Bar], signals: List[Optional[int]], 
                 initial_capital: float = 1000000.0, commission: float = 0.001) -> BacktestResult:
    """运行回测"""
    capital = initial_capital
    position = 0.0
    portfolio_values = []
    trades = 0
    wins = 0
    last_buy_price = 0
    
    # 买入持有基准
    bh_shares = initial_capital / abs(bars[0].close) * (1 - commission)
    bh_final = bh_shares * abs(bars[-1].close)
    bh_return = (bh_final - initial_capital) / initial_capital
    
    for i in range(len(bars)):
        sig = signals[i] if i < len(signals) else 0
        price = bars[i].close
        
        if sig == 1 and capital > abs(price) * 100:
            # 买入
            invest = capital * 0.95
            shares = invest / abs(price) * (1 - commission)
            capital -= invest
            position += shares
            last_buy_price = abs(price)
            trades += 1
        elif sig == -1 and position > 0:
            # 卖出
            sell_value = position * abs(price) * (1 - commission)
            if abs(price) > last_buy_price:
                wins += 1
            capital += sell_value
            position = 0
        
        portfolio_values.append(capital + position * abs(price))
    
    # 最后清仓
    if position > 0:
        capital += position * abs(bars[-1].close) * (1 - commission)
        position = 0
    
    final_value = portfolio_values[-1] if portfolio_values else initial_capital
    total_return = (final_value - initial_capital) / initial_capital
    
    # 年化收益
    years = len(bars) / 250.0
    annualized = (1 + total_return) ** (1/years) - 1 if years > 0 else 0
    
    # 最大回撤
    peak = portfolio_values[0]
    max_dd = 0
    for v in portfolio_values:
        if v > peak:
            peak = v
        dd = (peak - v) / peak
        if dd > max_dd:
            max_dd = dd
    
    # 夏普比率
    daily_returns = [(portfolio_values[i] - portfolio_values[i-1])/portfolio_values[i-1] 
                     for i in range(1, len(portfolio_values)) if portfolio_values[i-1] != 0]
    if daily_returns:
        avg_ret = sum(daily_returns) / len(daily_returns)
        std_ret = math.sqrt(sum((r - avg_ret)**2 for r in daily_returns) / len(daily_returns))
        sharpe = (avg_ret - 0.03/250) / std_ret * math.sqrt(250) if std_ret > 0 else 0
    else:
        sharpe = 0
    
    win_rate = wins / trades if trades > 0 else 0
    
    return BacktestResult(
        name=name,
        total_return=total_return,
        annualized_return=annualized,
        max_drawdown=max_dd,
        sharpe_ratio=sharpe,
        win_rate=win_rate,
        trade_count=trades,
        final_value=final_value
    )

def strategy_sma_cross(bars: List[Bar], fast: int, slow: int) -> List[Optional[int]]:
    """SMA交叉策略"""
    closes = [b.close for b in bars]
    sf = sma(closes, fast)
    sl = sma(closes, slow)
    
    signals = [None] * len(bars)
    for i in range(1, len(bars)):
        if sf[i] is not None and sf[i-1] is not None and sl[i] is not None and sl[i-1] is not None:
            if sf[i-1] <= sl[i-1] and sf[i] > sl[i]:
                signals[i] = 1  # 金叉买入
            elif sf[i-1] >= sl[i-1] and sf[i] < sl[i]:
                signals[i] = -1  # 死叉卖出
    return signals

def strategy_ema_cross(bars: List[Bar], fast: int, slow: int) -> List[Optional[int]]:
    """EMA交叉策略"""
    closes = [b.close for b in bars]
    ef = ema(closes, fast)
    es = ema(closes, slow)
    
    signals = [None] * len(bars)
    for i in range(1, len(bars)):
        if ef[i] is not None and ef[i-1] is not None and es[i] is not None and es[i-1] is not None:
            if ef[i-1] <= es[i-1] and ef[i] > es[i]:
                signals[i] = 1
            elif ef[i-1] >= es[i-1] and ef[i] < es[i]:
                signals[i] = -1
    return signals

def strategy_rsi(bars: List[Bar], period: int = 14, oversold: float = 30, overbought: float = 70) -> List[Optional[int]]:
    """RSI策略"""
    closes = [b.close for b in bars]
    rsi_vals = rsi(closes, period)
    
    signals = [None] * len(bars)
    for i in range(1, len(bars)):
        if rsi_vals[i] is not None and rsi_vals[i-1] is not None:
            if rsi_vals[i-1] <= oversold and rsi_vals[i] > oversold:
                signals[i] = 1  # 超卖反弹买入
            elif rsi_vals[i-1] >= overbought and rsi_vals[i] < overbought:
                signals[i] = -1  # 超买回落卖出
    return signals

def strategy_macd(bars: List[Bar]) -> List[Optional[int]]:
    """MACD策略"""
    closes = [b.close for b in bars]
    dif, dea, _ = macd(closes)
    
    signals = [None] * len(bars)
    for i in range(1, len(bars)):
        if dif[i] is not None and dif[i-1] is not None and dea[i] is not None and dea[i-1] is not None:
            if dif[i-1] <= dea[i-1] and dif[i] > dea[i]:
                signals[i] = 1  # DIF上穿DEA
            elif dif[i-1] >= dea[i-1] and dif[i] < dea[i]:
                signals[i] = -1  # DIF下穿DEA
    return signals

def strategy_bollinger(bars: List[Bar], period: int = 20, std_dev: float = 2.0) -> List[Optional[int]]:
    """布林带策略"""
    closes = [b.close for b in bars]
    upper, middle, lower = bollinger(closes, period, std_dev)
    
    signals = [None] * len(bars)
    for i in range(1, len(bars)):
        if upper[i] is not None and lower[i] is not None:
            # 价格从下轨上方突破买入
            if bars[i-1].close <= (lower[i-1] or float('inf')) and bars[i].close > lower[i]:
                signals[i] = 1
            # 价格从上轨下方突破卖出
            elif bars[i-1].close >= (upper[i-1] or 0) and bars[i].close < upper[i]:
                signals[i] = -1
            # 跌破中轨卖出
            elif bars[i].close < (middle[i] or 0) and bars[i-1].close >= (middle[i-1] or 0):
                signals[i] = -1
    return signals

def strategy_turtle(bars: List[Bar], entry_period: int = 20, exit_period: int = 10) -> List[Optional[int]]:
    """海龟交易策略"""
    signals = [None] * len(bars)
    
    for i in range(entry_period, len(bars)):
        # 计算入场高点
        entry_high = max(bars[j].high for j in range(i-entry_period, i))
        # 计算出场低点
        exit_low = min(bars[j].low for j in range(i-exit_period, i))
        
        if bars[i].close > entry_high:
            signals[i] = 1  # 突破高点买入
        elif bars[i].close < exit_low:
            signals[i] = -1  # 跌破低点卖出
    
    return signals

def main():
    # 读取数据
    with open('maotai_data.json', 'r') as f:
        json_str = f.read()
    
    bars = parse_data(json_str)
    print(f"加载数据: {len(bars)} 条K线 ({bars[0].date} 至 {bars[-1].date})")
    print(f"价格范围: {min(b.low for b in bars):.2f} ~ {max(b.high for b in bars):.2f}\n")
    
    # 定义策略
    strategies = [
        ("SMA(5/20)", lambda: strategy_sma_cross(bars, 5, 20)),
        ("SMA(10/60)", lambda: strategy_sma_cross(bars, 10, 60)),
        ("SMA(20/120)", lambda: strategy_sma_cross(bars, 20, 120)),
        ("EMA(12/26)", lambda: strategy_ema_cross(bars, 12, 26)),
        ("RSI(14) 30/70", lambda: strategy_rsi(bars, 14, 30, 70)),
        ("RSI(14) 20/80", lambda: strategy_rsi(bars, 14, 20, 80)),
        ("MACD", lambda: strategy_macd(bars)),
        ("BOLL(20,2)", lambda: strategy_bollinger(bars, 20, 2.0)),
        ("海龟(20/10)", lambda: strategy_turtle(bars, 20, 10)),
        ("海龟(55/20)", lambda: strategy_turtle(bars, 55, 20)),
    ]
    
    results = []
    
    print("="*100)
    print(f"{'策略':<15} {'总收益率%':>10} {'年化收益%':>10} {'最大回撤%':>10} {'夏普比率':>10} {'胜率%':>8} {'交易次数':>8}")
    print("="*100)
    
    for name, strategy_fn in strategies:
        signals = strategy_fn()
        result = run_backtest(name, bars, signals)
        results.append(result)
        
        print(f"{result.name:<15} {result.total_return*100:>9.2f}% {result.annualized_return*100:>9.2f}% "
              f"{result.max_drawdown*100:>9.2f}% {result.sharpe_ratio:>10.3f} "
              f"{result.win_rate*100:>7.1f}% {result.trade_count:>8}")
    
    print("="*100)
    
    # 买入持有基准
    closes = [b.close for b in bars]
    bh_return = (abs(bars[-1].close) - abs(bars[0].close)) / abs(bars[0].close)
    years = len(bars) / 250.0
    bh_annual = (1 + bh_return) ** (1/years) - 1
    print(f"\n买入持有基准: 总收益 {bh_return*100:.2f}%, 年化 {bh_annual*100:.2f}%")
    
    # 找出最佳策略
    best_sharpe = max(results, key=lambda x: x.sharpe_ratio)
    best_return = max(results, key=lambda x: x.total_return)
    best_dd = min(results, key=lambda x: x.max_drawdown)
    
    print(f"\n🏆 最佳夏普比率: {best_sharpe.name} (夏普: {best_sharpe.sharpe_ratio:.3f}, 年化: {best_sharpe.annualized_return*100:.2f}%, 回撤: {best_sharpe.max_drawdown*100:.2f}%)")
    print(f"🏆 最高总收益: {best_return.name} (总收益: {best_return.total_return*100:.2f}%, 年化: {best_return.annualized_return*100:.2f}%, 回撤: {best_return.max_drawdown*100:.2f}%)")
    print(f"🏆 最小回撤: {best_dd.name} (回撤: {best_dd.max_drawdown*100:.2f}%, 年化: {best_dd.annualized_return*100:.2f}%, 夏普: {best_dd.sharpe_ratio:.3f})")

if __name__ == "__main__":
    main()
