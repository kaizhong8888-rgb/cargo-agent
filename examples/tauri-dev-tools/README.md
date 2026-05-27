# Dev Tools - Tauri 2 Desktop App

一个功能完整的 **Tauri 2.x** 桌面应用示例，展示 Rust 后端与 Web 前端的深度集成。

## ✨ 功能

| 功能 | 描述 |
|------|------|
| 📊 **系统仪表盘** | 显示 OS、CPU、内存、Rust 版本等系统信息 |
| 📝 **笔记管理** | 创建/编辑/删除/搜索笔记，自动持久化到本地文件系统 |
| 📁 **文件浏览** | 浏览本地目录、查看文本文件内容 |

## 🏗️ 技术栈

- **后端**: Rust + Tauri 2.x
- **前端**: 原生 HTML + CSS + JavaScript（零依赖）
- **数据**: JSON 文件持久化（`~/.local/share/dev-notes/`）

## 🚀 运行

### 前提条件

- Rust 1.75+
- Node.js（仅用于开发工具，前端无构建步骤）
- 系统库依赖：参照 [Tauri 文档](https://v2.tauri.app/start/prerequisites/)

### 启动

```bash
cd examples/tauri-dev-tools
cargo tauri dev
```

### 构建

```bash
cargo tauri build
```

## 🧠 架构设计

```
┌─────────────────────────────────────────────┐
│                 Frontend                     │
│  HTML + CSS + JS (vanilla)                   │
│  window.__TAURI__.invoke()                   │
└─────────────────┬───────────────────────────┘
                  │ JSON (serde)
                  ▼
┌─────────────────────────────────────────────┐
│              Rust Backend                    │
│                                              │
│  #[tauri::command] fn get_system_info()      │
│  #[tauri::command] fn create_note()          │
│  #[tauri::command] fn list_directory()       │
│  AppState (Mutex<Vec<Note>>)                 │
└─────────────────────────────────────────────┘
```

## 📁 项目结构

```
tauri-dev-tools/
├── src/                    # 前端代码
│   ├── index.html          # 主页面
│   ├── style.css           # 深色主题样式
│   └── main.js             # 前端逻辑 + Tauri 调用
├── src-tauri/              # Rust 后端
│   ├── Cargo.toml          # 依赖配置
│   ├── tauri.conf.json     # Tauri 配置
│   ├── build.rs            # 构建脚本
│   ├── capabilities/       # 权限配置
│   └── src/
│       ├── main.rs         # 入口
│       └── lib.rs          # 全部命令和状态管理
└── README.md
```

## 🎯 学习要点

这个示例展示了 Tauri 2.x 的核心模式：

1. **`#[tauri::command]`** - 定义可被前端调用的 Rust 函数
2. **`State<AppState>`** - 管理全局应用状态
3. **`invoke_handler!`** - 注册所有命令
4. **`serde` 序列化** - Rust ↔ JS 数据类型自动转换
5. **文件 I/O** - Rust 后端处理文件系统操作
6. **事件驱动** - 前端调用 → 后端处理 → 返回结果
