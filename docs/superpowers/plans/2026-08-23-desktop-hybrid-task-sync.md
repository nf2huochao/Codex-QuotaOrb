# Codex Desktop 混合任务状态同步实施计划

1. 增加只读 `thread_history_1.sqlite` 状态读取模块，按线程选择最新轮次并生成规范任务状态。
2. 增加 Windows UI Automation 授权控件观察器，仅产出待批准补充信号。
3. 调整任务归并优先级，统一 Hook、辅助功能、状态库与元数据来源。
4. 将状态观察器接入现有快速轮询，所有界面继续消费同一 `SnapshotStore.tasks`。
5. 增加状态库、乱序轮次、红色保持和计数一致性测试。
6. 用真实 Codex Desktop 验证黄、红、绿、灰及多任务一致性；全部通过后再打包。

