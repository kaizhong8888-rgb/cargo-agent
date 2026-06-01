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

/// 简单伪随机数生成器 (LCG) — 用于 Bootstrap 采样
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    fn next(&mut self) -> usize {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.state >> 33) as usize
    }
}

// ========================================================================
// Random Forest (多棵决策树 + Bootstrap 采样)
// ========================================================================

pub struct RandomForestStrategy {
    trees: Vec<TreeNode>,
    config: FeatureConfig,
    #[allow(dead_code)]
    n_trees: usize,
    name: String,
}

impl RandomForestStrategy {
    pub fn new(candles: &[Candle], config: FeatureConfig, n_trees: usize, max_depth: usize, min_samples: usize) -> Self {
        let (features, labels, _start) = extract_features(candles, &config);
        let _refs: Vec<&[f64]> = features.iter().map(|f| f.as_slice()).collect();
        let n = features.len();

        let mut trees = Vec::with_capacity(n_trees);

        if n == 0 {
            trees.push(TreeNode::Leaf(0));
        } else {
            for t in 0..n_trees {
                let mut rng = SimpleRng::new(t as u64 + 42);
                let (boot_features, boot_labels) = Self::bootstrap_sample(&features, &labels, &mut rng);
                let boot_refs: Vec<&[f64]> = boot_features.iter().map(|f| f.as_slice()).collect();

                if boot_refs.is_empty() {
                    trees.push(TreeNode::Leaf(0));
                } else {
                    trees.push(TreeNode::build(&boot_refs, &boot_labels, max_depth, min_samples));
                }
            }
        }

        let name = format!("Random Forest (trees={}, depth={})", n_trees, max_depth);
        Self { trees, config, n_trees, name }
    }

    fn bootstrap_sample(features: &FeatureMatrix, labels: &[usize], rng: &mut SimpleRng)
        -> (Vec<[f64; N_FEATURES]>, Vec<usize>)
    {
        let n = features.len();
        let mut bf = Vec::with_capacity(n);
        let mut bl = Vec::with_capacity(n);
        for _ in 0..n {
            let idx = rng.next() % n;
            bf.push(features[idx]);
            bl.push(labels[idx]);
        }
        (bf, bl)
    }
}

impl Strategy for RandomForestStrategy {
    fn name(&self) -> &str { &self.name }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];

        let (features, _labels, start_idx) = extract_features(candles, &self.config);
        for (i, feat) in features.iter().enumerate() {
            let idx = start_idx + i;
            if idx < n {
                let mut votes_1 = 0;
                for tree in &self.trees {
                    if tree.predict(feat) == 1 { votes_1 += 1; }
                }
                signals[idx] = if votes_1 > self.trees.len() / 2 { Signal::Buy } else { Signal::Sell };
            }
        }
        signals
    }
}

// ========================================================================
// Gaussian Naive Bayes (朴素贝叶斯分类器)
// ========================================================================

struct GaussianNB {
    class_priors: [f64; 2],
    means: [[f64; N_FEATURES]; 2],
    variances: [[f64; N_FEATURES]; 2],
}

impl GaussianNB {
    fn new() -> Self {
        Self {
            class_priors: [0.5, 0.5],
            means: [[0.0; N_FEATURES]; 2],
            variances: [[1.0; N_FEATURES]; 2],
        }
    }

    fn fit(&mut self, features: &FeatureMatrix, labels: &[usize]) {
        let n = features.len();
        if n == 0 { return; }

        let mut class_counts = [0usize; 2];
        for &l in labels { class_counts[l] += 1; }

        for c in 0..2 {
            self.class_priors[c] = if n > 0 { class_counts[c] as f64 / n as f64 } else { 0.5 };
        }

        for c in 0..2 {
            if class_counts[c] == 0 { continue; }
            for j in 0..N_FEATURES {
                let mut sum = 0.0;
                let mut count = 0;
                for i in 0..n {
                    if labels[i] == c { sum += features[i][j]; count += 1; }
                }
                self.means[c][j] = if count > 0 { sum / count as f64 } else { 0.0 };
            }
            for j in 0..N_FEATURES {
                let mut sum_sq = 0.0;
                let mut count = 0;
                for i in 0..n {
                    if labels[i] == c {
                        let diff = features[i][j] - self.means[c][j];
                        sum_sq += diff * diff;
                        count += 1;
                    }
                }
                self.variances[c][j] = if count > 0 { sum_sq / count as f64 + 1e-6 } else { 1.0 };
            }
        }
    }

    fn predict(&self, x: &[f64]) -> usize {
        let mut log_probs = [0.0f64; 2];
        for c in 0..2 {
            log_probs[c] = self.class_priors[c].max(1e-10).ln();
            for j in 0..N_FEATURES {
                let diff = x[j] - self.means[c][j];
                let var = self.variances[c][j];
                log_probs[c] -= 0.5 * (2.0 * std::f64::consts::PI * var).ln() + diff * diff / (2.0 * var);
            }
        }
        if log_probs[0] >= log_probs[1] { 0 } else { 1 }
    }
}

pub struct NaiveBayesStrategy {
    model: GaussianNB,
    config: FeatureConfig,
    name: String,
}

impl NaiveBayesStrategy {
    pub fn new(candles: &[Candle], config: FeatureConfig) -> Self {
        let (features, labels, _start) = extract_features(candles, &config);
        let mut model = GaussianNB::new();
        model.fit(&features, &labels);
        Self { model, config, name: "Naive Bayes".to_string() }
    }
}

impl Strategy for NaiveBayesStrategy {
    fn name(&self) -> &str { &self.name }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];
        let (features, _labels, start_idx) = extract_features(candles, &self.config);
        for (i, feat) in features.iter().enumerate() {
            let idx = start_idx + i;
            if idx < n {
                let pred = self.model.predict(feat);
                signals[idx] = if pred == 1 { Signal::Buy } else { Signal::Sell };
            }
        }
        signals
    }
}

// ========================================================================
// Gradient Boosting (梯度提升决策树)
// ========================================================================

struct ShallowTree {
    feature_idx: usize,
    threshold: f64,
    left_value: f64,
    right_value: f64,
}

impl ShallowTree {
    fn predict(&self, x: &[f64]) -> f64 {
        if x[self.feature_idx] <= self.threshold { self.left_value } else { self.right_value }
    }

    fn fit(features: &[&[f64]], residuals: &[f64], max_leaves: usize) -> Self {
        let n = features.len();
        if n == 0 {
            return Self { feature_idx: 0, threshold: 0.0, left_value: 0.0, right_value: 0.0 };
        }

        let global_mean: f64 = residuals.iter().sum::<f64>() / n as f64;
        let max_feat = max_leaves.min(N_FEATURES);
        let total_var: f64 = residuals.iter().map(|r| { let d = r - global_mean; d * d }).sum::<f64>();

        let mut best_gain = 0.0f64;
        let mut best_feat = 0;
        let mut best_thresh = 0.0;
        let mut best_left_val = global_mean;
        let mut best_right_val = global_mean;

        for feat_idx in 0..max_feat {
            let mut vals: Vec<(f64, usize)> = features.iter()
                .enumerate()
                .map(|(i, f)| (f[feat_idx], i))
                .collect();
            vals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            let step = vals.len().max(10) / 20;
            let mut left_sum = 0.0f64;
            let mut left_count = 0usize;
            for (pos, (_, idx)) in vals.iter().enumerate() {
                left_sum += residuals[*idx];
                left_count += 1;

                if (pos + 1) % step != 0 && pos + 1 < vals.len() { continue; }

                let left_mean = left_sum / left_count as f64;
                let right_sum: f64 = (left_count..n).map(|k| residuals[vals[k].1]).sum();
                let right_count = n - left_count;
                if right_count == 0 { continue; }
                let right_mean = right_sum / right_count as f64;

                let left_var: f64 = (0..left_count).map(|k| { let d = residuals[vals[k].1] - left_mean; d * d }).sum::<f64>();
                let right_var: f64 = (left_count..n).map(|k| { let d = residuals[vals[k].1] - right_mean; d * d }).sum::<f64>();
                let gain = total_var - left_var - right_var;

                if gain > best_gain {
                    best_gain = gain;
                    best_feat = feat_idx;
                    best_thresh = vals[pos].0;
                    best_left_val = left_mean;
                    best_right_val = right_mean;
                }
            }
        }

        ShallowTree {
            feature_idx: best_feat,
            threshold: best_thresh,
            left_value: best_left_val,
            right_value: best_right_val,
        }
    }
}

pub struct GradientBoostingStrategy {
    trees: Vec<ShallowTree>,
    learning_rate: f64,
    initial_pred: f64,
    config: FeatureConfig,
    name: String,
}

impl GradientBoostingStrategy {
    pub fn new(candles: &[Candle], config: FeatureConfig, n_estimators: usize, learning_rate: f64, max_leaves: usize) -> Self {
        let (features, labels, _start) = extract_features(candles, &config);
        let n = features.len();

        if n == 0 {
            return Self {
                trees: Vec::new(), learning_rate, initial_pred: 0.0,
                config, name: "Gradient Boosting".to_string(),
            };
        }

        let label_mean: f64 = labels.iter().map(|l| *l as f64).sum::<f64>() / n as f64;
        let initial_pred = label_mean - 0.5;

        let mut preds: Vec<f64> = vec![initial_pred; n];
        let mut trees = Vec::with_capacity(n_estimators);

        for _ in 0..n_estimators {
            let residuals: Vec<f64> = labels.iter()
                .zip(preds.iter())
                .map(|(l, p)| (*l as f64 - 0.5) - p)
                .collect();

            let feature_refs: Vec<&[f64]> = features.iter().map(|f| f.as_slice()).collect();
            let tree = ShallowTree::fit(&feature_refs, &residuals, max_leaves);
            // update predictions before moving tree
            for i in 0..n {
                preds[i] += learning_rate * tree.predict(&features[i]);
            }
            trees.push(tree);
        }

        let name = format!("Gradient Boosting (trees={}, lr={})", n_estimators, learning_rate);
        Self { trees, learning_rate, initial_pred, config, name }
    }
}

impl Strategy for GradientBoostingStrategy {
    fn name(&self) -> &str { &self.name }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];
        let (features, _labels, start_idx) = extract_features(candles, &self.config);
        for (i, feat) in features.iter().enumerate() {
            let idx = start_idx + i;
            if idx < n {
                let mut pred = self.initial_pred;
                for tree in &self.trees {
                    pred += self.learning_rate * tree.predict(feat);
                }
                signals[idx] = if pred > 0.0 { Signal::Buy } else { Signal::Sell };
            }
        }
        signals
    }
}

// ========================================================================
// Logistic Regression (逻辑回归 + SGD)
// ========================================================================

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp().clamp(1e-10, 1e10))
}

struct LogisticRegressionModel {
    weights: [f64; N_FEATURES],
    bias: f64,
}

impl LogisticRegressionModel {
    fn new() -> Self {
        Self { weights: [0.0; N_FEATURES], bias: 0.0 }
    }

    fn fit(&mut self, features: &FeatureMatrix, labels: &[usize], lr: f64, epochs: usize, reg: f64) {
        if features.is_empty() { return; }
        self.weights = [0.0; N_FEATURES];
        self.bias = 0.0;
        let n = features.len();
        let inv_n = 1.0 / n as f64;

        for _epoch in 0..epochs {
            let mut grad_w = [0.0f64; N_FEATURES];
            let mut grad_b = 0.0f64;

            for i in 0..n {
                let mut logit = self.bias;
                for j in 0..N_FEATURES { logit += self.weights[j] * features[i][j]; }
                let pred = sigmoid(logit);
                let error = pred - labels[i] as f64;
                for j in 0..N_FEATURES { grad_w[j] += error * features[i][j] + reg * self.weights[j]; }
                grad_b += error;
            }

            for j in 0..N_FEATURES { self.weights[j] -= lr * grad_w[j] * inv_n; }
            self.bias -= lr * grad_b * inv_n;
        }
    }

    fn predict_proba(&self, x: &[f64]) -> f64 {
        let mut logit = self.bias;
        for j in 0..N_FEATURES { logit += self.weights[j] * x[j]; }
        sigmoid(logit)
    }
}

pub struct LogisticRegressionStrategy {
    model: LogisticRegressionModel,
    config: FeatureConfig,
    threshold: f64,
    name: String,
}

impl LogisticRegressionStrategy {
    pub fn new(candles: &[Candle], config: FeatureConfig, lr: f64, epochs: usize, reg: f64, threshold: f64) -> Self {
        let (features, labels, _start) = extract_features(candles, &config);
        let mut model = LogisticRegressionModel::new();
        model.fit(&features, &labels, lr, epochs, reg);
        let name = format!("Logistic Regression (lr={}, epochs={}, reg={})", lr, epochs, reg);
        Self { model, config, threshold, name }
    }
}

impl Strategy for LogisticRegressionStrategy {
    fn name(&self) -> &str { &self.name }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];
        let (features, _labels, start_idx) = extract_features(candles, &self.config);
        for (i, feat) in features.iter().enumerate() {
            let idx = start_idx + i;
            if idx < n {
                let prob = self.model.predict_proba(feat);
                if prob > self.threshold { signals[idx] = Signal::Buy; }
                else { signals[idx] = Signal::Sell; }
            }
        }
        signals
    }
}

// ========================================================================
// Linear SVM (线性支持向量机 + Hinge loss + SGD)
// ========================================================================

struct LinearSVM {
    weights: [f64; N_FEATURES],
    bias: f64,
}

impl LinearSVM {
    fn new() -> Self {
        Self { weights: [0.0; N_FEATURES], bias: 0.0 }
    }

    fn fit(&mut self, features: &FeatureMatrix, labels: &[usize], lr: f64, epochs: usize, c_param: f64) {
        if features.is_empty() { return; }
        self.weights = [0.0; N_FEATURES];
        self.bias = 0.0;
        let n = features.len();
        let inv_n = 1.0 / n as f64;

        // 转换标签 0→-1, 1→+1
        let ys: Vec<f64> = labels.iter().map(|l| if *l == 1 { 1.0 } else { -1.0 }).collect();

        for _epoch in 0..epochs {
            let mut grad_w = [0.0f64; N_FEATURES];
            let mut grad_b = 0.0f64;

            for i in 0..n {
                let mut score = self.bias;
                for j in 0..N_FEATURES { score += self.weights[j] * features[i][j]; }
                let margin = ys[i] * score;

                if margin < 1.0 {
                    // Hinge loss 激活区域
                    for j in 0..N_FEATURES {
                        grad_w[j] += -ys[i] * features[i][j] + 2.0 * c_param * self.weights[j];
                    }
                    grad_b += -ys[i];
                } else {
                    // 仅正则化
                    for j in 0..N_FEATURES { grad_w[j] += 2.0 * c_param * self.weights[j]; }
                }
            }

            for j in 0..N_FEATURES { self.weights[j] -= lr * grad_w[j] * inv_n; }
            self.bias -= lr * grad_b * inv_n;
        }
    }

    fn predict(&self, x: &[f64]) -> usize {
        let mut score = self.bias;
        for j in 0..N_FEATURES { score += self.weights[j] * x[j]; }
        if score > 0.0 { 1 } else { 0 }
    }
}

pub struct LinearSvmStrategy {
    model: LinearSVM,
    config: FeatureConfig,
    name: String,
}

impl LinearSvmStrategy {
    pub fn new(candles: &[Candle], config: FeatureConfig, lr: f64, epochs: usize, c_param: f64) -> Self {
        let (features, labels, _start) = extract_features(candles, &config);
        let mut model = LinearSVM::new();
        model.fit(&features, &labels, lr, epochs, c_param);
        let name = format!("Linear SVM (lr={}, epochs={}, C={})", lr, epochs, c_param);
        Self { model, config, name }
    }
}

impl Strategy for LinearSvmStrategy {
    fn name(&self) -> &str { &self.name }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];
        let (features, _labels, start_idx) = extract_features(candles, &self.config);
        for (i, feat) in features.iter().enumerate() {
            let idx = start_idx + i;
            if idx < n {
                let pred = self.model.predict(feat);
                signals[idx] = if pred == 1 { Signal::Buy } else { Signal::Sell };
            }
        }
        signals
    }
}

// ========================================================================
// Perceptron (感知机 — 单层神经网络)
// ========================================================================

struct PerceptronModel {
    weights: [f64; N_FEATURES],
    bias: f64,
}

impl PerceptronModel {
    fn new() -> Self {
        Self { weights: [0.0; N_FEATURES], bias: 0.0 }
    }

    fn fit(&mut self, features: &FeatureMatrix, labels: &[usize], lr: f64, epochs: usize) {
        if features.is_empty() { return; }
        // 初始化小随机权重
        let mut seed: u64 = 42;
        let mut next_rand = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as f64 / 4294967296.0) * 0.1
        };
        for j in 0..N_FEATURES { self.weights[j] = next_rand(); }
        self.bias = 0.0;
        let n = features.len();

        for _epoch in 0..epochs {
            for i in 0..n {
                let mut score = self.bias;
                for j in 0..N_FEATURES { score += self.weights[j] * features[i][j]; }
                let pred = if score >= 0.0 { 1 } else { 0 };
                let error = labels[i] as isize - pred as isize;

                if error != 0 {
                    for j in 0..N_FEATURES {
                        self.weights[j] += lr * error as f64 * features[i][j];
                    }
                    self.bias += lr * error as f64;
                }
            }
        }
    }

    fn predict(&self, x: &[f64]) -> usize {
        let mut score = self.bias;
        for j in 0..N_FEATURES { score += self.weights[j] * x[j]; }
        if score >= 0.0 { 1 } else { 0 }
    }
}

pub struct PerceptronStrategy {
    model: PerceptronModel,
    config: FeatureConfig,
    name: String,
}

impl PerceptronStrategy {
    pub fn new(candles: &[Candle], config: FeatureConfig, lr: f64, epochs: usize) -> Self {
        let (features, labels, _start) = extract_features(candles, &config);
        let mut model = PerceptronModel::new();
        model.fit(&features, &labels, lr, epochs);
        Self { model, config, name: "Perceptron".to_string() }
    }
}

impl Strategy for PerceptronStrategy {
    fn name(&self) -> &str { &self.name }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];
        let (features, _labels, start_idx) = extract_features(candles, &self.config);
        for (i, feat) in features.iter().enumerate() {
            let idx = start_idx + i;
            if idx < n {
                let pred = self.model.predict(feat);
                signals[idx] = if pred == 1 { Signal::Buy } else { Signal::Sell };
            }
        }
        signals
    }
}

// ========================================================================
// MLP (2层多层感知机)
// ========================================================================

struct MlpModel {
    hidden_size: usize,
    w1: Vec<[f64; N_FEATURES]>, // hidden x features
    b1: Vec<f64>,
    w2: Vec<f64>, // hidden
    b2: f64,
}

impl MlpModel {
    fn new(hidden_size: usize) -> Self {
        let mut seed: u64 = 123;
        let mut next_rand = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as f64 / 4294967296.0) * 0.5
        };
        let mut w1 = vec![[0.0; N_FEATURES]; hidden_size];
        let mut b1 = vec![0.0; hidden_size];
        let mut w2 = vec![0.0; hidden_size];
        for h in 0..hidden_size {
            for j in 0..N_FEATURES { w1[h][j] = next_rand(); }
            b1[h] = 0.0;
            w2[h] = next_rand();
        }
        Self { hidden_size, w1, b1, w2, b2: 0.0 }
    }

    fn relu(x: f64) -> f64 { if x > 0.0 { x } else { 0.0 } }
    fn relu_deriv(x: f64) -> f64 { if x > 0.0 { 1.0 } else { 0.0 } }

    fn fit(&mut self, features: &FeatureMatrix, labels: &[usize], lr: f64, epochs: usize) {
        if features.is_empty() { return; }
        let n = features.len();
        let h = self.hidden_size;

        for _epoch in 0..epochs {
            for i in 0..n {
                // Forward pass
                let mut hidden = vec![0.0; h];
                let mut hidden_pre = vec![0.0; h];
                for hh in 0..h {
                    let mut s = self.b1[hh];
                    for j in 0..N_FEATURES { s += self.w1[hh][j] * features[i][j]; }
                    hidden_pre[hh] = s;
                    hidden[hh] = Self::relu(s);
                }
                let mut out = self.b2;
                for hh in 0..h { out += self.w2[hh] * hidden[hh]; }
                let pred = sigmoid(out);
                let target = labels[i] as f64;
                let error = pred - target; // d_loss/d_out

                // Backward pass
                let d_out = error * pred * (1.0 - pred); // sigmoid derivative

                let mut dw2 = vec![0.0; h];
                let db2 = d_out;
                for hh in 0..h {
                    dw2[hh] = d_out * hidden[hh];
                }

                // Backprop to hidden layer
                let mut dw1 = vec![[0.0; N_FEATURES]; h];
                let mut db1 = vec![0.0; h];
                for hh in 0..h {
                    let d_hidden = d_out * self.w2[hh] * Self::relu_deriv(hidden_pre[hh]);
                    db1[hh] = d_hidden;
                    for j in 0..N_FEATURES {
                        dw1[hh][j] = d_hidden * features[i][j];
                    }
                }

                // Update weights
                for hh in 0..h {
                    for j in 0..N_FEATURES { self.w1[hh][j] -= lr * dw1[hh][j] / n as f64; }
                    self.b1[hh] -= lr * db1[hh] / n as f64;
                    self.w2[hh] -= lr * dw2[hh] / n as f64;
                }
                self.b2 -= lr * db2 / n as f64;
            }
        }
    }

    fn predict_proba(&self, x: &[f64]) -> f64 {
        let h = self.hidden_size;
        let mut hidden = vec![0.0; h];
        for hh in 0..h {
            let mut s = self.b1[hh];
            for j in 0..N_FEATURES { s += self.w1[hh][j] * x[j]; }
            hidden[hh] = Self::relu(s);
        }
        let mut out = self.b2;
        for hh in 0..h { out += self.w2[hh] * hidden[hh]; }
        sigmoid(out)
    }
}

pub struct MlpStrategy {
    model: MlpModel,
    config: FeatureConfig,
    threshold: f64,
    name: String,
}

impl MlpStrategy {
    pub fn new(candles: &[Candle], config: FeatureConfig, hidden_size: usize, lr: f64, epochs: usize, threshold: f64) -> Self {
        let (features, labels, _start) = extract_features(candles, &config);
        let mut model = MlpModel::new(hidden_size);
        model.fit(&features, &labels, lr, epochs);
        let name = format!("MLP (hidden={}, lr={}, epochs={})", hidden_size, lr, epochs);
        Self { model, config, threshold, name }
    }
}

impl Strategy for MlpStrategy {
    fn name(&self) -> &str { &self.name }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let n = candles.len();
        let mut signals = vec![Signal::Hold; n];
        let (features, _labels, start_idx) = extract_features(candles, &self.config);
        for (i, feat) in features.iter().enumerate() {
            let idx = start_idx + i;
            if idx < n {
                let prob = self.model.predict_proba(feat);
                if prob > self.threshold { signals[idx] = Signal::Buy; }
                else { signals[idx] = Signal::Sell; }
            }
        }
        signals
    }
}

// ========================================================================
// 全模型 ML Ensemble (10个模型投票)
// ========================================================================

pub struct MlFullEnsembleStrategy {
    tree: DecisionTreeStrategy,
    rf: RandomForestStrategy,
    lr: LinearRegressionStrategy,
    logreg: LogisticRegressionStrategy,
    svm: LinearSvmStrategy,
    perceptron: PerceptronStrategy,
    mlp: MlpStrategy,
    nb: NaiveBayesStrategy,
    gb: GradientBoostingStrategy,
    knn: KnnStrategy,
    name: String,
}

impl MlFullEnsembleStrategy {
    pub fn new(candles: &[Candle], config: FeatureConfig) -> Self {
        let tree = DecisionTreeStrategy::new(candles, config.clone(), 5, 10);
        let rf = RandomForestStrategy::new(candles, config.clone(), 10, 4, 10);
        let lr = LinearRegressionStrategy::new(candles, config.clone(), 0.001);
        let logreg = LogisticRegressionStrategy::new(candles, config.clone(), 0.05, 200, 0.01, 0.5);
        let svm = LinearSvmStrategy::new(candles, config.clone(), 0.01, 200, 0.1);
        let perceptron = PerceptronStrategy::new(candles, config.clone(), 0.1, 100);
        let mlp = MlpStrategy::new(candles, config.clone(), 16, 0.05, 100, 0.5);
        let nb = NaiveBayesStrategy::new(candles, config.clone());
        let gb = GradientBoostingStrategy::new(candles, config.clone(), 20, 0.1, 8);
        let knn = KnnStrategy::new(candles, config.clone(), 7);
        let name = "ML Full Ensemble (10 models)".to_string();
        Self { tree, rf, lr, logreg, svm, perceptron, mlp, nb, gb, knn, name }
    }
}

impl Strategy for MlFullEnsembleStrategy {
    fn name(&self) -> &str { &self.name }

    fn generate(&self, candles: &[Candle]) -> Vec<Signal> {
        let all_signals: Vec<Vec<Signal>> = vec![
            self.tree.generate(candles),
            self.rf.generate(candles),
            self.lr.generate(candles),
            self.logreg.generate(candles),
            self.svm.generate(candles),
            self.perceptron.generate(candles),
            self.mlp.generate(candles),
            self.nb.generate(candles),
            self.gb.generate(candles),
            self.knn.generate(candles),
        ];

        let n = candles.len();
        let threshold = (all_signals.len() as f64 / 2.0).ceil() as usize;
        let mut signals = vec![Signal::Hold; n];

        for i in 0..n {
            let mut buy_votes = 0;
            let mut sell_votes = 0;
            for s in &all_signals {
                if i < s.len() {
                    match s[i] {
                        Signal::Buy => buy_votes += 1,
                        Signal::Sell => sell_votes += 1,
                        _ => {}
                    }
                }
            }
            if buy_votes >= threshold && buy_votes > sell_votes {
                signals[i] = Signal::Buy;
            } else if sell_votes >= threshold && sell_votes > buy_votes {
                signals[i] = Signal::Sell;
            }
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

    #[test]
    fn test_random_forest() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let strategy = RandomForestStrategy::new(&candles, config, 5, 4, 10);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_naive_bayes() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let strategy = NaiveBayesStrategy::new(&candles, config);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_gradient_boosting() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let strategy = GradientBoostingStrategy::new(&candles, config, 10, 0.1, 8);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_ml_full_ensemble() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let strategy = MlFullEnsembleStrategy::new(&candles, config);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_logistic_regression() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let strategy = LogisticRegressionStrategy::new(&candles, config, 0.05, 200, 0.01, 0.5);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_linear_svm() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let strategy = LinearSvmStrategy::new(&candles, config, 0.01, 200, 0.1);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_perceptron() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let strategy = PerceptronStrategy::new(&candles, config, 0.1, 100);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }

    #[test]
    fn test_mlp() {
        let candles = create_test_candles();
        let config = FeatureConfig::default();
        let strategy = MlpStrategy::new(&candles, config, 16, 0.05, 100, 0.5);
        let signals = strategy.generate(&candles);
        assert_eq!(signals.len(), candles.len());
    }
}
