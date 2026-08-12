# Windows 交付

在已安装 Rust、Node.js 和 Visual Studio C++ 构建工具的 PowerShell 中运行：

```powershell
npm.cmd run tauri build
```

安装包位于 `src-tauri/target/release/bundle/nsis/`。签名更新版本还会生成对应 `.sig` 文件。
