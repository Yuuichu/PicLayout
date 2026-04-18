# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

PicLayout 是一个 Python Tkinter GUI 工具，用于将多张图片拼接成正方形拼贴图，支持重采样、边框添加、水印叠加等功能。

## 运行方式

### Python 原版（保留）
```bash
python PicsLayout_V8.py
```
依赖：`Pillow`（PIL）、Python 标准库（tkinter、concurrent.futures、threading）

### Rust + Electron 新版（开发模式）
```bash
# 需先安装 Rust（https://rustup.rs）和 Node.js
bash scripts/dev.sh
```

### 构建发布包
```bash
bash scripts/build.sh
# 输出：dist-electron/PicLayout Setup *.exe
```

### 单独测试 Rust 核心
```bash
cd rust-core
cargo test                        # 单元测试
echo '{"image_paths":[...],...}' | cargo run --release  # CLI 集成测试
```

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

### Rust 核心模块

| 模块 | 职责 |
|------|------|
| `config.rs` | `CollageConfig` 反序列化，`BackgroundColor → Rgba<u8>` |
| `image_proc.rs` | `resample`（Lanczos3）+ `add_square_border`（居中填充） |
| `collage.rs` | 分块拼贴（2行/块）→ 临时文件 → 合并 |
| `border.rs` | `calculate_dynamic_border` + 最终缩放加边框 |
| `watermark.rs` | Porter-Duff alpha 合成水印 |
| `dpi.rs` | JFIF APP0 DPI 字节注入 |
| `pipeline.rs` | rayon 并行流水线编排 |
| `progress.rs` | NDJSON 进度消息序列化写 stdout |

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

`PicsLayout_V4.py` 是旧版本，保留备用，不主动维护。
