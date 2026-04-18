# PicLayout

将多张照片拼接成正方形拼贴图，支持重采样、边框、水印，输出高分辨率 JPEG。

## 功能

- 并行处理图片（rayon 多线程）
- 每张图等比缩放后居中放置在正方形底色画布上
- 自动计算网格布局（接近正方形排列）
- 根据列数动态调整外边框宽度
- 可选水印（位置、缩放自定义）
- 输出 300 DPI JPEG，兼容打印

## 使用

### 桌面应用（推荐）

从 [Releases](../../releases) 下载安装包，双击安装，无需额外依赖。

### 从源码运行（开发模式）

需要 [Rust](https://rustup.rs) 和 Node.js 18+。

```bash
# 编译 Rust 核心
cd rust-core
cargo build --release

# 启动 Electron 前端
cd ../electron-app
npm install
npm run dev
```

## 参数说明

| 参数 | 默认值 | 说明 |
|------|--------|------|
| 重采样大小 | 4000 px | 单张图片长边缩放目标 |
| 单图边框 | 4200 px | 正方形底色画布边长 |
| 最终长边 | 10000 px | 加外边框后整体缩放目标 |
| DPI | 300 | 输出分辨率 |
| 背景色 | white | 底色及边框颜色 |

## 输出文件

| 文件 | 说明 |
|------|------|
| `{prefix}_collage.jpg` | 拼贴图（无外边框） |
| `{prefix}_collage_final.jpg` | 加外边框并缩放至目标长边 |
| `{prefix}_collage_final_watermarked.jpg` | 加水印版本（可选） |

## 架构

```
rust-core/     Rust 图像处理核心，stdin 读 JSON → stdout 输出 NDJSON 进度
electron-app/  Electron + Vite + Vue 3 前端
scripts/       构建脚本
PicsLayout_V8.py  原始 Python/Tkinter 版本（保留备用）
```

## License

MIT
