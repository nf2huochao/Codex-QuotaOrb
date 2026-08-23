# Codex 实时状态与批准请求桥接

悬浮窗使用 Codex 结构化 Hooks：`UserPromptSubmit` 写入黄色正在执行，`PermissionRequest` 写入红色待批准，`Stop` 写入绿色可验收，`SessionEnd` 收束为灰色。桌面端和网页端都读取同一份快照；日志文字不会改变状态。批准请求仍由用户手动批准或拒绝，超时按拒绝处理，不会自动批准。

## 启用方式

应用需要在 `~/.codex/hooks.json` 中保留以下 Hook。该文件是 Codex 的全局配置，修改前应备份；不要覆盖已有的其他 Hook。下面是 Windows 示例：

```json
{
  "hooks": {
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File %USERPROFILE%/.codex/tools/codex-lifecycle-hook.ps1 -EventName UserPromptSubmit", "timeout": 5 }] }],
    "PermissionRequest": [{ "hooks": [{ "type": "command", "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File %USERPROFILE%/.codex/tools/codex-permission-hook.ps1", "timeout": 310 }] }],
    "Stop": [{ "hooks": [{ "type": "command", "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File %USERPROFILE%/.codex/tools/codex-lifecycle-hook.ps1 -EventName Stop", "timeout": 5 }] }],
    "SessionEnd": [{ "hooks": [{ "type": "command", "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File %USERPROFILE%/.codex/tools/codex-lifecycle-hook.ps1 -EventName SessionEnd", "timeout": 5 }] }]
  }
}
```

修改后必须重启 Codex，使配置重新加载。悬浮窗退出或接口不可用时，生命周期 Hook 会快速返回，不阻塞 Codex；批准 Hook 仍会在 310 秒后按拒绝处理，避免任务无期限卡住。
