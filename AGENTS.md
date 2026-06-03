# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## 项目概述

PicLayout 是一个 Python Tkinter GUI 工具，用于将多张图片拼接成正方形拼贴图，支持重采样、边框添加、水印叠加等功能。

## 运行方式

### Python 原版（保留）
```bash
python PicsLayout_V8.py
```
依赖：`Pillow`（PIL）、Python 标准库（tkinter、concurrent.futures、threading）

`PicsLayout_V4.py` 是旧版本，保留备用，**不要修改**。

### Rust + Electron 新版（开发模式）
```bash
# 方式 1：一键启动（需先安装 Rust 和 Node.js）
bash scripts/dev.sh

# 方式 2：分步启动
cd rust-core && cargo build          # 编译 Rust debug 版
cd electron-app && npm install && npm run dev   # 启动 Electron + Vite HMR
```

### 构建发布包
```bash
bash scripts/build.sh
# 等价于：cargo build --release → npm install → npm run electron:build
# 输出：dist-electron/PicLayout Setup *.exe
```

### 单独测试 Rust 核心
```bash
cd rust-core
cargo test                        # 单元测试
echo '{"image_paths":["a.jpg","b.jpg"],"output_dir":".","prefix":"test"}' | cargo run --release
```

Rust release 构建配置（Cargo.toml）：`opt-level=3`、`lto=true`、`codegen-units=1`、`strip=true`。debug 构建用于开发，release 用于生产。

---

## 架构说明

### Python 原版（PicsLayout_V8.py）

单文件架构，分两部分：
1. **处理函数**（顶部）：图像处理逻辑，与 GUI 强耦合（直接访问 `dpi_var.get()` 等全局变量）
2. **GUI 代码**（底部 ~600 行起）：Tkinter 窗口、控件、全局变量

### Rust + Electron 新版

```
rust-core/          Rust 图像处理核心（独立可执行文件，stdin→JSON→stdout NDJSON）
electron-app/
  main/             Electron 主进程（spawn Rust sidecar，IPC 注册）
  preload/          contextBridge 安全 API 暴露
  renderer/         Vite + Vue 3 + Pinia 渲染进程 UI
scripts/            构建/开发脚本
```

**通信链路：**
```
Vue 渲染进程 → IPC → Electron 主进程 → stdin → rust-core.exe
rust-core.exe → stdout NDJSON → Electron 主进程 → IPC → Vue 渲染进程
```

**NDJSON 协议（rust-core ↔ Electron 的合约）：**

Rust 通过 `println!` 向 stdout 逐行输出 JSON，Electron `rust-bridge.ts` 逐行解析。消息类型：

| type | 字段 | 含义 |
|------|------|------|
| `image_processed` | `index`, `total` | 单张图片处理完成（Rayon 并行） |
| `stage_changed` | `stage`, `message` | 阶段切换：`processing_images` → `creating_collage` → `adding_border` → `adding_watermark` |
| `completed` | `outputs` | 处理成功，返回输出文件路径数组 |
| `error` | `message` | 处理失败，rust-core 以 exit code 1 退出 |

**RustBridge 生命周期**（`electron-app/main/rust-bridge.ts`）：
- `start()`: spawn Rust 子进程 → 写 JSON 到 stdin → `stdin.end()` → 缓冲解析 stdout NDJSON → 进程 close 后 resolve/reject
- `cancel()`: `childProcess.kill()` 强制终止
- `isRunning()`: 检查 `this.process !== null`
- exe 路径：打包后从 `process.resourcesPath` 读取，开发模式优先 `target/release/` 其次 `target/debug/`

**类型同步注意**：`CollageConfig` 在 3 处定义（`main/rust-bridge.ts`、`preload/preload.ts`、`renderer/src/types/protocol.ts`），修改字段时必须三处同步。`renderer` 侧在调用 IPC 前需 `JSON.parse(JSON.stringify(config))` 将 Vue reactive Proxy 转为纯对象，否则结构化克隆会失败。

### 进度条阶段权重

App.vue 中 `STAGE_PROGRESS` 将 Rust 的阶段消息映射到 0-100 进度条：

| stage | 基础进度 |
|-------|---------|
| `processing_images` | 0%（此前由 `image_processed` 驱动 0→60%） |
| `creating_collage` | 62% |
| `adding_border` | 82% |
| `adding_watermark` | 92% |
| `completed` | 100% |

### Vue 组件树

```
App.vue (标签页路由：主页/设置)
├── FileSelector.vue      — 文件列表、拖拽排序、清空
├── ProgressBar.vue       — 进度条 + 错误/完成状态展示
├── SettingsPanel.vue     — 重采样/边框/DPI/背景色/前缀设置
└── WatermarkSettings.vue — 水印开关、路径、位置、缩放
```

状态管理：Pinia `appStore`（`electron-app/renderer/src/stores/appStore.ts`），设置通过 `localStorage` 持久化，key 为 `piclayout_settings`。

### Rust 核心模块

| 模块 | 职责 |
|------|------|
| `config.rs` | `CollageConfig` 反序列化，`BackgroundColor → Rgba<u8>` |
| `image_proc.rs` | `resample`（Lanczos3）+ `add_square_border`（居中填充） |
| `collage.rs` | 分块拼贴（`CHUNK_SIZE=2` 行/块，控制内存占用）→ 临时文件 → 多 chunk 垂直合并 |
| `border.rs` | `calculate_dynamic_border`（cols 线性插值）+ 最终缩放加边框 |
| `watermark.rs` | Porter-Duff "over" alpha 合成水印 |
| `dpi.rs` | JFIF APP0 DPI 字节注入（直接修改 JPEG 二进制偏移 11-15，写入 units=1 和 density 值） |
| `pipeline.rs` | rayon 并行流水线编排：阶段1 并行处理单图 → 阶段2 拼贴 → 阶段3 最终边框 → 阶段4 水印 |
| `error.rs` | `AppError` enum（`thiserror`），含 Image/Io/Json/NoImagesProcessed/Processing 变体 |
| `progress.rs` | NDJSON 进度消息序列化，通过 `println!` 写 stdout |

### 参数默认值

| 参数 | 默认值 |
|------|--------|
| 重采样大小（长边） | 4000 px |
| 单图边框（正方形边长） | 4200 px |
| 最终图像长边 | 10000 px |
| DPI | 300 |
| 背景色 | white |
| 动态外边框（cols≤2）| 1000 px |
| 动态外边框（cols≥10）| 200 px |
