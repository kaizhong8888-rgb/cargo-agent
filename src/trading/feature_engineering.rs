/// ML 特征工程模块
/// 为机器学习策略提供丰富的特征集，防止数据泄露，支持滚动窗口训练
use super::data::Candle;
use super::indicators;

/// 特征集合
#[derive(Debug, Clone)]
pub struct FeatureSet {
    /// 特征名称列表
    pub feature_names: Vec<String>,
    /// 特征矩阵: [samples][features]
    pub features: Vec<Vec<f64>>,
    /// 标签: [samples] (1=涨, 0=跌)
    pub labels: Vec<i32>,
}

impl FeatureSet {
    /// 获取有效样本 (无 NaN 的行)
    pub fn valid_samples(&self) -> (Vec<Vec<f64>>, Vec<i32>) {
        let mut valid_features = Vec::new();
        let mut valid_labels = Vec::new();

        for (features, label) in self.features.iter().zip(self.labels.iter()) {
            if features.iter().all(|f| !f.is_nan() && f.is_finite()) {
                valid_features.push(features.clone());
                valid_labels.push(*label);
            }
        }

        (valid_features, valid_labels)
    }

    /// 样本数量
    pub fn num_samples(&self) -> usize {
        self.features.len()
    }

    /// 特征数量
    pub fn num_features(&self) -> usize {
        self.feature_names.len()
    }
}

/// 特征引擎配置
#[derive(Debug, Clone)]
pub struct FeatureEngineConfig {
    /// 预测未来 N 期 (默认 1)
    pub forecast_horizon: usize,
    /// 预测阈值 (涨跌幅 > threshold 才标记为正)
    pub return_threshold: f64,
    /// 是否包含滞后特征
    pub include_lags: bool,
    /// 滞后阶数
    pub lag_periods: Vec<usize>,
    /// 是否包含滚动统计
    pub include_rolling_stats: bool,
    /// 滚动窗口
    pub rolling_windows: Vec<usize>,
}

impl Default for FeatureEngineConfig {
    fn default() -> Self {
        Self {
            forecast_horizon: 1,
            return_threshold: 0.0,
            include_lags: true,
            lag_periods: vec![1, 2, 3, 5],
            include_rolling_stats: true,
            rolling_windows: vec![5, 10, 20],
        }
    }
}

/// 特征引擎
pub struct FeatureEngine {
    config: FeatureEngineConfig,
}

impl FeatureEngine {
    pub fn new(config: FeatureEngineConfig) -> Self {
        Self { config }
    }

    /// 从 K 线数据构建特征集
    pub fn build_features(&self, candles: &[Candle]) -> FeatureSet {
        let len = candles.len();
        if len < 30 {
            return FeatureSet {
                feature_names: vec![],
                features: vec![],
                labels: vec![],
            };
        }

        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
        let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
        let volumes: Vec<f64> = candles.iter().map(|c| c.volume).collect();

        let mut feature_names = Vec::new();
        let mut feature_matrix: Vec<Vec<f64>> = Vec::new();

        // ========== 1. 价格类特征 ==========

        // 对数收益率
        let log_ret = indicators::log_returns(&closes);
        feature_names.push("log_return".to_string());
        feature_matrix.push(log_ret.clone());

        // 不同周期的动量
        for period in &[5, 10, 20] {
            let mom = indicators::momentum(&closes, *period);
            let norm = self.normalize_series(&mom, &closes);
            feature_names.push(format!("momentum_{}", period));
            feature_matrix.push(norm);
        }

        // 变化率
        for period in &[5, 10, 20] {
            let roc = indicators::roc(&closes, *period);
            feature_names.push(format!("roc_{}", period));
            feature_matrix.push(roc);
        }

        // ========== 2. 均线类特征 ==========

        // SMA 偏离度
        for period in &[5, 10, 20, 50] {
            let sma = indicators::sma(&closes, *period);
            let deviation: Vec<f64> = closes
                .iter()
                .zip(sma.iter())
                .map(|(&c, &s)| if s > 0.0 { (c - s) / s } else { 0.0 })
                .collect();
            feature_names.push(format!("sma_deviation_{}", period));
            feature_matrix.push(deviation);
        }

        // EMA 偏离度
        for period in &[5, 12, 26] {
            let ema = indicators::ema(&closes, *period);
            let deviation: Vec<f64> = closes
                .iter()
                .zip(ema.iter())
                .map(|(&c, &e)| if e > 0.0 { (c - e) / e } else { 0.0 })
                .collect();
            feature_names.push(format!("ema_deviation_{}", period));
            feature_matrix.push(deviation);
        }

        // 均线斜率 (EMA 变化率)
        for period in &[5, 10, 20] {
            let ema = indicators::ema(&closes, *period);
            let slope: Vec<f64> = ema
                .iter()
                .skip(1)
                .zip(ema.iter())
                .map(|(&curr, &prev)| {
                    if prev > 0.0 {
                        (curr - prev) / prev
                    } else {
                        0.0
                    }
                })
                .collect();
            // 补一个值对齐
            let mut slope_full = vec![0.0];
            slope_full.extend(slope);
            feature_names.push(format!("ema_slope_{}", period));
            feature_matrix.push(slope_full);
        }

        // ========== 3. 振荡器特征 ==========

        // RSI
        let rsi = indicators::rsi(&closes, 14);
        let rsi_norm: Vec<f64> = rsi.iter().map(|&v| v / 100.0).collect();
        feature_names.push("rsi_14".to_string());
        feature_matrix.push(rsi_norm);

        // MACD 偏离度
        let macd = indicators::macd(&closes, 12, 26, 9);
        let macd_norm: Vec<f64> = closes
            .iter()
            .zip(macd.macd_line.iter())
            .map(|(&c, &m)| if c > 0.0 { m / c } else { 0.0 })
            .collect();
        feature_names.push("macd_normalized".to_string());
        feature_matrix.push(macd_norm);

        let hist_norm: Vec<f64> = closes
            .iter()
            .zip(macd.histogram.iter())
            .map(|(&c, &h)| if c > 0.0 { h / c } else { 0.0 })
            .collect();
        feature_names.push("macd_histogram_normalized".to_string());
        feature_matrix.push(hist_norm);

        // Stochastic
        let stoch = indicators::stochastic(&highs, &lows, &closes, 14, 3);
        let stoch_k_norm: Vec<f64> = stoch.k.iter().map(|&v| v / 100.0).collect();
        let stoch_d_norm: Vec<f64> = stoch.d.iter().map(|&v| v / 100.0).collect();
        feature_names.push("stoch_k".to_string());
        feature_names.push("stoch_d".to_string());
        feature_matrix.push(stoch_k_norm);
        feature_matrix.push(stoch_d_norm);

        // Williams %R
        let williams = indicators::williams_r(&highs, &lows, &closes, 14);
        let williams_norm: Vec<f64> = williams.iter().map(|&v| (v + 100.0) / 100.0).collect();
        feature_names.push("williams_r".to_string());
        feature_matrix.push(williams_norm);

        // CCI
        let cci = indicators::cci(&highs, &lows, &closes, 20);
        let cci_norm: Vec<f64> = cci.iter().map(|&v| v / 200.0).collect();
        feature_names.push("cci_20".to_string());
        feature_matrix.push(cci_norm);

        // ========== 4. 波动率特征 ==========

        // ATR 相对值
        let atr = indicators::atr(&highs, &lows, &closes, 14);
        let atr_pct: Vec<f64> = atr
            .iter()
            .zip(closes.iter())
            .map(|(&a, &c)| if c > 0.0 { a / c } else { 0.0 })
            .collect();
        feature_names.push("atr_pct_14".to_string());
        feature_matrix.push(atr_pct);

        // 布林带宽度
        let bb = indicators::bollinger_bands(&closes, 20, 2.0);
        let bb_width: Vec<f64> = bb
            .upper
            .iter()
            .zip(bb.lower.iter())
            .zip(bb.middle.iter())
            .map(|((&u, &l), &m)| if m > 0.0 { (u - l) / m } else { 0.0 })
            .collect();
        feature_names.push("bb_width_20".to_string());
        feature_matrix.push(bb_width);

        // 布林带位置
        let bb_pos: Vec<f64> = closes
            .iter()
            .zip(bb.upper.iter())
            .zip(bb.lower.iter())
            .map(|((&c, &u), &l)| {
                let range = u - l;
                if range > 0.0 {
                    (c - l) / range
                } else {
                    0.5
                }
            })
            .collect();
        feature_names.push("bb_position".to_string());
        feature_matrix.push(bb_pos);

        // 历史波动率
        let hist_vol = indicators::historical_volatility(&log_ret, 20);
        feature_names.push("hist_vol_20".to_string());
        feature_matrix.push(hist_vol);

        // ========== 5. 成交量特征 ==========

        // 成交量变化率
        let vol_roc: Vec<f64> = volumes
            .iter()
            .skip(1)
            .zip(volumes.iter())
            .map(|(&curr, &prev)| {
                if prev > 0.0 {
                    (curr - prev) / prev
                } else {
                    0.0
                }
            })
            .collect();
        let mut vol_roc_full = vec![0.0];
        vol_roc_full.extend(vol_roc);
        feature_names.push("volume_roc".to_string());
        feature_matrix.push(vol_roc_full);

        // 成交量相对均值
        let vol_sma = indicators::sma(&volumes, 20);
        let vol_ratio: Vec<f64> = volumes
            .iter()
            .zip(vol_sma.iter())
            .map(|(&v, &s)| if s > 0.0 { v / s } else { 1.0 })
            .collect();
        feature_names.push("volume_ratio_20".to_string());
        feature_matrix.push(vol_ratio);

        // OBV 偏离度
        let obv = indicators::obv(&closes, &volumes);
        let obv_sma = indicators::sma(&obv, 20);
        let obv_dev: Vec<f64> = obv
            .iter()
            .zip(obv_sma.iter())
            .map(|(&o, &s)| if s != 0.0 { (o - s) / s.abs() } else { 0.0 })
            .collect();
        feature_names.push("obv_deviation".to_string());
        feature_matrix.push(obv_dev);

        // ========== 6. ADX 特征 ==========

        let adx_values = indicators::adx(&highs, &lows, &closes, 14);
        let adx_norm: Vec<f64> = adx_values.iter().map(|&v| v / 100.0).collect();
        feature_names.push("adx_14".to_string());
        feature_matrix.push(adx_norm);

        // ========== 7. 滞后特征 ==========

        if self.config.include_lags {
            for &lag in &self.config.lag_periods {
                let mut lagged = vec![0.0; lag];
                lagged.extend(log_ret.iter().take(len - lag).cloned());
                feature_names.push(format!("log_return_lag_{}", lag));
                feature_matrix.push(lagged);
            }
        }

        // ========== 8. 滚动统计特征 ==========

        if self.config.include_rolling_stats {
            for &window in &self.config.rolling_windows {
                // 滚动均值
                let roll_mean = indicators::sma(&log_ret, window);
                feature_names.push(format!("ret_mean_{}", window));
                feature_matrix.push(roll_mean);

                // 滚动标准差
                let mut roll_std = vec![f64::NAN; len];
                for i in (window - 1)..len {
                    let segment = &log_ret[i + 1 - window..=i];
                    let mean: f64 = segment.iter().filter(|&&v| !v.is_nan()).sum::<f64>()
                        / segment.iter().filter(|&&v| !v.is_nan()).count() as f64;
                    let variance: f64 = segment
                        .iter()
                        .filter(|&&v| !v.is_nan())
                        .map(|v| (v - mean).powi(2))
                        .sum::<f64>()
                        / segment.iter().filter(|&&v| !v.is_nan()).count() as f64;
                    roll_std[i] = variance.sqrt();
                }
                feature_names.push(format!("ret_std_{}", window));
                feature_matrix.push(roll_std);

                // 滚动偏度 (skewness)
                let mut roll_skew = vec![f64::NAN; len];
                for i in (window - 1)..len {
                    let segment: Vec<f64> = log_ret[i + 1 - window..=i]
                        .iter()
                        .filter(|&&v| !v.is_nan())
                        .copied()
                        .collect();
                    if segment.len() >= 3 {
                        let mean: f64 = segment.iter().sum::<f64>() / segment.len() as f64;
                        let std: f64 = segment.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
                            / segment.len() as f64;
                        let std = std.sqrt();
                        if std > 0.0 {
                            let skew: f64 = segment
                                .iter()
                                .map(|v| ((v - mean) / std).powi(3))
                                .sum::<f64>()
                                / segment.len() as f64;
                            roll_skew[i] = skew;
                        }
                    }
                }
                feature_names.push(format!("ret_skew_{}", window));
                feature_matrix.push(roll_skew);
            }
        }

        // ========== 9. 价格模式特征 ==========

        // 最高价/最低价比率 (K线实体比例)
        let body_ratio: Vec<f64> = candles
            .iter()
            .map(|c| {
                let body = (c.close - c.open).abs();
                let range = c.high - c.low;
                if range > 0.0 {
                    body / range
                } else {
                    0.0
                }
            })
            .collect();
        feature_names.push("body_ratio".to_string());
        feature_matrix.push(body_ratio);

        // 上影线/下影线比率
        let upper_shadow: Vec<f64> = candles
            .iter()
            .map(|c| {
                let body_top = c.open.max(c.close);
                let range = c.high - c.low;
                if range > 0.0 {
                    (c.high - body_top) / range
                } else {
                    0.0
                }
            })
            .collect();
        feature_names.push("upper_shadow".to_string());
        feature_matrix.push(upper_shadow);

        let lower_shadow: Vec<f64> = candles
            .iter()
            .map(|c| {
                let body_bottom = c.open.min(c.close);
                let range = c.high - c.low;
                if range > 0.0 {
                    (body_bottom - c.low) / range
                } else {
                    0.0
                }
            })
            .collect();
        feature_names.push("lower_shadow".to_string());
        feature_matrix.push(lower_shadow);

        // 转置特征矩阵: [features][len] → [len][features]
        let num_features = feature_matrix.len();
        let mut features = vec![vec![0.0; num_features]; len];
        for i in 0..len {
            for j in 0..num_features {
                features[i][j] = feature_matrix[j][i];
            }
        }

        // ========== 生成标签 ==========
        let labels = self.generate_labels(&closes);

        FeatureSet {
            feature_names,
            features,
            labels,
        }
    }

    /// 生成标签
    /// 如果未来 forecast_horizon 期的收益率 > threshold → 1 (涨), 否则 0 (跌)
    fn generate_labels(&self, closes: &[f64]) -> Vec<i32> {
        let len = closes.len();
        let mut labels = vec![0; len];

        for i in 0..len {
            if i + self.config.forecast_horizon < len {
                let future_price = closes[i + self.config.forecast_horizon];
                let current_price = closes[i];
                if current_price > 0.0 {
                    let future_return = (future_price - current_price) / current_price;
                    if future_return > self.config.return_threshold {
                        labels[i] = 1;
                    } else {
                        labels[i] = 0;
                    }
                }
            }
        }

        labels
    }

    /// 标准化特征序列 (除以价格)
    fn normalize_series(&self, series: &[f64], base: &[f64]) -> Vec<f64> {
        series
            .iter()
            .zip(base.iter())
            .map(|(&s, &b)| if b > 0.0 { s / b } else { 0.0 })
            .collect()
    }
}

/// 滚动窗口训练器 (防止数据泄露)
pub struct RollingTrainer {
    /// 训练窗口大小
    pub train_window: usize,
    /// 测试窗口大小
    pub test_window: usize,
    /// 滚动步长
    pub step: usize,
}

impl RollingTrainer {
    pub fn new(train_window: usize, test_window: usize, step: usize) -> Self {
        Self {
            train_window,
            test_window,
            step,
        }
    }

    /// 执行滚动训练测试
    /// 返回每个窗口的训练/测试性能
    pub fn rolling_eval<T, F>(&self, features: &FeatureSet, model_builder: F) -> Vec<RollingResult>
    where
        T: Model,
        F: Fn(&[Vec<f64>], &[i32]) -> T,
    {
        let n = features.num_samples();
        if n < self.train_window + self.test_window {
            return vec![];
        }

        let mut results = Vec::new();
        let mut start = 0;

        while start + self.train_window + self.test_window <= n {
            let train_end = start + self.train_window;
            let test_end = train_end + self.test_window;

            // 训练集
            let (train_x, train_y) = self.extract_window(features, start, train_end);
            let model = model_builder(&train_x, &train_y);

            // 测试集
            let (test_x, test_y) = self.extract_window(features, train_end, test_end);
            let train_acc = model.accuracy(&train_x, &train_y);
            let test_acc = model.accuracy(&test_x, &test_y);

            results.push(RollingResult {
                window_start: start,
                window_end: test_end,
                train_accuracy: train_acc,
                test_accuracy: test_acc,
                overfitting_gap: train_acc - test_acc,
            });

            start += self.step;
        }

        results
    }

    fn extract_window(
        &self,
        features: &FeatureSet,
        start: usize,
        end: usize,
    ) -> (Vec<Vec<f64>>, Vec<i32>) {
        let mut x = Vec::new();
        let mut y = Vec::new();

        for i in start..end {
            if i < features.features.len() && i < features.labels.len() {
                let f = &features.features[i];
                if f.iter().all(|v| !v.is_nan() && v.is_finite()) {
                    x.push(f.clone());
                    y.push(features.labels[i]);
                }
            }
        }

        (x, y)
    }
}

/// 滚动评估结果
#[derive(Debug, Clone)]
pub struct RollingResult {
    pub window_start: usize,
    pub window_end: usize,
    pub train_accuracy: f64,
    pub test_accuracy: f64,
    pub overfitting_gap: f64,
}

/// 简化的模型 trait
pub trait Model {
    /// 训练
    fn train(&mut self, x: &[Vec<f64>], y: &[i32]);
    /// 预测
    fn predict(&self, x: &[f64]) -> i32;
    /// 准确率
    fn accuracy(&self, x: &[Vec<f64>], y: &[i32]) -> f64 {
        if x.is_empty() {
            return 0.0;
        }
        let correct = x
            .iter()
            .zip(y.iter())
            .filter(|(xi, yi)| self.predict(xi) == **yi)
            .count();
        correct as f64 / x.len() as f64
    }
}

/// 简单的逻辑回归模型 (用于演示)
pub struct SimpleLogisticRegression {
    pub weights: Vec<f64>,
    pub bias: f64,
    pub learning_rate: f64,
    pub epochs: usize,
}

impl SimpleLogisticRegression {
    pub fn new(n_features: usize, learning_rate: f64, epochs: usize) -> Self {
        Self {
            weights: vec![0.0; n_features],
            bias: 0.0,
            learning_rate,
            epochs,
        }
    }

    fn sigmoid(x: f64) -> f64 {
        1.0 / (1.0 + (-x).exp())
    }
}

impl Model for SimpleLogisticRegression {
    fn train(&mut self, x: &[Vec<f64>], y: &[i32]) {
        if x.is_empty() || x[0].is_empty() {
            return;
        }

        let n_features = x[0].len();
        self.weights = vec![0.0; n_features];
        self.bias = 0.0;

        for _ in 0..self.epochs {
            for (xi, yi) in x.iter().zip(y.iter()) {
                let z: f64 = xi
                    .iter()
                    .zip(self.weights.iter())
                    .map(|(a, b)| a * b)
                    .sum::<f64>()
                    + self.bias;
                let pred = Self::sigmoid(z);
                let error = pred - *yi as f64;

                for j in 0..n_features {
                    self.weights[j] -= self.learning_rate * error * xi[j];
                }
                self.bias -= self.learning_rate * error;
            }
        }
    }

    fn predict(&self, x: &[f64]) -> i32 {
        let z: f64 = x
            .iter()
            .zip(self.weights.iter())
            .map(|(a, b)| a * b)
            .sum::<f64>()
            + self.bias;
        if Self::sigmoid(z) >= 0.5 {
            1
        } else {
            0
        }
    }
}
