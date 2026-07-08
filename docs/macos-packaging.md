# macOS 打包

## 构建环境

macOS 包必须在 macOS 上构建。需要完整 Xcode、Node.js 22 LTS、Rust stable、CMake 和 pkg-config。

使用 Homebrew 安装：

```bash
brew install node@22 rustup cmake pkgconf
export PATH="$(brew --prefix node@22)/bin:$(brew --prefix rustup)/bin:$PATH"
rustup default stable
```

## 本地构建

Apple Silicon：

```bash
bash scripts/build-macos.sh arm64
```

Intel：

```bash
bash scripts/build-macos.sh x64
```

产物写入 `dist-electron/`：

- `Frameverse-<version>-macOS-<arch>.dmg`
- `Frameverse-<version>-macOS-<arch>.zip`

每个架构必须独立构建。脚本会先为目标架构编译 Rust，再把对应 sidecar 放入 App；不要用 arm64 sidecar 打 x64 包。

没有可用的 `Developer ID Application` 证书时，脚本会自动使用 ad-hoc 签名，确保 Electron helpers 和 Rust sidecar 形成完整的本地签名链。ad-hoc 签名不适用于对外分发。

## 签名与公证

直接下载分发使用 `Developer ID Application` 证书。把证书导入 Keychain，或通过 electron-builder 支持的 `CSC_LINK` 和 `CSC_KEY_PASSWORD` 环境变量提供证书。

公证可以使用 Apple ID 的 App 专用密码：

```bash
export CSC_LINK="/path/to/developer-id.p12"
export CSC_KEY_PASSWORD="p12-password"
export APPLE_ID="developer@example.com"
export APPLE_APP_SPECIFIC_PASSWORD="xxxx-xxxx-xxxx-xxxx"
export APPLE_TEAM_ID="ABCDE12345"
PICLAYOUT_NOTARIZE=1 bash scripts/build-macos.sh arm64
```

不要把证书或密码写入仓库。启用 `PICLAYOUT_NOTARIZE=1` 后，缺少有效签名或 Apple 凭据会导致构建失败，这是发布构建的预期行为。

## 验证

```bash
codesign --verify --deep --strict --verbose=2 \
  dist-electron/mac-arm64/Frameverse.app
spctl --assess --type execute --verbose=4 \
  dist-electron/mac-arm64/Frameverse.app
xcrun stapler validate dist-electron/Frameverse-*-macOS-arm64.dmg
```

未签名的本地测试包不会通过 `spctl`。正式发布前还应在一台没有开发环境的 Mac 上验证安装、图片选择、精准预览和导出。
