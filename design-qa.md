# 方案 1 设计验收

- source visual truth: user-approved design reference from the Codex workspace
- implementation screenshot: `docs/release/native-option-1-final-collapsed.png`
- full-view comparison: `docs/release/design-option-1-comparison.png`
- state: collapsed floating window; the reference uses sample values (`22%`, `0`), while the implementation uses live Codex values (`3%`, `214427K`).
- viewport and normalization: source capsule crop `1595 × 337 px`, resized to `960 × 202 px`; native implementation capsule crop `960 × 202 px`; Windows capture was taken at the system's 150% display density.

## Findings

No actionable P0, P1, or P2 differences.

- Fonts and typography: the implementation uses Microsoft YaHei UI / PingFang SC fallback, with the same two-level Chinese hierarchy and strong numeric emphasis as the reference.
- Spacing and layout rhythm: three equal-width sections, continuous pill silhouette, thin vertical dividers, and compact internal spacing match the reference composition.
- Colors and visual tokens: cream base, sage outline and progress remainder, apricot progress arc, gray-green text, and soft elevation match the selected palette.
- Image and asset fidelity: the reference contains no decorative raster asset. The live quota ring is drawn as a native canvas data visualization so it can reflect the actual remaining quota instead of a static image.
- Copy and content: the three persistent labels match the selected design; dynamic values intentionally differ because they are live data.

## Interaction and overflow evidence

- Browser preview at `660 × 160 CSS px`: the pill measured `640 × 136`, `border-radius: 999px`, `overflow: hidden`, document scroll `660 × 160`, and no console warnings or errors.
- Clicking the pill showed the details panel; the measured content height was `347 px`, and the window resized to `365 CSS px` instead of scrolling inside the collapsed size.
- In the native Windows window, a real mouse click resized the outer window from `1012 × 242 px` to `1012 × 739 px`; clicking `收起` restored it to `1012 × 242 px`.
- A real drag moved the native window `90 × 45 px` while keeping it collapsed.

## Comparison history

1. The prior design had a non-transparent outer frame, a vertical scrollbar, and no verified native drag behavior.
2. The implementation now uses transparent document roots, a `999px` pill radius, an adaptive native window height, and drag regions on every child element.
3. The post-fix browser and native captures show no square window corners or overflow scrollbar.

## Follow-up polish

- P3: compare on a second monitor with a different Windows scaling ratio if a future release needs pixel-perfect cross-DPI tuning.

final result: passed

## 紧凑版三态复核（2026-08-13）

- 悬浮球：原 92px 收为 88px，原生窗口固定为 `116 × 116`；四边保留 14px 透明安全区，外阴影不再贴到 WebView 边缘。
- 三等分胶囊：原 `660 × 154` 收为 `520 × 120`；可见胶囊为 `492 × 92`，三栏等宽，额度环从 76px 收为 60px，字号与间距同步收紧。
- 详情页：宽度从 660px 收为 620px；窗口高度按内容一次性调整，取消逐帧原生缩放，避免切换时的边缘闪动和阴影裁切。
- Windows 窗口状态：只保存桌面位置，不再恢复旧尺寸，避免圆球启动后套着上一次详情页的透明长方形。
- 图标：重新生成全套 Windows 图标，并把主程序名从通用 `app.exe` 改为 `CodexQuotaWindow.exe`，使新快捷方式绕过旧图标缓存。
- 托盘：删除配置层自动托盘，只保留带菜单状态的代码托盘，单进程仅创建一个图标。
- 任务状态：以共享任务日志中的真实 `request_user_input`、执行事件和 `task_complete` 判定红黄绿；不再把最近修改过的 `notLoaded` 任务猜成执行中。
- 自动验证：Rust 27 项、前端 20 项及生产构建全部通过；样式边界按窗口尺寸进行静态复核，无页面滚动区域。

final result: passed

## 第二版三态与 Windows 发布版复核（2026-08-11）

- ball state: browser preview measured `92 x 92 px`; `body.scrollWidth === innerWidth`, no horizontal overflow. Native release window measured `112 x 112` logical pixels.
- summary state: first click resized the native window to `660 x 154`; browser preview showed three equal segments and the approved cream/sage/apricot neomorphic treatment.
- details state: second click resized the native window to `660 x 468` in the current data state. Browser preview content measured `348 px`, with document overflow hidden and no internal page scrollbar.
- drag behavior: native pointer drag moved the window by exactly `140 x 60` pixels while preserving the compact size.
- release visual check: the Windows release capture shows the cream surface directly over the desktop with rounded transparent edges; no rectangular frame or console window is present.
- data safety: native Codex payloads with `null` numeric fields now normalize to unknown values and display `--` plus the explicit `读取 Codex 数据超时` message instead of showing `null%` or stale-looking numbers.

Evidence:

- browser summary: `docs/release/browser-summary-state.png`
- browser details: `docs/release/browser-details-state.png`
- native details with explicit error state: `docs/release/native-details-fixed-visible.png`

final result: passed
