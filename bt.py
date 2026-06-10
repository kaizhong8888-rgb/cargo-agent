#!/usr/bin/env python3
"""贵州茅台(600519) 多策略回测"""

import json, math, sys, urllib.request

def fetch_data():
    url = "https://push2his.eastmoney.com/api/qt/stock/kline/get?secid=1.600519&fields1=f1,f2,f3,f4,f5,f6&fields2=f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61&klt=101&fqt=1&beg=20150101&end=20250701&lmt=5000"
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
    resp = urllib.request.urlopen(req, timeout=30)
    raw = json.loads(resp.read().decode('utf-8'))
    klines = raw.get("data", {}).get("klines", [])
    bars = []
    for item in klines:
        p = item.split(',')
        if len(p) < 7: continue
        bars.append((p[0], float(p[1]), float(p[2]), float(p[3]), float(p[4]), float(p[5]), float(p[6])))
    return bars  # (date, open, close, high, low, volume, amount)

def sma(c, p):
    r = []
    for i in range(len(c)):
        if i < p-1: r.append(None)
        else: r.append(sum(c[i-p+1:i+1])/p)
    return r

def ema(c, p):
    if not c: return []
    k = 2.0/(p+1)
    r = [c[0]]
    for i in range(1, len(c)): r.append(c[i]*k+r[-1]*(1-k))
    return r

def rsi_c(c, period=14):
    if len(c)<period+1: return [None]*len(c)
    r = [None]*len(c)
    gains, losses = [], []
    for i in range(1, len(c)):
        ch = c[i]-c[i-1]; gains.append(max(0,ch)); losses.append(max(0,-ch))
    ag=sum(gains[:period])/period; al=sum(losses[:period])/period
    r[period]=100.0 if al==0 else 100-100/(1+ag/al)
    for i in range(period, len(gains)):
        ag=(ag*(period-1)+gains[i])/period; al=(al*(period-1)+losses[i])/period
        r[i+1]=100.0 if al==0 else 100-100/(1+ag/al)
    return r

def boll_c(c, p=20, m=2.0):
    u=[None]*len(c); mi=[None]*len(c); lo=[None]*len(c)
    for i in range(p-1, len(c)):
        w=c[i-p+1:i+1]; mn=sum(w)/p; st=math.sqrt(sum((x-mn)**2 for x in w)/p)
        mi[i]=mn; u[i]=mn+m*st; lo[i]=mn-m*st
    return u, mi, lo

def run_bt(name, bars, signals, init=1e6, cr=0.001):
    cap=init; pos=0; pvs=[]; wins=0; last=0
    for i,(d,o,cl,h,l,v,a) in enumerate(bars):
        s=signals[i] if i<len(signals) else 0
        p=cl
        if s==1 and cap>abs(p)*100:
            inv=cap*0.95; sh=inv/abs(p)*(1-cr); cap-=inv; pos+=sh; last=abs(p)
        elif s==-1 and pos>0:
            sel=pos*abs(p)*(1-cr)
            if abs(p)>last: wins+=1
            cap+=sel; pos=0
        pvs.append(cap+pos*abs(p))
    if pos>0: cap+=pos*abs(bars[-1][2])*(1-cr)
    fv=pvs[-1] if pvs else init; tr=(fv-init)/init
    yrs=len(bars)/250.0; ann=(1+tr)**(1/yrs)-1 if yrs>0 else 0
    pk=pvs[0]; mdd=0
    for v in pvs:
        if v>pk: pk=v
        dd=(pk-v)/pk
        if dd>mdd: mdd=dd
    drs=[(pvs[i]-pvs[i-1])/pvs[i-1] for i in range(1,len(pvs)) if pvs[i-1]!=0]
    if drs:
        avg=sum(drs)/len(drs); std=math.sqrt(sum((r-avg)**2 for r in drs)/len(drs))
        sr=(avg-0.03/250)/std*math.sqrt(250) if std>0 else 0
    else: sr=0
    return (name, tr, ann, mdd, sr, wins/len([s for s in signals if s==-1]) if any(s==-1 for s in signals) else 0, len([s for s in signals if s==1]))

bars = fetch_data()
print(f"数据: {len(bars)}条K线 ({bars[0][0]} ~ {bars[-1][0]})")
print(f"价格: {min(b[4] for b in bars):.2f} ~ {max(b[3] for b in bars):.2f}\n")

closes = [b[2] for b in bars]

def mk_sig(fn):
    s=[None]*len(bars)
    for i,v in enumerate(fn()):
        if i<len(s): s[i]=v
    return s

strategies = [
    ("SMA(5/20)", lambda: mk_sig(lambda: [1 if i>0 and sma(closes,5)[i-1] is not None and sma(closes,20)[i-1] is not None and sma(closes,5)[i] is not None and sma(closes,20)[i] is not None and sma(closes,5)[i-1]<=sma(closes,20)[i-1] and sma(closes,5)[i]>sma(closes,20)[i] else -1 if i>0 and sma(closes,5)[i-1] is not None and sma(closes,20)[i-1] is not None and sma(closes,5)[i] is not None and sma(closes,20)[i] is not None and sma(closes,5)[i-1]>=sma(closes,20)[i-1] and sma(closes,5)[i]<sma(closes,20)[i] else None for i in range(len(bars))])),
]

results = []
for name, fn in strategies:
    sig = fn()
    r = run_bt(name, bars, sig)
    results.append(r)
    print(f"{r[0]:<15} {r[1]*100:>9.2f}% {r[2]*100:>9.2f}% {r[3]*100:>9.2f}% {r[4]:>8.3f} {r[6]:>5}")
