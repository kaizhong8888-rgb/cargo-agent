---
name: cargo-agent-trading-module
description: Quantitative trading module with 26 submodules: backtesting, technical indicators, ML, factor models, portfolio optimization, risk management, and strategy routing.
metadata:
  type: project
---

## 量化交易模块

**路径**：`src/trading/`
**子模块数**：26

### 核心模块

| 模块 | 用途 |
|------|------|
| `backtest.rs` | 回测引擎 |
| `enhanced_backtest.rs` | 增强回测 |
| `walk_forward.rs` | 前向验证 |
| `data.rs` | 市场数据处理 |
| `indicators.rs` | 技术指标（MA、RSI、MACD 等） |
| `strategy.rs` | 交易策略基类 |
| `strategy_router.rs` | 策略路由 |
| `strategy_comparison.rs` | 策略对比 |

### 因子模型

| 模块 | 用途 |
|------|------|
| `factor_model.rs` | 因子模型 |
| `factor_model_ext.rs` | 因子模型扩展 |
| `feature_engineering.rs` | 特征工程 |
| `fundamental.rs` | 基本面数据 |
| `fundamental_processing.rs` | 基本面处理 |
| `fundamental_strategy.rs` | 基本面策略 |

### 优化和风险管理

| 模块 | 用途 |
|------|------|
| `optimizer.rs` | 参数优化 |
| `param_optimizer.rs` | 参数优化器 |
| `portfolio_optimizer.rs` | 投资组合优化 |
| `risk_management.rs` | 风险管理 |
| `market_regime.rs` | 市场状态检测 |

### 高级策略

| 模块 | 用途 |
|------|------|
| `ml.rs` | 机器学习模型 |
| `seasonal_strategy.rs` | 季节性策略 |
| `statistical_arbitrage.rs` | 统计套利 |
| `report.rs` | 报告生成 |
| `tencent.rs` | 腾讯数据源 |

**为什么**：这是 cargo-agent 的一个独特功能——内置量化交易能力。通过 `quantitative_trading` 工具可以访问这些功能。
