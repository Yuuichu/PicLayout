# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

Frameverse 是一个面向摄影师的跨平台拼图排版工具（Rust + Electron + Vue 3），可将多张照片排列为网格拼图，添加留白、外边框、Logo 和文本，按 Instagram、小红书等平台封面比例导出高分辨率 JPEG。

Python 原版 `PicsLayout_V8.py`（Tkinter GUI）保留备用，**不要修改**。`PicsLayout_V4.py` 是更旧版本。

## 运行与构建

### 开发模式

```bash
# 一键启动
bash scripts/dev.sh
# 等价于：cargo build --release → npm install → npm run dev

# 或分步启动（Rust debug 模式）
PICLAYOUT_RUST_PROFILE=debug bash scripts/dev.sh
```

### 构建发布包

```bash
# Windows NSIS 安装包
bash scripts/build.sh

# macOS DMG/ZIP（需 Xcode + CMake）
bash scripts/build-macos.sh arm64    # Apple Silicon
bash scripts/build-macos.sh x64      # Intel
```

### 单独测试 Rust 核心

```bash
cd rust-core
cargo test                                   # 单元测试
echo '{"image_paths":["a.jpg","b.jpg"],"output_dir":".","prefix":"test"}' | cargo run --release

# 列出系统字体（cosmic-text 字体数据库）
cargo run --release -- --list-fonts

# 渲染预览 PNG（参数：输出路径 + 预览长边像素）
cargo run --release -- --render-preview preview.png 1800
```

Rust release profile (`Cargo.toml`): `opt-level=3`, `lto=true`, `codegen-units=1`, `strip=true`。
首次构建时 `heif-rs`（HEIC 解码）会下载预编译的 libheif 静态库，需互联网连接。

## 架构说明

```
rust-core/          Rust 图像处理核心（独立可执行文件，3 种运行模式）
electron-app/
  main/             Electron 主进程（spawn Rust sidecar、IPC 注册）
  preload/          contextBridge 安全 API 暴露
  renderer/         Vite + Vue 3 + Pinia 渲染进程 UI
scripts/            构建/开发脚本
dist-electron/      构建输出目录
```

### 通信协议

Rust sidecar 通过 stdin 接收一行 JSON 配置，通过 stdout 逐行输出 NDJSON 进度消息（每个 `println!` 为一条），stderr 仅用于调试日志。Electron `rust-bridge.ts` 缓冲解析 stdout 行。

NDJSON 消息类型：

| type | 关键字段 | 含义 |
|------|---------|------|
| `job_started` | `total` | 任务开始 |
| `image_processed` | `index`, `total`, `elapsed_ms` | 单张图片处理完成（Rayon 并行） |
| `stage_changed` | `stage`, `message`, `elapsed_ms` | 阶段切换 |
| `stage_finished` | `stage`, `elapsed_ms`, `total_elapsed_ms`, `details[]` | 阶段结束计时 |
| `completed` | `outputs`, `processed_count`, `failed_images`, `warnings`, `elapsed_ms`, `stage_timings` | 处理成功 |
| `preview_completed` | `output_path`, `width`, `height`, `final_width`, `final_height` | 预览 PNG 已生成 |
| `error` | `message` | 处理失败，sidecar 以 exit code 1 退出 |
| `cancelled` | `message`, `partial_outputs` | Electron 主进程合成，非 Rust 发出 |

### Rust 侧运行模式

- **默认模式**（无参数）：stdin → JSON 配置 → 运行完整 pipeline → stdout NDJSON
- **`--render-preview`**：stdin → JSON 配置 → 渲染到临时 PNG → stdout NDJSON（preview_completed）
- **`--list-fonts`**：不读 stdin，直接 stdout 输出 JSON 字体列表

### Rust 核心模块

| 模块 | 职责 |
|------|------|
| `config.rs` | `CollageConfig` 反序列化与验证、`BackgroundColor`/`ProcessingMode`/`PositionReference` 等枚举、百分比→像素换算 |
| `image_loader.rs` | 图片解码（TurboJPEG for JPEG、heif-rs for HEIC/HEIF、image crate for 其他格式）、2GB 内存安全限制、EXIF/ICC 提取 |
| `color.rs` | lcms2 色彩管理：ICC profile 加载、源→目标色彩空间转换、sRGB/自定义 profile 支持、渲染意图 |
| `metadata.rs` | EXIF 方向读取、ICC profile 提取（JPEG/PNG）、`kamadak-exif` 解析 |
| `image_proc.rs` | `fit_long_edge`（保持比例缩放）、Lanczos3 高质量/线性光 resize、手动旋转 |
| `collage.rs` | `FinalCollageLayout`：网格布局计算、画布尺寸、瓦片排列、最终拼贴合成 |
| `border.rs` | 动态外边框 `calculate_dynamic_border`（按列数线性插值） |
| `watermark.rs` | Porter-Duff "over" alpha 合成水印（支持 `PositionReference::Canvas`/`Content`） |
| `text_block.rs` | cosmic-text 文字渲染：多行文本、对齐、字体/字重/样式、背景色块 |
| `fonts.rs` | cosmic-text fontdb 系统字体枚举、`system_has_family` 查询 |
| `jpeg_output.rs` | TurboJPEG 编码 → 原子写入（先写临时文件再 rename）、支持注入 ICC profile |
| `dpi.rs` | JFIF APP0 DPI 字节写入（直接修改 JPEG 二进制偏移）、ICC profile segment 注入 |
| `pipeline.rs` | 并行流水线编排：验证配置 → Rayon 并行处理单图 → 拼贴合成 → overlay（水印/文本）→ 保存输出。含 4GB 内存估算、线程池自适应（高分辨率/多图片时限制并发） |
| `progress.rs` | NDJSON `ProgressMessage` 序列化、`Stage` 枚举、`println!` 写 stdout |
| `error.rs` | `AppError` enum（`thiserror`）：Image/Io/Json/NoImagesProcessed/Processing |

### Vue 组件树

```
App.vue
├── FileSelector.vue   (variant: "task" 侧栏摘要 / "viewer" 预览画布 / "filmstrip" 底部队列)
├── SettingsPanel.vue  (tab: Layout / Watermark / Text / Quality)
│   ├── WatermarkSettings.vue
│   └── TextBlockSettings.vue
└── ProgressBar.vue
```

状态管理：Pinia `appStore`（`renderer/src/stores/appStore.ts`），设置通过 `localStorage` 持久化（key `piclayout_settings`）、UI 状态独立持久化（key `piclayout_ui`）。

### 预览系统

两种预览共存：

1. **快速分层预览**（FileSelector viewer 画布）：纯前端 SVG，根据 `previewLayout.ts` 实时计算网格/瓦片/Logo/文本框位置，无需 Rust 参与，随参数变更即时刷新。
2. **精准预览**（`preview:render` IPC）：调用 Rust `--render-preview`，走完整图像处理管线生成 PNG，展示真实色彩/字体/拼贴效果。拖动 overlay 时自动切回快速预览继续编辑状态。

输出完成后，viewer 可直接显示最终 JPEG 的 data URL 预览。

### ProgressBar 阶段权重

App.vue 中 `STAGE_PROGRESS` 映射：

| stage | 基础进度 |
|-------|---------|
| `processing_images` | 0%（`image_processed` 驱动 0→60%） |
| `creating_collage` | 62% |
| `adding_border` | 82% |
| `adding_watermark` | 92% |
| `saving_output` | 96% |
| `completed` | 100% |

### Overlay 定位系统

水印和文本框支持两种定位参照（`PositionReference`）：
- **Canvas**（`canvas`）：相对于最终画布，切换画幅后位置保持不变。
- **Content**（`content`）：相对于拼图内容区域，切换画幅后 overlay 跟随照片移动。

前端的 `overlayPosition.ts` 提供 `overlayPositionToCanvasPoint`、`convertOverlayPositionReference` 等函数，在两种参照间换算坐标。切换参照时同步换算，避免视觉跳动。

### 画布比例预设

`aspectRatioPresets.ts` 定义 9 种预设：Auto、Instagram cover/grid 3:4、Instagram content 3:4、Instagram portrait 4:5、Instagram square 1:1、Instagram landscape 1.91:1、Xiaohongshu portrait 3:4、Xiaohongshu square 1:1、Xiaohongshu landscape 4:3、Custom ratio。

选择非 Auto 预设时，最终画布按"补边保全"策略将拼图完整缩放居中放入目标比例画布，剩余区域用背景色填充。

### RustBridge 生命周期

`electron-app/main/rust-bridge.ts`：
- `start(config, onProgress)`: spawn Rust 子进程 → 写 JSON 到 stdin → `stdin.end()` → 缓冲解析 stdout NDJSON → 进程 close 后 resolve/reject
- `renderPreview(config, longEdge)`: 同上但附加 `--render-preview <tempPath> <longEdge>` 参数；完成后读取 PNG 文件转 base64 data URL；finally 清理临时目录
- `cancel()`: `childProcess.kill()` + 清理临时输出文件
- `isRunning()`: 检查 `this.process !== null`
- exe 路径：打包后从 `process.resourcesPath` 读取，开发模式优先 `target/release/` 其次 `target/debug/`，可通过 `PICLAYOUT_RUST_PROFILE=debug` 强制 debug

### 字体扫描

Electron `font-metadata.ts` 通过 `spawn(rustCorePath, ['--list-fonts'])` 获取系统字体列表。首次调用缓存结果，后续直接从内存返回。Rust 侧使用 `cosmic-text` 的 fontdb 枚举字体。

### 类型同步注意

`CollageConfig` 及相关类型在 3 处独立定义：
1. `electron-app/main/rust-bridge.ts`（Electron 主进程侧）
2. `electron-app/renderer/src/types/protocol.ts`（渲染进程侧）
3. `rust-core/src/config.rs`（Rust 反序列化目标）

`ElectronAPI` 类型在 2 处独立定义：
1. `electron-app/preload/preload.ts`
2. `electron-app/renderer/src/types/electron-api.d.ts`

修改字段时必须所有位置同步。渲染进程调用 IPC 前需 `JSON.parse(JSON.stringify(config))` 将 Vue reactive Proxy 转为纯对象。

### 参数默认值

| 参数 | 默认值 |
|------|--------|
| 最大图片数 | 40（硬限制 500） |
| 内容长边 | final_size 的 40% |
| 单图边框 | final_size 的 1% |
| 图片间距（横向/纵向）| 0% |
| 最终外边距 | 自动（按列数动态计算） |
| 最终画布长边 | 10000 px |
| 画布比例 | Auto |
| JPEG 质量 | 95 |
| DPI | 300 |
| 背景色 | white |
| Processing mode | standard_high_quality |
| 色彩管理 | 启用（sRGB 目标） |
| 水印位置参照 | content |
| 文本框位置参照 | content |

### 内存安全

- 单张图片解码：最大 2GB RGBA 分配
- HEIC 文件解码：最大 256MB 输入文件限制
- 整个 pipeline RGBA 工作集：硬限制 4GB（超过拒绝处理），2GB 时发出警告并自动降低线程并发数
- 高分辨率（内容长边 ≥3500px）或 >20 张图片时，Rayon 线程池限制为 ≤4 线程

### 处理模式

| 模式 | 线性光缩放 | 说明 |
|------|----------|------|
| `standard_high_quality` | 关闭 | 默认，标准高质量 |
| `maximum_quality` | 默认开启 | 极致画质（可手动关闭） |
| `fast_preview` | 关闭 | 快速预览（不使用线性光） |

### 输出文件

无 overlay 时：`{prefix}_collage_final.jpg`
有 overlay 时：`{prefix}_collage_final_watermarked.jpg`

### macOS 打包注意事项

- 需要 Xcode + CMake
- 依赖：`rustup target add aarch64-apple-darwin` 或 `x86_64-apple-darwin`
- Node.js 要求：22.12–24（推荐 22 LTS）
- Rust sidecar 复制到 `electron-app/build/sidecar/rust-core`（注意无 `.exe` 后缀）
- DMG/ZIP 输出到 `dist-electron/`
- 无 Developer ID 时使用 ad-hoc 签名（仅限本地测试）
- 完整文档：`docs/macos-packaging.md`
