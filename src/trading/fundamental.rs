use chrono::NaiveDate;

/// 行业分类常量
pub const INDUSTRIES: &[&str] = &[
    "银行",
    "非银金融",
    "房地产",
    "建筑装饰",
    "建筑材料",
    "食品饮料",
    "医药生物",
    "农林牧渔",
    "纺织服装",
    "家用电器",
    "计算机",
    "电子",
    "通信",
    "传媒",
    "机械设备",
    "汽车",
    "国防军工",
    "公用事业",
    "交通运输",
    "商贸零售",
];

/// 基本面数据快照（单只股票单期财报数据）
#[derive(Debug, Clone)]
pub struct Fundamentals {
    /// 股票代码
    pub symbol: String,
    /// 所属行业（申万一级行业分类）
    pub industry: String,
    /// 报告期
    pub report_date: NaiveDate,
    /// 发布日期（财报实际对外公布的日期，通常晚于报告期）
    pub publish_date: NaiveDate,

    // ===== 估值因子 =====
    /// 市盈率 PE (TTM)
    pub pe_ttm: f64,
    /// 市净率 PB (MRQ)
    pub pb: f64,
    /// 市销率 PS (TTM)
    pub ps_ttm: f64,
    /// 市现率 P/OCF (TTM)
    pub p_ocf: f64,
    /// EV/EBITDA
    pub ev_ebitda: f64,
    /// 股息率 (%)
    pub dividend_yield: f64,

    // ===== 盈利因子 =====
    /// 净资产收益率 ROE (%)
    pub roe: f64,
    /// 总资产收益率 ROA (%)
    pub roa: f64,
    /// 毛利率 (%)
    pub gross_margin: f64,
    /// 净利率 (%)
    pub net_margin: f64,
    /// 投入资本回报率 ROIC (%)
    pub roic: f64,

    // ===== 成长因子 =====
    /// 营收同比增速 (%)
    pub revenue_growth_yoy: f64,
    /// 净利润同比增速 (%)
    pub net_profit_growth_yoy: f64,
    /// 营收环比增速 (%)
    pub revenue_growth_qoq: f64,
    /// 净利润环比增速 (%)
    pub net_profit_qoq: f64,
    /// 过去3年营收复合增长率 CAGR (%)
    pub revenue_cagr_3y: f64,
    /// 过去3年净利润复合增长率 CAGR (%)
    pub profit_cagr_3y: f64,

    // ===== 财务健康因子 =====
    /// 资产负债率 (%)
    pub debt_to_assets: f64,
    /// 流动比率
    pub current_ratio: f64,
    /// 速动比率
    pub quick_ratio: f64,
    /// 利息保障倍数
    pub interest_coverage: f64,
    /// 自由现金流 (万元)
    pub free_cashflow: f64,

    // ===== 市值 =====
    /// 总市值 (亿元)
    pub market_cap: f64,
    /// 流通市值 (亿元)
    pub float_cap: f64,
}

/// 简单的可复现随机数生成器 (xorshift32)
struct SimpleRng {
    state: u32,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: ((seed as u32) ^ ((seed >> 32) as u32) ^ 0x5BD1E995).wrapping_mul(0x5BD1E995),
        }
    }

    /// 生成 [0, 1) 范围内的 f64
    fn next_f64(&mut self) -> f64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        (x as f64) / (u32::MAX as f64)
    }

    /// 生成 [min, max) 范围内的 f64
    fn range(&mut self, min: f64, max: f64) -> f64 {
        min + self.next_f64() * (max - min)
    }
}

impl Fundamentals {
    /// 创建一条基本面数据记录（含合理默认值）
    ///
    /// `industry` 和 `publish_date` 使用默认值（空字符串和 report_date），
    /// 可通过 struct update syntax 手动设置。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: &str,
        report_date: NaiveDate,
        pe_ttm: f64,
        pb: f64,
        ps_ttm: f64,
        p_ocf: f64,
        ev_ebitda: f64,
        dividend_yield: f64,
        roe: f64,
        roa: f64,
        gross_margin: f64,
        net_margin: f64,
        roic: f64,
        revenue_growth_yoy: f64,
        net_profit_growth_yoy: f64,
        revenue_growth_qoq: f64,
        net_profit_qoq: f64,
        revenue_cagr_3y: f64,
        profit_cagr_3y: f64,
        debt_to_assets: f64,
        current_ratio: f64,
        quick_ratio: f64,
        interest_coverage: f64,
        free_cashflow: f64,
        market_cap: f64,
        float_cap: f64,
    ) -> Self {
        // publish_date 默认比 report_date 晚 60 天（模拟财报发布延迟）
        let publish_date = report_date
            .checked_add_signed(chrono::Duration::days(60))
            .unwrap_or(report_date);

        Self {
            symbol: symbol.to_string(),
            industry: String::new(), // 需要手动设置
            report_date,
            publish_date,
            pe_ttm,
            pb,
            ps_ttm,
            p_ocf,
            ev_ebitda,
            dividend_yield,
            roe,
            roa,
            gross_margin,
            net_margin,
            roic,
            revenue_growth_yoy,
            net_profit_growth_yoy,
            revenue_growth_qoq,
            net_profit_qoq,
            revenue_cagr_3y,
            profit_cagr_3y,
            debt_to_assets,
            current_ratio,
            quick_ratio,
            interest_coverage,
            free_cashflow,
            market_cap,
            float_cap,
        }
    }

    /// 设置行业（链式调用）
    #[must_use]
    pub fn with_industry(mut self, industry: &str) -> Self {
        self.industry = industry.to_string();
        self
    }

    /// 设置发布日期（链式调用）
    #[must_use]
    pub fn with_publish_date(mut self, publish_date: NaiveDate) -> Self {
        self.publish_date = publish_date;
        self
    }

    /// 创建模拟基本面数据（用于测试/演示）
    ///
    /// 生成的数据包含：
    /// - 多样化的股票类型（低估值高成长、高估值高成长等）
    /// - 行业分布（按股票符号映射）
    /// - 财报发布延迟（report_date + 60-90 天）
    pub fn generate_mock(count: usize, seed: u64) -> Vec<Fundamentals> {
        let symbols = [
            "600519.SH",
            "000858.SZ",
            "000333.SZ",
            "601318.SH",
            "600036.SH",
            "000001.SZ",
            "601166.SH",
            "000651.SZ",
            "600276.SH",
            "002714.SZ",
            "601888.SH",
            "000568.SZ",
            "600887.SH",
            "002304.SZ",
            "601012.SH",
            "002415.SZ",
            "603259.SH",
            "300750.SZ",
            "002594.SZ",
            "600900.SH",
        ];

        let industries_map: std::collections::HashMap<&str, &str> = [
            ("600519.SH", "食品饮料"),
            ("000858.SZ", "食品饮料"),
            ("000333.SZ", "家用电器"),
            ("601318.SH", "非银金融"),
            ("600036.SH", "银行"),
            ("000001.SZ", "银行"),
            ("601166.SH", "银行"),
            ("000651.SZ", "家用电器"),
            ("600276.SH", "医药生物"),
            ("002714.SZ", "农林牧渔"),
            ("601888.SH", "商贸零售"),
            ("000568.SZ", "食品饮料"),
            ("600887.SH", "食品饮料"),
            ("002304.SZ", "食品饮料"),
            ("601012.SH", "电子"),
            ("002415.SZ", "计算机"),
            ("603259.SH", "医药生物"),
            ("300750.SZ", "电子"),
            ("002594.SZ", "汽车"),
            ("600900.SH", "公用事业"),
        ]
        .into_iter()
        .collect();

        let mut results = Vec::with_capacity(count);
        let base_date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

        let mut rng = SimpleRng::new(seed);

        for i in 0..count {
            let symbol = &symbols[i % symbols.len()];
            let industry = industries_map
                .get(*symbol)
                .copied()
                .unwrap_or("其他")
                .to_string();

            // 生成多样化的股票类型
            let stock_type = rng.next_f64();

            let (pe, pb, roe, rev_growth, profit_growth, debt_ratio, margin) = if stock_type < 0.25
            {
                // 低估值高成长 (理想标的)
                (
                    rng.range(5.0, 15.0),
                    rng.range(0.8, 2.3),
                    rng.range(15.0, 35.0),
                    rng.range(15.0, 45.0),
                    rng.range(20.0, 55.0),
                    rng.range(30.0, 60.0),
                    rng.range(10.0, 25.0),
                )
            } else if stock_type < 0.50 {
                // 高估值高成长 (成长股)
                (
                    rng.range(30.0, 80.0),
                    rng.range(3.0, 11.0),
                    rng.range(20.0, 45.0),
                    rng.range(25.0, 75.0),
                    rng.range(30.0, 90.0),
                    rng.range(40.0, 65.0),
                    rng.range(8.0, 20.0),
                )
            } else if stock_type < 0.75 {
                // 低估值低成长 (价值股)
                (
                    rng.range(5.0, 15.0),
                    rng.range(0.5, 1.5),
                    rng.range(5.0, 15.0),
                    rng.range(-5.0, 10.0),
                    rng.range(-3.0, 9.0),
                    rng.range(60.0, 80.0),
                    rng.range(5.0, 15.0),
                )
            } else {
                // 高估值低成长 (危险信号)
                (
                    rng.range(40.0, 120.0),
                    rng.range(5.0, 20.0),
                    rng.range(3.0, 11.0),
                    rng.range(-10.0, 5.0),
                    rng.range(-8.0, 7.0),
                    rng.range(55.0, 80.0),
                    rng.range(3.0, 11.0),
                )
            };

            let report_date = base_date
                .checked_sub_signed(chrono::Duration::days((i * 90) as i64))
                .unwrap_or(base_date);

            // 模拟财报发布延迟（报告期后 60-90 天）
            let publish_delay_days = rng.range(60.0, 90.0).round() as i64;
            let publish_date = report_date
                .checked_add_signed(chrono::Duration::days(publish_delay_days))
                .unwrap_or(report_date);

            results.push(Fundamentals {
                symbol: symbol.to_string(),
                industry,
                report_date,
                publish_date,
                pe_ttm: pe,
                pb,
                ps_ttm: rng.range(2.0, 10.0),
                p_ocf: rng.range(4.0, 24.0),
                ev_ebitda: ev_ebitda_from_pe(pe),
                dividend_yield: rng.range(1.0, 5.0),
                roe,
                roa: roe * 0.55,
                gross_margin: margin + 15.0,
                net_margin: margin,
                roic: roe * 0.85,
                revenue_growth_yoy: rev_growth,
                net_profit_growth_yoy: profit_growth,
                revenue_growth_qoq: rev_growth * 0.3,
                net_profit_qoq: profit_growth * 0.3,
                revenue_cagr_3y: rev_growth * 0.6,
                profit_cagr_3y: profit_growth * 0.6,
                debt_to_assets: debt_ratio,
                current_ratio: rng.range(1.0, 4.0),
                quick_ratio: rng.range(0.5, 2.5),
                interest_coverage: rng.range(3.0, 23.0),
                free_cashflow: rng.range(-500.0, 4500.0),
                market_cap: rng.range(50.0, 5050.0),
                float_cap: rng.range(30.0, 3030.0),
            });
        }

        results
    }

    /// 检查基本面数据是否完整（无 NaN/Inf/负异常值）
    pub fn is_valid(&self) -> bool {
        let vals = [
            self.pe_ttm,
            self.pb,
            self.ps_ttm,
            self.p_ocf,
            self.ev_ebitda,
            self.roe,
            self.roa,
            self.gross_margin,
            self.net_margin,
            self.roic,
            self.revenue_growth_yoy,
            self.net_profit_growth_yoy,
            self.revenue_cagr_3y,
            self.profit_cagr_3y,
            self.debt_to_assets,
            self.current_ratio,
            self.quick_ratio,
            self.market_cap,
        ];
        vals.iter().all(|v| v.is_finite() && !v.is_nan())
            && self.pe_ttm > 0.0
            && self.pb > 0.0
            && self.market_cap > 0.0
    }
}

/// 从 PE 估算 EV/EBITDA（简化估算）
fn ev_ebitda_from_pe(pe: f64) -> f64 {
    // EV/EBITDA 通常约为 PE 的 0.6-0.8 倍
    pe * 0.7
}

/// 从 CSV 解析基本面数据
pub fn parse_fundamentals_csv(csv_text: &str) -> anyhow::Result<Vec<Fundamentals>> {
    let mut records = Vec::new();
    let mut lines = csv_text.lines();

    // 跳过表头
    let header = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("CSV 为空，缺少表头"))?;
    let headers: Vec<&str> = header.split(',').map(|s| s.trim()).collect();

    // 建立列名到索引的映射
    let col_idx = |name: &str| -> anyhow::Result<usize> {
        headers
            .iter()
            .position(|&h| h == name)
            .ok_or_else(|| anyhow::anyhow!("CSV 缺少列: {}", name))
    };

    let idx_symbol = col_idx("symbol")?;
    let idx_industry = col_idx("industry").ok();
    let idx_report_date = col_idx("report_date")?;
    let idx_publish_date = col_idx("publish_date").ok();
    let idx_pe = col_idx("pe_ttm")?;
    let idx_pb = col_idx("pb")?;
    let idx_ps = col_idx("ps_ttm")?;
    let idx_pocf = col_idx("p_ocf")?;
    let idx_ev_ebitda = col_idx("ev_ebitda")?;
    let idx_div_yield = col_idx("dividend_yield")?;
    let idx_roe = col_idx("roe")?;
    let idx_roa = col_idx("roa")?;
    let idx_gross_margin = col_idx("gross_margin")?;
    let idx_net_margin = col_idx("net_margin")?;
    let idx_roic = col_idx("roic")?;
    let idx_rev_growth = col_idx("revenue_growth_yoy")?;
    let idx_profit_growth = col_idx("net_profit_growth_yoy")?;
    let idx_rev_qoq = col_idx("revenue_growth_qoq")?;
    let idx_profit_qoq = col_idx("net_profit_qoq")?;
    let idx_rev_cagr = col_idx("revenue_cagr_3y")?;
    let idx_profit_cagr = col_idx("profit_cagr_3y")?;
    let idx_debt = col_idx("debt_to_assets")?;
    let idx_current_ratio = col_idx("current_ratio")?;
    let idx_quick_ratio = col_idx("quick_ratio")?;
    let idx_interest_cov = col_idx("interest_coverage")?;
    let idx_fcf = col_idx("free_cashflow")?;
    let idx_mcap = col_idx("market_cap")?;
    let idx_fcap = col_idx("float_cap")?;

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if fields.len() < headers.len() {
            continue;
        }

        let parse_f64 = |i: usize| -> f64 {
            fields
                .get(i)
                .and_then(|s| s.parse().ok())
                .unwrap_or(f64::NAN)
        };

        let report_date = fields
            .get(idx_report_date)
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            .unwrap_or(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap());

        let publish_date = idx_publish_date
            .and_then(|idx| {
                fields
                    .get(idx)
                    .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
            })
            .unwrap_or_else(|| {
                report_date
                    .checked_add_signed(chrono::Duration::days(60))
                    .unwrap_or(report_date)
            });

        let industry = idx_industry
            .and_then(|idx| fields.get(idx).copied())
            .unwrap_or("")
            .to_string();

        records.push(Fundamentals {
            symbol: fields.get(idx_symbol).unwrap_or(&"").to_string(),
            industry,
            report_date,
            publish_date,
            pe_ttm: parse_f64(idx_pe),
            pb: parse_f64(idx_pb),
            ps_ttm: parse_f64(idx_ps),
            p_ocf: parse_f64(idx_pocf),
            ev_ebitda: parse_f64(idx_ev_ebitda),
            dividend_yield: parse_f64(idx_div_yield),
            roe: parse_f64(idx_roe),
            roa: parse_f64(idx_roa),
            gross_margin: parse_f64(idx_gross_margin),
            net_margin: parse_f64(idx_net_margin),
            roic: parse_f64(idx_roic),
            revenue_growth_yoy: parse_f64(idx_rev_growth),
            net_profit_growth_yoy: parse_f64(idx_profit_growth),
            revenue_growth_qoq: parse_f64(idx_rev_qoq),
            net_profit_qoq: parse_f64(idx_profit_qoq),
            revenue_cagr_3y: parse_f64(idx_rev_cagr),
            profit_cagr_3y: parse_f64(idx_profit_cagr),
            debt_to_assets: parse_f64(idx_debt),
            current_ratio: parse_f64(idx_current_ratio),
            quick_ratio: parse_f64(idx_quick_ratio),
            interest_coverage: parse_f64(idx_interest_cov),
            free_cashflow: parse_f64(idx_fcf),
            market_cap: parse_f64(idx_mcap),
            float_cap: parse_f64(idx_fcap),
        });
    }

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mock_data() {
        let data = Fundamentals::generate_mock(20, 42);
        assert_eq!(data.len(), 20);
        for d in &data {
            assert!(!d.symbol.is_empty());
            assert!(d.market_cap > 0.0);
            assert!(!d.industry.is_empty(), "行业不应为空");
            assert!(d.publish_date >= d.report_date, "发布日期应不早于报告期");
        }
    }

    #[test]
    fn test_is_valid() {
        let f = Fundamentals::generate_mock(1, 100).pop().unwrap();
        assert!(f.is_valid());
    }

    #[test]
    fn test_csv_parse() {
        let csv = "symbol,industry,report_date,publish_date,pe_ttm,pb,ps_ttm,p_ocf,ev_ebitda,dividend_yield,roe,roa,gross_margin,net_margin,roic,revenue_growth_yoy,net_profit_growth_yoy,revenue_growth_qoq,net_profit_qoq,revenue_cagr_3y,profit_cagr_3y,debt_to_assets,current_ratio,quick_ratio,interest_coverage,free_cashflow,market_cap,float_cap
600519.SH,食品饮料,2025-12-31,2026-02-28,25.5,8.2,12.1,20.3,17.8,1.5,32.0,17.6,65.0,28.0,27.2,18.5,22.3,5.5,6.7,11.1,13.4,45.0,2.5,1.8,12.0,8500.0,35000.0,25000.0
000858.SZ,食品饮料,2025-12-31,2026-02-28,15.0,3.5,4.2,12.0,10.5,3.0,18.0,9.9,55.0,18.0,15.3,8.0,10.5,2.4,3.1,4.8,6.3,55.0,1.5,1.0,8.0,3000.0,8000.0,6000.0";
        let records = parse_fundamentals_csv(csv).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].symbol, "600519.SH");
        assert_eq!(records[0].industry, "食品饮料");
        assert_eq!(records[1].symbol, "000858.SZ");
        assert!((records[0].pe_ttm - 25.5).abs() < 0.01);
        assert!((records[1].roe - 18.0).abs() < 0.01);
    }

    #[test]
    fn test_with_industry_and_publish_date() {
        let base_date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        let pub_date = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();

        let f = Fundamentals::new(
            "600519.SH",
            base_date,
            25.0,
            8.0,
            12.0,
            20.0,
            17.5,
            1.5,
            32.0,
            17.6,
            65.0,
            28.0,
            27.2,
            18.5,
            22.3,
            5.5,
            6.7,
            11.1,
            13.4,
            45.0,
            2.5,
            1.8,
            12.0,
            8500.0,
            35000.0,
            25000.0,
        )
        .with_industry("食品饮料")
        .with_publish_date(pub_date);

        assert_eq!(f.industry, "食品饮料");
        assert_eq!(f.publish_date, pub_date);
    }

    #[test]
    fn test_industry_distribution() {
        let data = Fundamentals::generate_mock(100, 42);
        let mut industry_counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();

        for d in &data {
            *industry_counts.entry(&d.industry).or_insert(0) += 1;
        }

        // 应该有多个行业
        assert!(
            industry_counts.len() >= 5,
            "行业分布应足够多样，当前: {:?}",
            industry_counts
        );

        println!("\n行业分布:");
        for (industry, count) in industry_counts.iter() {
            println!("  {}: {}", industry, count);
        }
    }
}
