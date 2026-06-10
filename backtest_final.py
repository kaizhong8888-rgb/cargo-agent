#!/usr/bin/env python3
"""贵州茅台(600519) 多策略回测 - 基于东方财富前复权日线数据"""

import json
import math
import sys
from typing import List, Optional
from dataclasses import dataclass

# ============ DATA ============
RAW_DATA = '''PLACEHOLDER'''

@dataclass
class Bar:
    date: str
    open: float
    close: float
    high: float
    low: float
    volume: float
    amount: float

@dataclass
class Result:
    name: str
    total_return: float
    annualized: float
    max_dd: float
    sharpe: float
    win_rate: float
    trades: int

def parse_data(json_str: str) -> List[Bar]:
    """解析东方财富JSON数据"""
    data = json.loads(json_str)
    klines = data.get("data", {}).get("klines", [])
    bars = []
    for item in klines:
        parts = item.split(',')
        if len(parts) < 7:
            continue
        try:
            bars.append(Bar(
                date=parts[0],
                open=float(parts[1]),
                close=float(parts[2]),
                high=float(parts[3]),
                low=float(parts[4]),
                volume=float(parts[5]),
                amount=float(parts[6])
            ))
        except (ValueError, IndexError):
            continue
    return bars

# ============ INDICATORS ============

def sma(closes: List[float], p: int) -> List[Optional[float]]:
    r = []
    for i in range(len(closes)):
        if i < p - 1:
            r.append(None)
        else:
            r.append(sum(closes[i-p+1:i+1]) / p)
    return r

def ema(closes: List[float], p: int) -> List[float]:
    if not closes:
        return []
    k = 2.0 / (p + 1)
    r = [closes[0]]
    for i in range(1, len(closes)):
        r.append(closes[i] * k + r[-1] * (1 - k))
    return r

def rsi_calc(closes: List[float], period: int = 14) -> List[Optional[float]]:
    if len(closes) < period + 1:
        return [None] * len(closes)
    r = [None] * len(closes)
    gains, losses = [], []
    for i in range(1, len(closes)):
        ch = closes[i] - closes[i-1]
        gains.append(max(0, ch))
        losses.append(max(0, -ch))
    ag = sum(gains[:period]) / period
    al = sum(losses[:period]) / period
    r[period] = 100.0 if al == 0 else 100 - 100/(1+ag/al)
    for i in range(period, len(gains)):
        ag = (ag*(period-1) + gains[i]) / period
        al = (al*(period-1) + losses[i]) / period
        if al == 0:
            r[i+1] = 100.0
        else:
            rs = ag/al
            r[i+1] = 100 - 100/(1+rs)
    return r

def macd_calc(closes: List[float]):
    e12 = ema(closes, 12)
    e26 = ema(closes, 26)
    dif = [e12[i] - e26[i] for i in range(len(closes))]
    dea = ema(dif, 9)
    hist = [2*(dif[i] - dea[i]) for i in range(len(closes))]
    return dif, dea, hist

def boll_calc(closes: List[float], p: int = 20, m: float = 2.0):
    upper = [None]*len(closes)
    mid = [None]*len(closes)
    lower = [None]*len(closes)
    for i in range(p-1, len(closes)):
        w = closes[i-p+1:i+1]
        mean = sum(w)/p
        std = math.sqrt(sum((x-mean)**2 for x in w)/p)
        mid[i] = mean
        upper[i] = mean + m*std
        lower[i] = mean - m*std
    return upper, mid, lower

# ============ SIGNALS ============

def sma_cross(bars, fast, slow):
    c = [b.close for b in bars]
    sf, sl = sma(c, fast), sma(c, slow)
    sig = [None]*len(bars)
    for i in range(1, len(bars)):
        if all(x is not None for x in [sf[i],sf[i-1],sl[i],sl[i-1]]):
            if sf[i-1]<=sl[i-1] and sf[i]>sl[i]: sig[i]=1
            elif sf[i-1]>=sl[i-1] and sf[i]<sl[i]: sig[i]=-1
    return sig

def ema_cross(bars, fast, slow):
    c = [b.close for b in bars]
    ef, es = ema(c, fast), ema(c, slow)
    sig = [None]*len(bars)
    for i in range(1, len(bars)):
        if ef[i-1]<=es[i-1] and ef[i]>es[i]: sig[i]=1
        elif ef[i-1]>=es[i-1] and ef[i]<es[i]: sig[i]=-1
    return sig

def rsi_sig(bars, period=14, ob=70, os=30):
    c = [b.close for b in bars]
    rv = rsi_calc(c, period)
    sig = [None]*len(bars)
    for i in range(1, len(bars)):
        if rv[i] is not None and rv[i-1] is not None:
            if rv[i-1]<=os and rv[i]>os: sig[i]=1
            elif rv[i-1]>=ob and rv[i]<ob: sig[i]=-1
    return sig

def macd_sig(bars):
    c = [b.close for b in bars]
    dif, dea, _ = macd_calc(c)
    sig = [None]*len(bars)
    for i in range(1, len(bars)):
        if dif[i-1]<=dea[i-1] and dif[i]>dea[i]: sig[i]=1
        elif dif[i-1]>=dea[i-1] and dif[i]<dea[i]: sig[i]=-1
    return sig

def boll_sig(bars, p=20, m=2.0):
    c = [b.close for b in bars]
    u, _, l = boll_calc(c, p, m)
    sig = [None]*len(bars)
    for i in range(1, len(bars)):
        if u[i] is not None and l[i] is not None:
            if bars[i-1].close<=(l[i-1] or float('inf')) and bars[i].close>l[i]:
                sig[i]=1
            elif bars[i-1].close>=(u[i-1] or 0) and bars[i].close<u[i]:
                sig[i]=-1
            elif bars[i].close<(mid[i] if (mid:=boll_calc(c,p,m)[1])[i] else 0) and \
                 bars[i-1].close>=(mid[i-1] or 0):
                sig[i]=-1
    return sig

def turtle(bars, ep=20, xp=10):
    sig = [None]*len(bars)
    for i in range(ep, len(bars)):
        eh = max(bars[j].high for j in range(i-ep, i))
        xl = min(bars[j].low for j in range(i-xp, i))
        if bars[i].close > eh: sig[i]=1
        elif bars[i].close < xl: sig[i]=-1
    return sig

# ============ BACKTEST ============

def run(name, bars, signals, init=1000000.0, cr=0.001):
    cap = init
    pos = 0.0
    pvs = []
    trades = 0
    wins = 0
    last_buy = 0
    
    for i in range(len(bars)):
        s = signals[i] if i < len(signals) else 0
        p = bars[i].close
        
        if s == 1 and cap > abs(p)*100:
            inv = cap * 0.95
            sh = inv/abs(p)*(1-cr)
            cap -= inv; pos += sh; last_buy = abs(p)
            trades += 1
        elif s == -1 and pos > 0:
            sel = pos*abs(p)*(1-cr)
            if abs(p) > last_buy: wins += 1
            cap += sel; pos = 0
        
        pvs.append(cap + pos*abs(p))
    
    if pos > 0:
        cap += pos*abs(bars[-1].close)*(1-cr)
    
    fv = pvs[-1] if pvs else init
    tr = (fv - init)/init
    yrs = len(bars)/250.0
    ann = (1+tr)**(1/yrs)-1 if yrs>0 else 0
    
    pk = pvs[0]; mdd = 0
    for v in pvs:
        if v>pk: pk=v
        dd=(pk-v)/pk
        if dd>mdd: mdd=dd
    
    drs = [(pvs[i]-pvs[i-1])/pvs[i-1] for i in range(1,len(pvs)) if pvs[i-1]!=0]
    if drs:
        avg = sum(drs)/len(drs)
        std = math.sqrt(sum((r-avg)**2 for r in drs)/len(drs))
        sr = (avg-0.03/250)/std*math.sqrt(250) if std>0 else 0
    else:
        sr = 0
    
    wr = wins/trades if trades>0 else 0
    return Result(name, tr, ann, mdd, sr, wr, trades)

# ============ MAIN ============

def main():
    raw = RAW_DATA.strip()
    bars = parse_data(raw)
    if not bars:
        print("ERROR: No bars parsed!")
        sys.exit(1)
    
    print(f"数据: {len(bars)}条K线 ({bars[0].date} ~ {bars[-1].date})")
    print(f"价格: {min(b.low for b in bars):.2f} ~ {max(b.high for b in bars):.2f}\n")
    
    strategies = [
        ("SMA(5/20)", lambda: sma_cross(bars, 5, 20)),
        ("SMA(10/60)", lambda: sma_cross(bars, 10, 60)),
        ("SMA(20/120)", lambda: sma_cross(bars, 20, 120)),
        ("EMA(12/26)", lambda: ema_cross(bars, 12, 26)),
        ("RSI(14)30/70", lambda: rsi_sig(bars, 14, 70, 30)),
        ("RSI(14)20/80", lambda: rsi_sig(bars, 14, 80, 20)),
        ("MACD", lambda: macd_sig(bars)),
        ("BOLL(20,2)", lambda: boll_sig(bars)),
        ("Turtle(20/10)", lambda: turtle(bars, 20, 10)),
        ("Turtle(55/20)", lambda: turtle(bars, 55, 20)),
    ]
    
    results = []
    hdr = f"{'策略':<15} {'总收益%':>10} {'年化%':>10} {'回撤%':>10} {'夏普':>8} {'胜率%':>7} {'次数':>5}"
    print("="*len(hdr))
    print(hdr)
    print("="*len(hdr))
    
    for name, fn in strategies:
        sig = fn()
        r = run(name, bars, sig)
        results.append(r)
        print(f"{r.name:<15} {r.total_return*100:>9.2f}% {r.annualized*100:>9.2f}% {r.max_dd*100:>9.2f}% {r.sharpe:>8.3f} {r.win_rate*100:>6.1f}% {r.trades:>5}")
    
    print("="*len(hdr))
    
    # BH
    bh_tr = (abs(bars[-1].close)-abs(bars[0].close))/abs(bars[0].close)
    bh_ann = (1+bh_tr)**(1/(len(bars)/250))-1
    print(f"\n买入持有: 总收益 {bh_tr*100:.2f}%, 年化 {bh_ann*100:.2f}%")
    
    best_sharpe = max(results, key=lambda x: x.sharpe)
    best_ret = max(results, key=lambda x: x.total_return)
    best_dd = min(results, key=lambda x: x.max_dd)
    best_ann = max(results, key=lambda x: x.annualized)
    
    print(f"\n🏆 最高夏普: {best_sharpe.name} (夏普:{best_sharpe.sharpe:.3f}, 年化:{best_sharpe.annualized*100:.2f}%, 回撤:{best_sharpe.max_dd*100:.2f}%)")
    print(f"🏆 最高总收益: {best_ret.name} (总收益:{best_ret.total_return*100:.2f}%, 年化:{best_ret.annualized*100:.2f}%, 回撤:{best_ret.max_dd*100:.2f}%)")
    print(f"🏆 最高年化: {best_ann.name} (年化:{best_ann.annualized*100:.2f}%, 回撤:{best_ann.max_dd*100:.2f}%, 夏普:{best_ann.sharpe:.3f})")
    print(f"🏆 最小回撤: {best_dd.name} (回撤:{best_dd.max_dd*100:.2f}%, 年化:{best_dd.annualized*100:.2f}%, 夏普:{best_dd.sharpe:.3f})")

if __name__ == "__main__":
    main()
