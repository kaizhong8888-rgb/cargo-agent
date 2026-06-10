use serde_json::Value;
use std::fs;

fn main() {
    // Read raw JSON from eastmoney
    let raw = fs::read_to_string("maotai_raw.json").expect("need maotai_raw.json");
    let json: Value = serde_json::from_str(&raw).expect("invalid json");
    let klines = json["data"]["klines"].as_array().expect("no klines");

    // Parse bars: "2015-01-05,-101.33,-89.60,-88.03,-102.17,94515,1875063136.00,-13.95,11.58,11.73,0.83"
    // Format: date,open,close,high,low,volume,amount,...
    let mut dates: Vec<String> = Vec::new();
    let mut opens: Vec<f64> = Vec::new();
    let mut closes: Vec<f64> = Vec::new();
    let mut highs: Vec<f64> = Vec::new();
    let mut lows: Vec<f64> = Vec::new();
    
    for kl in klines {
        let s = kl.as_str().unwrap();
        let p: Vec<&str> = s.split(',').collect();
        if p.len() < 7 { continue; }
        dates.push(p[0].to_string());
        opens.push(p[1].parse().unwrap_or(0.0));
        closes.push(p[2].parse().unwrap_or(0.0));
        highs.push(p[3].parse().unwrap_or(0.0));
        lows.push(p[4].parse().unwrap_or(0.0));
    }

    let n = dates.len();
    println!("Bars: {} ({} to {})", n, dates[0], dates[n-1]);
    let min_l = lows.iter().cloned().fold(f64::MAX, f64::min);
    let max_h = highs.iter().cloned().fold(f64::MIN, f64::max);
    println!("Price range: {:.2} ~ {:.2}", min_l, max_h);
    
    // BH return
    let cr = 0.001;
    let bh_sh = 1_000_000.0 / closes[0] * (1.0 - cr);
    let bh_fin = bh_sh * closes[n-1];
    let bh_ret = (bh_fin - 1_000_000.0) / 1_000_000.0;
    let yrs = n as f64 / 250.0;

    fn bt(name: &str, c: &[f64], h: &[f64], l: &[f64], sig: &[Option<i8>], bh: f64, yrs: f64) -> (String, f64, f64, f64, f64, f64, usize) {
        let mut cap = 1_000_000.0;
        let mut pos = 0.0;
        let mut pvs = Vec::new();
        let mut wins = 0; let mut trades = 0;
        let mut lbuy = 0.0;
        let cr = 0.001;
        for i in 0..c.len() {
            let s = sig[i].unwrap_or(0);
            let p = c[i];
            if s == 1 && cap > p * 100.0 {
                let inv = cap * 0.95;
                let sh = inv / p * (1.0 - cr);
                cap -= inv; pos += sh; lbuy = p;
            } else if s == -1 && pos > 0.0 {
                let sel = pos * p * (1.0 - cr);
                if p > lbuy { wins += 1; }
                trades += 1; cap += sel; pos = 0.0;
            }
            pvs.push(cap + pos * p);
        }
        if pos > 0.0 { cap += pos * c[c.len()-1] * (1.0 - cr); pos = 0.0; }
        let fv = pvs.last().copied().unwrap_or(1_000_000.0);
        let tr = (fv - 1_000_000.0) / 1_000_000.0;
        let ar = (1.0 + tr).powf(1.0 / yrs) - 1.0;
        let mut pk = pvs[0]; let mut mdd = 0.0;
        for &v in &pvs { if v > pk { pk = v; } let dd = (pk - v) / pk; if dd > mdd { mdd = dd; } }
        let drs: Vec<f64> = pvs.windows(2).map(|w| (w[1]-w[0])/w[0]).collect();
        let ad = drs.iter().sum::<f64>() / drs.len() as f64;
        let sd = (drs.iter().map(|r| (r - ad).powi(2)).sum::<f64>() / drs.len() as f64).sqrt();
        let sr = if sd > 0.0 { (ad - 0.03/250.0) / sd * 250.0f64.sqrt() } else { 0.0 };
        let wr = if trades > 0 { wins as f64 / trades as f64 } else { 0.0 };
        (name.to_string(), tr, ar, mdd, sr, wr, trades)
    }

    fn sma(c: &[f64], p: usize) -> Vec<Option<f64>> {
        let mut r = Vec::new();
        for i in 0..c.len() {
            if i + 1 < p { r.push(None); }
            else { r.push(Some(c[i+1-p..=i].iter().sum::<f64>() / p as f64)); }
        }
        r
    }

    fn ema(c: &[f64], p: usize) -> Vec<Option<f64>> {
        let mut r = Vec::new();
        let k = 2.0 / (p as f64 + 1.0);
        for i in 0..c.len() {
            if i == 0 { r.push(Some(c[0])); }
            else if let Some(prev) = r[i-1] { r.push(Some((c[i] - prev) * k + prev)); }
            else { r.push(Some(c[i])); }
        }
        r
    }

    fn rsi(c: &[f64], p: usize) -> Vec<Option<f64>> {
        let mut r = vec![None; c.len()];
        if c.len() < p + 1 { return r; }
        let mut g = 0.0; let mut lo = 0.0;
        for i in 1..=p { let d = c[i]-c[i-1]; if d>0.0{g+=d;}else{lo-=d;} }
        let mut ag = g/p as f64; let mut al = lo/p as f64;
        if al == 0.0 { r[p] = Some(100.0); } else { r[p] = Some(100.0-100.0/(1.0+ag/al)); }
        for i in (p+1)..c.len() {
            let d = c[i]-c[i-1];
            let gain = if d>0.0{d}else{0.0}; let loss = if d<0.0{-d}else{0.0};
            ag=(ag*(p as f64-1.0)+gain)/p as f64; al=(al*(p as f64-1.0)+loss)/p as f64;
            if al==0.0{r[i]=Some(100.0);}else{r[i]=Some(100.0-100.0/(1.0+ag/al));}
        }
        r
    }

    fn macd_line(c: &[f64]) -> (Vec<Option<f64>>, Vec<Option<f64>>) {
        let e12 = ema(c, 12); let e26 = ema(c, 26);
        let mut dif = Vec::new();
        for i in 0..c.len() {
            match (e12[i], e26[i]) {
                (Some(a),Some(b)) => dif.push(Some(a-b)),
                _ => dif.push(None),
            }
        }
        let dv: Vec<f64> = dif.iter().filter_map(|x|*x).collect();
        let dea = ema(&dv, 9);
        let mut r = Vec::new(); let mut di = 0;
        for i in 0..c.len() {
            if dif[i].is_some() {
                if di < dea.len() { r.push(Some(dea[di])); di+=1; } else { r.push(None); }
            } else { r.push(None); }
        }
        (dif, r)
    }

    fn boll(c: &[f64], p: usize, m: f64) -> (Vec<Option<f64>>, Vec<Option<f64>>) {
        let mut u = vec![None; c.len()]; let mut lo = vec![None; c.len()];
        for i in (p-1)..c.len() {
            let sl = &c[i+1-p..=i];
            let mn = sl.iter().sum::<f64>() / p as f64;
            let std = (sl.iter().map(|x|(x-mn).powi(2)).sum::<f64>()/p as f64).sqrt();
            u[i] = Some(mn + m*std); lo[i] = Some(mn - m*std);
        }
        (u, lo)
    }

    // Generate signals
    let results = vec![
        { let mut s=vec![None;n]; for i in 1..n{if s[0].is_some()||true{}} ("买入持有", s) },
        {
            let sf=sma(&closes,5); let sl=sma(&closes,20); let mut s=vec![None;n];
            for i in 1..n { if let(Some(a),Some(b),Some(pa),Some(pb))=(sf[i],sl[i],sf[i-1],sl[i-1]){
                if pa<=pb&&a>b{s[i]=Some(1);}else if pa>=pb&&a<b{s[i]=Some(-1);} } }
            ("SMA(5/20)", s)
        },
        {
            let sf=sma(&closes,10); let sl=sma(&closes,60); let mut s=vec![None;n];
            for i in 1..n { if let(Some(a),Some(b),Some(pa),Some(pb))=(sf[i],sl[i],sf[i-1],sl[i-1]){
                if pa<=pb&&a>b{s[i]=Some(1);}else if pa>=pb&&a<b{s[i]=Some(-1);} } }
            ("SMA(10/60)", s)
        },
        {
            let sf=sma(&closes,20); let sl=sma(&closes,120); let mut s=vec![None;n];
            for i in 1..n { if let(Some(a),Some(b),Some(pa),Some(pb))=(sf[i],sl[i],sf[i-1],sl[i-1]){
                if pa<=pb&&a>b{s[i]=Some(1);}else if pa>=pb&&a<b{s[i]=Some(-1);} } }
            ("SMA(20/120)", s)
        },
        {
            let ef=ema(&closes,12); let el=ema(&closes,26); let mut s=vec![None;n];
            for i in 1..n { if let(Some(a),Some(b),Some(pa),Some(pb))=(ef[i],el[i],ef[i-1],el[i-1]){
                if pa<=pb&&a>b{s[i]=Some(1);}else if pa>=pb&&a<b{s[i]=Some(-1);} } }
            ("EMA(12/26)", s)
        },
        {
            let rv=rsi(&closes,14); let mut s=vec![None;n];
            for i in 1..n { if let(Some(a),Some(b))=(rv[i],rv[i-1]){
                if b<=30.0&&a>30.0{s[i]=Some(1);}else if b>=70.0&&a<70.0{s[i]=Some(-1);} } }
            ("RSI(14)30/70", s)
        },
        {
            let rv=rsi(&closes,14); let mut s=vec![None;n];
            for i in 1..n { if let(Some(a),Some(b))=(rv[i],rv[i-1]){
                if b<=20.0&&a>20.0{s[i]=Some(1);}else if b>=80.0&&a<80.0{s[i]=Some(-1);} } }
            ("RSI(14)20/80", s)
        },
        {
            let(dif,dea)=macd_line(&closes); let mut s=vec![None;n];
            for i in 1..n { if let(Some(a),Some(b),Some(pa),Some(pb))=(dif[i],dea[i],dif[i-1],dea[i-1]){
                if pa<=pb&&a>b{s[i]=Some(1);}else if pa>=pb&&a<b{s[i]=Some(-1);} } }
            ("MACD", s)
        },
        {
            let(u,lo)=boll(&closes,20,2.0); let mut s=vec![None;n];
            for i in 1..n {
                if let(Some(ub),Some(lb))=(u[i],lo[i]){
                    if closes[i-1]<=lo[i-1].unwrap_or(f64::MAX)&&closes[i]>lb{s[i]=Some(1);}
                    else if closes[i-1]>=u[i-1].unwrap_or(0.0)&&closes[i]<ub{s[i]=Some(-1);}
                }
            }
            ("BOLL(20,2)", s)
        },
        {
            let ep=20; let xp=10; let mut s=vec![None;n];
            for i in ep..n {
                let eh=highs[i-ep..i].iter().cloned().fold(f64::MIN,f64::max);
                let xl=lows[i-xp..i].iter().cloned().fold(f64::MAX,f64::min);
                if closes[i]>eh{s[i]=Some(1);}else if closes[i]<xl{s[i]=Some(-1);}
            }
            ("Turtle(20/10)", s)
        },
        {
            let ep=55; let xp=20; let mut s=vec![None;n];
            for i in ep..n {
                let eh=highs[i-ep..i].iter().cloned().fold(f64::MIN,f64::max);
                let xl=lows[i-xp..i].iter().cloned().fold(f64::MAX,f64::min);
                if closes[i]>eh{s[i]=Some(1);}else if closes[i]<xl{s[i]=Some(-1);}
            }
            ("Turtle(55/20)", s)
        },
    ];

    println!("\n{:-<110}", "");
    println!("{:<14} {:>9} {:>9} {:>9} {:>9} {:>7} {:>7}",
        "策略", "总收益%", "年化%", "最大回撤%", "夏普", "胜率%", "次数");
    println!("{:-<110}", "");
    
    let mut parsed = Vec::new();
    for (name, sig) in results {
        let r = bt(name, &closes, &highs, &lows, &sig, bh_ret, yrs);
        parsed.push(r.clone());
        println!("{:<14} {:>8.1}% {:>8.1}% {:>8.1}% {:>9.2} {:>6.1}% {:>7}",
            r.0, r.1*100.0, r.2*100.0, r.3*100.0, r.4, r.5*100.0, r.6);
    }
    println!("{:-<110}", "");
    println!("买入持有基准: 总收益 {:.1}%, 年化 {:.1}%", bh_ret*100.0, ((1.0+bh_ret).powf(1.0/yrs)-1.0)*100.0);
    
    let best_dd = parsed[1..].iter().min_by(|a,b| a.3.partial_cmp(&b.3).unwrap()).unwrap();
    let best_sr = parsed[1..].iter().max_by(|a,b| a.4.partial_cmp(&b.4).unwrap()).unwrap();
    let best_ar = parsed[1..].iter().max_by(|a,b| a.2.partial_cmp(&b.2).unwrap()).unwrap();
    let best_tr = parsed[1..].iter().max_by(|a,b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
    
    println!("\n🏆 最小回撤: {} (回撤 {:.1}%, 年化 {:.1}%, 夏普 {:.2})", best_dd.0, best_dd.3*100.0, best_dd.2*100.0, best_dd.4);
    println!("🏆 最高夏普: {} (夏普 {:.2}, 年化 {:.1}%, 回撤 {:.1}%)", best_sr.0, best_sr.4, best_sr.2*100.0, best_sr.3*100.0);
    println!("🏆 最高年化: {} (年化 {:.1}%, 回撤 {:.1}%, 夏普 {:.2})", best_ar.0, best_ar.2*100.0, best_ar.3*100.0, best_ar.4);
    println!("🏆 最高总收益: {} (总收益 {:.1}%, 年化 {:.1}%, 回撤 {:.1}%)", best_tr.0, best_tr.1*100.0, best_tr.2*100.0, best_tr.3*100.0);
}
