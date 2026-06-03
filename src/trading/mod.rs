#![allow(clippy::needless_range_loop)]
#![allow(clippy::type_complexity)]

pub mod backtest;
pub mod data;
pub mod enhanced_backtest;
pub mod factor_model;
pub mod factor_model_ext;
pub mod feature_engineering;
pub mod fundamental;
pub mod fundamental_processing;
pub mod fundamental_strategy;
pub mod indicators;
pub mod market_regime;
pub mod ml;
pub mod optimizer;
pub mod param_optimizer;
pub mod portfolio_optimizer;
pub mod report;
pub mod risk_management;
pub mod strategy;
pub mod strategy_router;
pub mod tencent;
pub mod walk_forward;
