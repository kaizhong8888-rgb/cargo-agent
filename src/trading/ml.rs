//! 机器学习策略模块 (纯 Rust 实现，零外部 ML 依赖)
//!
//! 实现以下 ML 策略:
//! - 线性回归预测 (Online SGD)
//! - 决策树分类 (CART 算法)
//! - KNN 分类 (K-Nearest Neighbors)
//! - ML Ensemble (投票组合)

use crate::trading::data::Candle;
use crate::trading::indicators;
use crate::trading::strategy::{Signal, Strategy};
use serde::{Deserialize, Serialize};

// ========================================================================
// 特征工程
// ========================================================================

/// 特征配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    pub rsi_period: usize,
    pub sma_fast: usize,
    pub sma_slow: usize,
    pub macd_fast: usize,
    pub macd_slow: usize,
    pub macd_signal: usize,
    pub bb_period: usize,
    pub bb_std: f64,
    pub atr_period: usize,
    pub lookback: usize,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            rsi_period: 14, sma_fast: 5, sma_slow: 20,
            macd_fast: 12, macd_slow: 26, macd_signal: 9,
            bb_period: 20, bb_std: 2.0, atr_period: 14, lookback: 5,
        }
    }
}

const N_FEATURES: usize = 12;

/// 特征矩阵 [样本数][特征数]
type FeatureMatrix = Vec<[f64; N_FEATURES]>;

/// 提取特征和标签
pub fn extract_features(candles: &[Candle], config: &FeatureConfig) -> (FeatureMatrix, Vec<usize>, usize) {
    let n = candles.len();
    let warmup = config.sma_slow.max(config.bb_period).max(config.atr_period + config.lookback);
    if n < warmup + 1 { return (Vec::new(), Vec::new(), 0); }

    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let highs: Vec<f64> = candles.iter().map(|c| c.high).collect();
    let lows: Vec<f64> = candles.iter().map(|c| c.low).collect();
    let volumes: Vec<f64> = candles.iter().map(|c| c.volume).collect();

    let rsi = indicators::rsi(&closes, config.rsi_period);
    let sma_f = indicators::sma(&closes, config.sma_fast);
    let sma_s = indicators::sma(&closes, config.sma_slow);
    let macd_out = indicators::macd(&closes, config.macd_fast, config.macd_slow, config.macd_signal);
    let bb = indicators::bollinger_bands(&closes, config.bb_period, config.bb_std);
    let atr = indicators::atr(&highs, &lows, &closes, config.atr_period);

    let start_idx = warmup;
    let n_samples = n - start_idx;
    if n_samples == 0 { return (Vec::new(), Vec::new(), 0); }

    let mut features = Vec::with_capacity(n_samples);
    let mut labels = Vec::with_capacity(n_samples);

    for i in 0..n_samples {
        let idx = start_idx + i;
        let past_idx = idx - config.lookback;

        let f0 = if rsi[idx].is_nan() { 0.5 } else { rsi[idx] / 100.0 };
        let f1 = if sma_s[idx].is_nan() || sma_s[idx] == 0.0 { 1.0 } else { sma_f[idx] / sma_s[idx] };
        let f2 = if closes[idx] == 0.0 || macd_out.histogram[idx].is_nan() { 0.0 } else { macd_out.histogram[idx] / closes[idx] };
        let f3 = if closes[idx] == 0.0 || macd_out.signal_line[idx].is_nan() { 0.0 } else { macd_out.signal_line[idx] / closes[idx] };
        let bb_range = bb.upper[idx] - bb.lower[idx];
        let f4 = if bb_range == 0.0 || bb.upper[idx].is_nan() { 0.5 } else { (closes[idx] - bb.lower[idx]) / bb_range };
        let f5 = if closes[idx] == 0.0 || atr[idx].is_nan() { 0.01 } else { atr[idx] / closes[idx] };
        let f6 = if closes[past_idx] == 0.0 { 0.0 } else { (closes[idx] - closes[past_idx]) / closes[past_idx] };
        let short_past = idx.saturating_sub(3);
        let f7 = if closes[short_past] == 0.0 { 0.0 } else { (closes[idx] - closes[short_past]) / closes[short_past] };
        let avg_vol: f64 = volumes[past_idx..idx].iter().sum::<f64>() / config.lookback as f64;
        let f8 = if avg_vol == 0.0 { 1.0 } else { volumes[idx] / avg_vol };
        let daily_range = highs[idx] - lows[idx];
        let f9 = if daily_range == 0.0 { 0.5 } else { (closes[idx] - lows[idx]) / daily_range };
        let f10 = if bb.middle[idx] == 0.0 || bb.middle[idx].is_nan() { 0.0 } else { (bb.upper[idx] - bb.lower[idx]) / bb.middle[idx] };
        let prev_sma = if idx >= 3 && !sma_f[idx - 3].is_nan() { sma_f[idx - 3] } else { sma_f[idx] };
        let f11 = if prev_sma == 0.0 { 0.0 } else { (sma_f[idx] - prev_sma) / prev_sma };

        features.push([f0, f1, f2, f3, f4, f5, f6, f7, f8, f9, f10, f11]);

        let future_idx = (idx + config.lookback).min(n - 1);
        let future_ret = if closes[idx] == 0.0 { 0.0 } else { (closes[future_idx] - closes[idx]) / closes[idx] };
        labels.push(if future_ret > 0.0 { 1 } else { 0 });
    }

    (features, labels, start_idx)
}

// ========================================================================
// 决策树 (CART 简化版)
// ========================================================================

#[derive(Debug)]
enum TreeNode {
    Leaf(usize),
    Split {
        feature_idx: usize,
        threshold: f64,
        left: Box<TreeNode>,
        right: Box<TreeNode>,
    },
}

impl TreeNode {
    fn predict(&self, x: &[f64]) -> usize {
        match self {
            TreeNode::Leaf(label) => *label,
            TreeNode::Split { feature_idx, threshold, left, right } => {
                if x[*feature_idx] <= *threshold {
                    left.predict(x)
                } else {
                    right.predict(x)
                }
            }
        }
    }

    fn build(features: &[&[f64]], labels: &[usize], max_depth: usize, min_samples: usize) -> Self {
        if max_depth == 0 || features.len() <= min_samples || features.is_empty() {
            return TreeNode::Leaf(Self::majority_label(labels));
        }

        // 检查是否纯节点
        if labels.iter().all(|&l| l == labels[0]) {
            return TreeNode::Leaf(labels[0]);
        }

        // 寻找最佳分割
        let mut best_gain = 0.0f64;
        let mut best_feature = 0;
        let mut best_threshold = 0.0;

        let total_samples = features.len();
        let parent_entropy = Self::entropy(labels);

        for feat_idx in 0..N_FEATURES {
            // 收集该特征的所有值并排序
            let mut values: Vec<f64> = features.iter().map(|x| x[feat_idx]).collect();
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            values.dedup_by(|a, b| (*a - *b).abs() < 1e-10);

            for threshold in values.windows(2).map(|w| (w[0] + w[1]) / 2.0) {
                let (left_labels, right_labels) = Self::split_labels(features, labels, feat_idx, threshold);

                if left_labels.is_empty() || right_labels.is_empty() { continue; }

                let left_entropy = Self::entropy(&left_labels);
                let right_entropy = Self::entropy(&right_labels);
                let weighted_entropy = (left_labels.len() as f64 * left_entropy
                    + right_labels.len() as f64 * right_entropy) / total_samples as f64;
                let gain = parent_entropy - weighted_entropy;

                if gain > best_gain {
                    best_gain = gain;
                    best_feature = feat_idx;
                    best_threshold = threshold;
                }
            }
        }

        if best_gain < 1e-10 {
            return TreeNode::Leaf(Self::majority_label(labels));
        }

        let (left_features, right_features, left_labels, right_labels) =
            Self::split_data(features, labels, best_feature, best_threshold);

        let left_refs: Vec<&[f64]> = left_features.iter().map(|f| f.as_slice()).collect();
        let right_refs: Vec<&[f64]> = right_features.iter().map(|f| f.as_slice()).collect();

        let left = Box::new(Self::build(&left_refs, &left_labels, max_depth - 1, min_samples));
        let right = Box::new(Self::build(&right_refs, &right_labels, max_depth - 1, min_samples));

        TreeNode::Split {
            feature_idx: best_feature,
            threshold: best_threshold,
            left,
            right,
        }
    }

    fn split_labels(features: &[&[f64]], labels: &[usize], feat_idx: usize, threshold: f64)
        -> (Vec<usize>, Vec<usize>)
    {
        let mut left = Vec::new();
        let mut right = Vec::new();
        for (x, &l) in features.iter().zip(labels.iter()) {
            if x[feat_idx] <= threshold { left.push(l); } else { right.push(l); }
        }
        (left, right)
    }

    fn split_data(features: &[&[f64]], labels: &[usize], feat_idx: usize, threshold: f64)
        -> (Vec<Vec<f64>>, Vec<Vec<f64>>, Vec<usize>, Vec<usize>)
    {
        let mut left_f = Vec::new();
        let mut right_f = Vec::new();
        let mut left_l = Vec::new();
        let mut right_l = Vec::new();
        for (x, &l) in features.iter().zip(labels.iter()) {
            if x[feat_idx] <= threshold {
                left_f.push(x.to_vec()); left_l.push(l);
            } else {
                right_f.push(x.to_vec()); right_l.push(l);
            }
        }
        (left_f, right_f, left_l, right_l)
    }

    fn majority_label(labels: &[usize]) -> usize {
        let mut counts = [0usize; 2];
        for &l in labels { counts[l] += 1; }
        if counts[0] >= counts[1] { 0 } else { 1 }
    }

    fn entropy(labels: &[usize]) -> f64 {
        if labels.is_empty() { return 0.0; }
        let n = labels.len() as f64;
        let mut counts = [0usize; 2];
        for &l in labels { counts[l] += 1; }
        let mut entropy = 0.0;
        for &c in &counts {
            if c > 0 {
                let p = c as f64 / n;
                entropy -= p * p.log2();
            }
        }
        entropy
    }
}

pub struct DecisionTreeStrategy {
    tree: TreeNode,
    config: FeatureConfig,
    name: String,
}

impl DecisionTreeStrategy {
    pub fn new(candles: &[Candle], config: FeatureConfig, max_depth: usize, min_samples: usize) -> Self {
        let (features, labels, _start) = extract_features(candles, &config);
        let feature_refs: Vec<&[f64]> = features.iter().map(|f| f.as_slice()).collect();

        let tree = if feature_refs.is_empty() {
            TreeNode::Leaf(0)
        } else {
            TreeNode::build(&feature_refs, &labels, max_depth, min_samples)
        };

        let name = format!("Decision Tree (depth={})", max_depth);
        Self { tree, config, name }
    }
}

impl Strategy for DecisionTreeStrategy {
    fn name(&self) -> &str { &self.name }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];

        let (features, _labels, start_idx) = extract_features(candles, &self.config);
        for (i, feat) in features.iter().enumerate() {
            let idx = start_idx + i;
            if idx < n {
                let pred = self.tree.predict(feat);
                signals[idx] = if pred == 1 { Signal::Buy } else { Signal::Sell };
            }
        }
        signals
    }
}

// ========================================================================
// 在线线性回归 (SGD)
// ========================================================================

struct OnlineLinearRegression {
    weights: [f64; N_FEATURES],
    bias: f64,
}

impl OnlineLinearRegression {
    fn new() -> Self {
        Self { weights: [0.0; N_FEATURES], bias: 0.0 }
    }

    fn fit(&mut self, features: &FeatureMatrix, targets: &[f64], lr: f64, epochs: usize) {
        if features.is_empty() { return; }
        self.weights = [0.0; N_FEATURES];
        self.bias = 0.0;

        let n = features.len();
        let inv_n = 1.0 / n as f64;

        for _ in 0..epochs {
            let mut grad_w = [0.0f64; N_FEATURES];
            let mut grad_b = 0.0f64;

            for i in 0..n {
                let mut pred = self.bias;
                for j in 0..N_FEATURES { pred += self.weights[j] * features[i][j]; }
                let error = pred - targets[i];
                for j in 0..N_FEATURES { grad_w[j] += error * features[i][j]; }
                grad_b += error;
            }

            for j in 0..N_FEATURES { self.weights[j] -= lr * grad_w[j] * inv_n; }
            self.bias -= lr * grad_b * inv_n;
        }
    }

    fn predict(&self, x: &[f64]) -> f64 {
        let mut pred = self.bias;
        for j in 0..N_FEATURES { pred += self.weights[j] * x[j]; }
        pred
    }
}

pub struct LinearRegressionStrategy {
    model: OnlineLinearRegression,
    config: FeatureConfig,
    threshold: f64,
    name: String,
}

impl LinearRegressionStrategy {
    pub fn new(candles: &[Candle], config: FeatureConfig, threshold: f64) -> Self {
        let (features, _labels, start_idx) = extract_features(candles, &config);
        let n = candles.len();

        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let mut targets = Vec::with_capacity(features.len());
        for i in 0..features.len() {
            let idx = start_idx + i;
            let future_idx = (idx + config.lookback).min(n - 1);
            let ret = if closes[idx] == 0.0 { 0.0 } else { (closes[future_idx] - closes[idx]) / closes[idx] };
            targets.push(ret);
        }

        let mut model = OnlineLinearRegression::new();
        model.fit(&features, &targets, 0.01, 200);

        let name = format!("Linear Regression (threshold={:.4})", threshold);
        Self { model, config, threshold, name }
    }
}

impl Strategy for LinearRegressionStrategy {
    fn name(&self) -> &str { &self.name }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];

        let (features, _labels, start_idx) = extract_features(candles, &self.config);
        for (i, feat) in features.iter().enumerate() {
            let idx = start_idx + i;
            if idx < n {
                let pred = self.model.predict(feat);
                if pred > self.threshold { signals[idx] = Signal::Buy; }
                else if pred < -self.threshold { signals[idx] = Signal::Sell; }
            }
        }
        signals
    }
}

// ========================================================================
// KNN 策略
// ========================================================================

pub struct KnnStrategy {
    features: FeatureMatrix,
    labels: Vec<usize>,
    k: usize,
    config: FeatureConfig,
    #[allow(dead_code)]
    start_idx: usize,
    name: String,
}

impl KnnStrategy {
    pub fn new(candles: &[Candle], config: FeatureConfig, k: usize) -> Self {
        let (features, labels, start_idx) = extract_features(candles, &config);
        let name = format!("KNN (k={})", k);
        Self { features, labels, k, config, start_idx, name }
    }

    fn predict_single(&self, query: &[f64]) -> usize {
        if self.features.is_empty() { return 0; }

        // 计算所有距离，使用 partial sort
        let mut dists: Vec<(f64, usize)> = (0..self.features.len())
            .map(|i| {
                let mut dist = 0.0f64;
                for j in 0..N_FEATURES {
                    let diff = query[j] - self.features[i][j];
                    dist += diff * diff;
                }
                (dist, self.labels[i])
            })
            .collect();

        let k = self.k.min(dists.len());
        dists.select_nth_unstable_by(k - 1, |a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut votes_1 = 0;
        for i in 0..k { if dists[i].1 == 1 { votes_1 += 1; } }

        if votes_1 > k / 2 { 1 } else { 0 }
    }
}

impl Strategy for KnnStrategy {
    fn name(&self) -> &str { &self.name }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];

        let (features, _labels, start_idx) = extract_features(candles, &self.config);
        if features.is_empty() { return signals; }

        // KNN O(n²) 计算量大，只预测最近 300 个点
        let max_predict = features.len().min(300);
        let offset = features.len().saturating_sub(max_predict);

        for i in offset..features.len() {
            let idx = start_idx + i;
            if idx < n {
                let pred = self.predict_single(&features[i]);
                signals[idx] = if pred == 1 { Signal::Buy } else { Signal::Sell };
            }
        }
        signals
    }
}

// ========================================================================
// ML 组合策略
// ========================================================================

pub struct MlEnsembleStrategy {
    tree: DecisionTreeStrategy,
    lr: LinearRegressionStrategy,
    knn: KnnStrategy,
    name: String,
}

impl MlEnsembleStrategy {
    pub fn new(candles: &[Candle], config: FeatureConfig) -> Self {
        let tree = DecisionTreeStrategy::new(candles, config.clone(), 5, 10);
        let lr = LinearRegressionStrategy::new(candles, config.clone(), 0.001);
        let knn = KnnStrategy::new(candles, config.clone(), 7);
        let name = "ML Ensemble (Tree + LR + KNN)".to_string();
        Self { tree, lr, knn, name }
    }
}

impl Strategy for MlEnsembleStrategy {
    fn name(&self) -> &str { &self.name }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let tree_signals = self.tree.generate(candles);
        let lr_signals = self.lr.generate(candles);
        let knn_signals = self.knn.generate(candles);

        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];

        for i in 0..n {
            let mut buy_votes = 0;
            let mut sell_votes = 0;
            for s in [&tree_signals, &lr_signals, &knn_signals] {
                match s[i] {
                    Signal::Buy => buy_votes += 1,
                    Signal::Sell => sell_votes += 1,
                    _ => {}
                }
            }
            if buy_votes >= 2 { signals[i] = Signal::Buy; }
            else if sell_votes >= 2 { signals[i] = Signal::Sell; }
        }
        signals
    }
}

// ========================================================================
// 创建 ML 策略
// ========================================================================

pub fn create_ml_strategies(candles: &[Candle]) -> Vec<Box<dyn Strategy>> {
    let config = FeatureConfig::default();
    vec![
        Box::new(DecisionTreeStrategy::new(candles, config.clone(), 5, 10)),
        Box::new(LinearRegressionStrategy::new(candles, config.clone(), 0.001)),
        Box::new(KnnStrategy::new(candles, config.clone(), 7)),
        Box::new(MlEnsembleStrategy::new(candles, config)),
    ]
}

// ========================================================================
// 测试
// ========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading::data::DataSource;

    fn create_test_candles() -> Vec<Candle> {
        DataSource::generate_mock(500, 100.0)
    }

    #[test]
    fn test_feature_extraction() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let (features, labels, start) = extract_features(&candles, &config);
        assert!(!features.is_empty());
        assert_eq!(features.len(), labels.len());
        assert!(start > 0);
    }

    #[test]
    fn test_feature_ranges() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let (features, _labels, _start) = extract_features(&candles, &config);
        for f in &features {
            assert!(f[0] >= 0.0 && f[0] <= 1.0); // RSI norm
            assert!(f[4] >= -0.1 && f[4] <= 1.1); // BB pos
        }
    }

    #[test]
    fn test_decision_tree() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let strategy = DecisionTreeStrategy::new(&candles, config, 5, 10);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_linear_regression() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let strategy = LinearRegressionStrategy::new(&candles, config, 0.001);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_knn() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let strategy = KnnStrategy::new(&candles, config, 7);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_ml_ensemble() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let strategy = MlEnsembleStrategy::new(&candles, config);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_create_ml_strategies() {
        let candles = create_test_candles();
        let strategies = create_ml_strategies(&candles);
        assert_eq!(strategies.len(), 4);
        for s in &strategies {
            let signals = s.generate(&candles);
            assert_eq!(signals.len(), candles.len());
        }
    }
}
