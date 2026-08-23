# 批准状态 UI 桥接设计

## 目标

当 Codex Windows 桌面端已经显示“等待批准”而 app-server 没有发送批准事件时，悬浮窗仍能把对应任务显示为红色；普通正在执行、已完成和无任务状态保持现有逻辑不变。

## 方案

采用双通道状态源：

1. app-server 事件和 `PermissionRequest` Hook 继续作为主通道，收到真实批准请求时使用真实 request id，并保留现有批准/拒绝处理。
2. 当主通道没有批准请求时，Windows 专用 UI 桥接读取 Codex 桌面窗口的 Windows UI Automation 可访问文本，识别包含“等待批准”的任务条目，并按任务标题与现有 `thread/list` 任务合并，生成红色状态。UI 桥接不读取 rollout、JSONL 或日志内容。

UI 桥接只负责发现状态，不改变黄色/绿色/灰色的优先级。对于没有真实 request id 的 UI 桥接任务，不显示会误导用户的“批准/拒绝”按钮；用户仍可在 Codex 原窗口完成批准。真实事件恢复后，自动切换回真实 request id 和按钮。

## 数据流

`thread/list + app-server notifications + PermissionRequest Hook` → 任务注册表 → `UI Automation approval candidates` → 标题匹配合并 → 状态快照 → 桌面/网页三种视图。

UI Automation 读取失败时静默跳过并保留主通道结果，不把读取失败伪装成红色。

## 验收标准

- Codex 桌面端显示“测试审批弹窗 · 等待批准”时，快照中该任务为 `needs_action`，任务计数红色加 1。
- 同一任务没有等待批准标记时仍保持 `running`，不会因为普通文字出现“批准”而变红。
- 已经由真实 app-server 事件识别的任务仍保留真实 request id 和批准/拒绝按钮。
- UI Automation 不可用、Codex 未运行或窗口未找到时，其他任务和额度数据照常更新。
- 不读取日志，不修改 Codex 会话文件，不改变用户当前批准策略。

## 风险与边界

UI Automation 依赖 Codex 桌面端提供可访问文本，Codex UI 改版可能导致识别失效；因此它是批准事件的兜底通道，不替代官方 app-server 事件。按钮操作不在本次桥接中伪造，避免误批准。
