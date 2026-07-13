# macOS 打包

## 构建环境

macOS 安装包必须在 macOS 上构建。项目使用 Node.js 22、Rust stable、CMake 和 Xcode Command Line Tools；签名和公证需要完整 Xcode。

```bash
nvm use
rustup default stable
cd electron-app
npm ci
```

## 本地构建

在目标架构的 Mac 上执行：

```bash
cd electron-app
npm run package
```

构建命令会先编译当前架构的 Rust release sidecar，再生成 DMG 和 ZIP。Apple Silicon 与 Intel 产物应分别在对应架构环境验证，避免把一个架构的 sidecar 放入另一个架构的应用包。

产物写入 `dist-electron/`：

- `Frameverse-<version>-<arch>.dmg`
- `Frameverse-<version>-<arch>.zip`

macOS Dock、应用包和 DMG 共用 `electron-app/build/icon.png`。electron-builder 在打包时从该文件生成 `.icns`，不要再维护第二份图标源文件。

## 签名与公证

公开分发需要 `Developer ID Application` 证书。可以把证书导入 Keychain，也可以通过 electron-builder 支持的环境变量提供：

```bash
export CSC_LINK="/path/to/developer-id.p12"
export CSC_KEY_PASSWORD="p12-password"
export APPLE_ID="developer@example.com"
export APPLE_APP_SPECIFIC_PASSWORD="xxxx-xxxx-xxxx-xxxx"
export APPLE_TEAM_ID="ABCDE12345"

cd electron-app
npm run package -- --config.mac.notarize=true
```

不要把证书或密码写入仓库。没有签名身份时生成的本地测试包可能触发 Gatekeeper，不适合公开分发。

## 验证

```bash
codesign --verify --deep --strict --verbose=2 \
  dist-electron/mac-arm64/Frameverse.app
spctl --assess --type execute --verbose=4 \
  dist-electron/mac-arm64/Frameverse.app
xcrun stapler validate dist-electron/Frameverse-*.dmg
```

正式发布前，还应在一台没有开发环境的 Mac 上验证安装、图片选择、精准预览、字体扫描和导出。
