# Codex 更新兼容与降级设计

## 目标

发布 1.0.3，解决 Codex CLI 更新后额度悬浮窗因用量接口超时而进入错误状态的问题，并避免 Windows 误选桌面自带旧版 CLI。

## 范围

- Windows 启动 Codex 时优先使用 npm 安装的当前架构二进制；npm 版本不存在时再使用 PATH 中的版本。
- 额度限制和 Token 用量分开读取。额度限制成功、用量暂时失败时，仍发布新额度并保留最近一次 Token 用量。
- 对 JSON-RPC 返回的后端超时、网络错误、未登录和真正协议错误分别分类，避免把后端超时显示为协议不兼容。
- 保留现有旧版 Codex 回退能力，不绑定单一 CLI 版本。
- 版本元数据和更新说明统一提升到 1.0.3。

## 数据流

1. 启动时按平台解析 Codex 二进制路径；Windows 先检查 `%APPDATA%\\npm\\node_modules\\@openai\\codex` 的固定 vendor 路径，再检查 PATH。
2. 启动 app-server 并完成 initialize。
3. 每次指标轮询先读取 rate limits；成功后立即更新额度字段。
4. 再读取 usage；成功则更新 Token 字段，失败则保留旧 Token 字段，并记录可重试的状态提示。
5. 下一轮继续重试 usage；只有 app-server 启动失败、登录失效或额度接口本身失败时才阻止新额度发布。

## 错误处理

- 包含 `timeout`、`timed out`、`network`、`connection` 的 JSON-RPC 错误归为超时/网络不可用。
- 包含 `unauthenticated` 或 `login` 的错误归为未登录。
- 仅无法解析 JSON、字段结构不符或明确方法/协议错误时归为协议或格式错误。
- UI 文案必须说明“额度可用、用量稍后重试”，不得显示过度笼统的“协议版本不兼容”。

## 验收标准

- 使用 Codex 0.155.0-alpha.3.10 时，首次 usage 请求返回后端超时不会让额度页面变成不可用状态。
- rate limits 成功而 usage 失败时，页面仍显示最新 5 小时/周额度，并保留上一次 Token 数据。
- Windows 同时存在 npm CLI 和桌面自带 CLI 时，启动日志/测试可证明优先选择 npm CLI。
- 真实协议错误仍会被识别，未登录错误仍显示登录提示。
- npm、Cargo、Tauri 配置和更新说明中的版本均为 1.0.3。
