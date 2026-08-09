# Codex 额度悬浮窗实施计划

> For agentic workers: REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** 构建一个 Windows Tauri 常驻应用，以紧凑的三段等宽灵动岛悬浮球显示 Codex 周额度、任务状态数量和本日 Token，并在同一 Wi‑Fi 内提供只读网页/PWA。

**Architecture:** Rust/Tauri 负责系统托盘、置顶窗口、Codex app-server 子进程、快照轮询和局域网 HTTP/WebSocket；Vite + 原生 TypeScript 负责悬浮球、详情面板和网页仪表盘。Codex 协议、业务快照、任务状态映射、传输层和 UI 彼此隔离，iPhone/Kindle 复用局域网网页 API，原生 iOS 外壳作为第二阶段产物。

**Tech Stack:** Tauri v2、Rust stable、Tokio、Serde、Axum、Vite、TypeScript、Vitest、Playwright、Capacitor（第二阶段 iOS 外壳）。

## Global Constraints

- 第一阶段只允许同一 Wi‑Fi 访问，不开放公网、不上传云端、不保存或展示 Codex 登录令牌。
- Codex 数据通过本机 codex app-server 的 JSON-RPC 读取，协议适配必须隔离并保存 schemaVersion。
- 自动刷新间隔为 2 分钟；超过 15 分钟没有成功读取时必须标记过期。
- 灰色=没有活跃任务；红色=需要回复、确认或授权；黄色=正在执行；绿色=已完成、可以验收。
- “已验收”只隐藏本地仪表盘记录，不归档、不删除、不授权、不消费 Codex 数据。
- UI 使用 #ebe4d6 奶油白、#bdcfa2 鼠尾草绿、#ddb480 杏色，并保留外凸高光、内凹阴影和轻量投影。
- 悬浮球三段等宽、尽量无空档；左侧圆环周额度，中间任务状态数量，右侧本日 Token 总量。
- Windows 第一阶段可完整构建；iOS 原生编译和签名需要 macOS/Xcode，Windows 只生成可移交的 Capacitor 项目。

---

### Task 1: 初始化 Tauri 与前端工程

**Files:**
- Create: package.json
- Create: vite.config.ts
- Create: index.html
- Create: src/main.ts
- Create: src/styles.css
- Create: src-tauri/Cargo.toml
- Create: src-tauri/tauri.conf.json
- Create: src-tauri/src/main.rs
- Create: .gitignore

**Interfaces:**
- Produces npm scripts dev、build、test、test:ui 和 Tauri binary codex-quota-floating-window。

- [ ] Step 1: Scaffold the empty application

    Run:
    
        npm create vite@latest . -- --template vanilla-ts
        npm install
        npx tauri init --app-name "Codex 额度悬浮窗" --window-title "Codex 额度悬浮窗"

    Expected: src/main.ts、src-tauri/、package.json 和 Vite dev server are created.

- [ ] Step 2: Add the required scripts and dependencies

    package.json must expose dev、build、test、test:ui、tauri. Install @tauri-apps/api、@tauri-apps/plugin-autostart、@tauri-apps/plugin-process、vitest、jsdom 和 @playwright/test. Enable Tauri shell、process 和 autostart plugins in src-tauri/src/main.rs.

- [ ] Step 3: Verify the scaffold

    Run: npm run build
    Expected: TypeScript check and Vite build finish with exit code 0.

- [ ] Step 4: Commit

        git add package.json package-lock.json vite.config.ts index.html src src-tauri .gitignore
        git commit -m "chore: scaffold Tauri desktop app"

### Task 2: 建立领域模型与状态规则

**Files:**
- Create: src-tauri/src/domain.rs
- Create: src-tauri/src/domain_tests.rs
- Create: src/domain.ts
- Create: src/domain.test.ts

**Interfaces:**
- Produces Rust Snapshot、TaskSummary、TaskStatus、DataStatus and identical serialized TypeScript mirrors.
- Produces map_task_status(event: &TaskEvent) -> TaskStatus and snapshot_status(last_success: Option<i64>, now: i64, has_error: bool, authenticated: bool) -> DataStatus.

- [ ] Step 1: Write failing Rust tests

        #[test]
        fn needs_action_has_red_priority() {
            let event = TaskEvent { waiting_for_user: true, running: true, completed: false };
            assert_eq!(map_task_status(&event), TaskStatus::NeedsAction);
        }

        #[test]
        fn snapshot_is_stale_after_fifteen_minutes() {
            assert_eq!(snapshot_status(Some(0), 901, false, true), DataStatus::Stale);
        }

- [ ] Step 2: Run the focused test and verify failure

    Run: cargo test needs_action_has_red_priority snapshot_is_stale_after_fifteen_minutes
    Expected: FAIL because the domain types and functions do not exist.

- [ ] Step 3: Implement the domain types and rules

    Use these serialized fields:
    
        pub enum TaskStatus { None, NeedsAction, Running, Completed }
        pub enum DataStatus { Fresh, Stale, Error, Unauthenticated }
        pub struct TaskSummary {
            pub id: String, pub title: String, pub status: TaskStatus,
            pub token_count: Option<u64>, pub updated_at: i64, pub acknowledged: bool
        }
        pub struct Snapshot {
            pub status: DataStatus, pub fetched_at: Option<i64>,
            pub quota_remaining_percent: Option<u8>, pub quota_resets_at: Option<i64>,
            pub plan: Option<String>, pub reset_credits: Option<u64>,
            pub today_tokens: Option<u64>, pub active_task_count: u32,
            pub tasks: Vec<TaskSummary>, pub error: Option<String>,
            pub schema_version: String
        }

    Map waiting-for-user before running, running before completed, and use None only when there is no active task. Treat a successful read older than 900 seconds as stale.

- [ ] Step 4: Add equivalent TypeScript guards and tests

    Test that NeedsAction renders red, Running renders yellow, Completed renders green, None renders gray, and stale snapshots never render as fresh.

- [ ] Step 5: Run all domain tests and commit

    Run: cargo test domain
    Run: npm test -- src/domain.test.ts
    Expected: all focused tests pass.

        git add src-tauri/src/domain.rs src-tauri/src/domain_tests.rs src/domain.ts src/domain.test.ts
        git commit -m "feat: add quota snapshot and task status domain model"

### Task 3: 接入 Codex app-server JSON-RPC

**Files:**
- Create: src-tauri/src/codex_protocol.rs
- Create: src-tauri/src/codex_client.rs
- Create: src-tauri/fixtures/rate_limits.json
- Create: src-tauri/fixtures/usage.json
- Create: src-tauri/fixtures/thread_events.jsonl
- Create: src-tauri/src/codex_client_tests.rs

**Interfaces:**
- Produces CodexClient::spawn(codex_binary: &Path) -> Result<Self, CodexError>.
- Produces read_rate_limits() -> Result<RateLimitResponse, CodexError> and read_usage() -> Result<UsageResponse, CodexError>.
- Produces subscribe_events() -> impl Stream<Item = Result<CodexEvent, CodexError>>.

- [ ] Step 1: Write fixture-backed failing tests

        #[test]
        fn parses_remaining_percent_and_reset_credit_count() {
            let response = parse_rate_limits(include_str!("../fixtures/rate_limits.json")).unwrap();
            assert_eq!(response.remaining_percent, Some(72));
            assert_eq!(response.reset_credits, Some(1));
        }

    Also test missing primary、missing credits、unknown plan、malformed JSON、process exit and request timeout.

- [ ] Step 2: Run tests and verify failure

    Run: cargo test codex_client
    Expected: FAIL before the parser and process client are implemented.

- [ ] Step 3: Implement the JSON-RPC transport

    Spawn codex app-server --stdio, assign monotonically increasing request IDs, write one JSON-RPC request per line, and read response lines without logging request parameters. Implement account/rateLimits/read and account/usage/read. Preserve unknown fields with Serde defaults and reject missing required values.

- [ ] Step 4: Implement event normalization

    Normalize thread/turn events into TaskEvent { id, title, waiting_for_user, running, completed, token_count, updated_at }. Never send a write method to Codex.

- [ ] Step 5: Run parser, timeout and process tests; commit

    Run: cargo test codex_client -- --nocapture
    Expected: parser and failure-path tests pass without a real account.

        git add src-tauri/src/codex_protocol.rs src-tauri/src/codex_client.rs src-tauri/fixtures src-tauri/src/codex_client_tests.rs
        git commit -m "feat: read Codex rate limits and usage through app-server"

### Task 4: 建立快照轮询与内存状态仓库

**Files:**
- Create: src-tauri/src/snapshot_store.rs
- Create: src-tauri/src/poller.rs
- Create: src-tauri/src/snapshot_store_tests.rs
- Modify: src-tauri/src/main.rs

**Interfaces:**
- Produces SnapshotStore::current() -> Snapshot and SnapshotStore::acknowledge(task_id: &str) -> bool.
- Produces a Tokio task that runs immediately, then every Duration::from_secs(120), and publishes through tokio::watch::Receiver<Snapshot>.

- [ ] Step 1: Write failing freshness and merge tests

    Test immediate first poll, 120-second interval configuration, 900-second stale threshold, recovery from stale to fresh, unauthenticated response, and local acknowledgement that survives refresh but not process restart.

- [ ] Step 2: Implement the store and poller

    Merge rate limits, usage and normalized task events into one Snapshot. Keep fetched_at from the last successful complete read; on failure preserve the previous payload only as stale and set error.

- [ ] Step 3: Expose Tauri commands

    Add get_snapshot() -> Snapshot, refresh_now() -> Result<Snapshot, String> and acknowledge_task(task_id: String) -> bool.

- [ ] Step 4: Run focused tests and commit

    Run: cargo test snapshot_store
    Expected: all freshness、error and acknowledgement tests pass.

        git add src-tauri/src/snapshot_store.rs src-tauri/src/poller.rs src-tauri/src/snapshot_store_tests.rs src-tauri/src/main.rs
        git commit -m "feat: poll and publish quota snapshots every two minutes"

### Task 5: 实现 Windows 托盘与悬浮窗行为

**Files:**
- Create: src-tauri/src/tray.rs
- Modify: src-tauri/src/main.rs
- Modify: src-tauri/tauri.conf.json
- Test: src-tauri/src/tray_tests.rs

**Interfaces:**
- Produces tray menu items 显示悬浮球、隐藏悬浮球、立即更新、开机自启、检查更新、退出.
- Produces a transparent、undecorated、always-on-top window with persisted position and drag support.

- [ ] Step 1: Add tray menu tests

    Test menu labels, that 立即更新 invokes refresh_now, that 已验收 never appears in the Codex write client, and that exit terminates the poller cleanly.

- [ ] Step 2: Implement the tray and window configuration

    Set transparent background、rounded content、always-on-top、no taskbar entry、minimum size and a safe default position near top-center. Persist only window position and local acknowledgement IDs.

- [ ] Step 3: Wire autostart

    Use the Tauri autostart plugin; toggling the menu item updates a local setting and does not require administrator privileges.

- [ ] Step 4: Run desktop smoke test and commit

    Run: npm run tauri dev
    Verify tray menu、drag、hide/show、topmost、autostart and clean exit; then run cargo test tray.

        git add src-tauri/src/tray.rs src-tauri/src/tray_tests.rs src-tauri/src/main.rs src-tauri/tauri.conf.json
        git commit -m "feat: add Windows tray and floating window controls"

### Task 6: 实现三段等宽灵动岛与详情面板

**Files:**
- Create: src/components/FloatingIsland.ts
- Create: src/components/DetailsPanel.ts
- Create: src/components/TaskList.ts
- Modify: src/main.ts
- Modify: src/styles.css
- Create: src/components/FloatingIsland.test.ts

**Interfaces:**
- renderFloatingIsland(root: HTMLElement, snapshot: Snapshot, onOpen: () => void): void.
- renderDetailsPanel(root: HTMLElement, snapshot: Snapshot, onRefresh: () => void, onAcknowledge: (taskId: string) => void): void.
- renderTaskList(root: HTMLElement, tasks: TaskSummary[], onAcknowledge: (taskId: string) => void): void.

- [ ] Step 1: Write failing DOM tests

    Assert that a fresh 72% snapshot renders one quota ring、3 个任务执行中 and 128.4K; stale renders 数据已过期; error renders --; completed tasks show 已验收 and do not disappear until clicked.

- [ ] Step 2: Implement the compact island

    Use a three-column CSS grid inside one rounded capsule. Keep the left ring 54–56px, center task count single-line, right token value compact; remove decorative gaps. Use CSS variables --cream、--sage、--apricot、--shadow-out and --shadow-in.

- [ ] Step 3: Implement in-place expansion

    On click, animate the same window into a details card showing reset time、plan、reset credits、today Token、last update and task list. 立即更新 invokes the Tauri command; 已验收 only calls local acknowledgement.

- [ ] Step 4: Run frontend tests and visual smoke test

    Run: npm test -- src/components/FloatingIsland.test.ts
    Run: npm run tauri dev
    Expected: compact island、expansion、stale/error copy、four status colors and responsive narrow layout all work.

        git add src/main.ts src/styles.css src/components
        git commit -m "feat: render compact quota island and details"

### Task 7: 提供同一 Wi‑Fi 的网页/PWA

**Files:**
- Create: src-tauri/src/lan_server.rs
- Create: src-tauri/src/lan_server_tests.rs
- Create: src/web/app.ts
- Create: src/web/index.html
- Create: public/manifest.webmanifest
- Create: public/offline.html
- Modify: src-tauri/src/main.rs

**Interfaces:**
- GET /api/snapshot returns Snapshot and no authentication data.
- POST /api/tasks/:id/acknowledge changes only local acknowledgement state.
- GET /ws pushes snapshot updates; pairing middleware rejects requests without the random local session token.

- [ ] Step 1: Write failing Axum route tests

    Test unauthorized requests return 401, paired GET /api/snapshot returns DataStatus, acknowledgement changes only acknowledged, and WebSocket clients receive a fresh snapshot after the poller publishes.

- [ ] Step 2: Implement the LAN server

    Bind only to the selected LAN interface, generate a random session token at startup, keep it in memory, and expose read-only snapshot data plus local acknowledgement. Do not expose the app-server process or any auth file path.

- [ ] Step 3: Build the responsive PWA shell

    Reuse FloatingIsland、DetailsPanel and TaskList; add pairing screen、connection status、offline message and basic HTML fallback for Kindle browsers.

- [ ] Step 4: Run route and browser tests; commit

    Run: cargo test lan_server
    Run: npm run test:ui
    Expected: paired desktop、iPhone Safari and Kindle-compatible fallback can read the same snapshot; unpaired requests fail closed.

        git add src-tauri/src/lan_server.rs src-tauri/src/lan_server_tests.rs src/web public src-tauri/src/main.rs
        git commit -m "feat: add paired LAN dashboard and PWA"

### Task 8: 集成异常、日志与协议兼容

**Files:**
- Create: src-tauri/src/diagnostics.rs
- Create: src-tauri/src/integration_tests.rs
- Modify: src-tauri/src/codex_client.rs
- Modify: src-tauri/src/snapshot_store.rs

**Interfaces:**
- Produces typed error categories NotLoggedIn、AppServerUnavailable、ProtocolMismatch、NetworkUnavailable、Timeout、MalformedResponse.
- Produces redacted diagnostics containing timestamp、category and schema version only.

- [ ] Step 1: Add fixture-based failure tests

    Cover app-server missing、login expired、malformed rate-limit response、unknown schema field、LAN disconnect、stale recovery and no-snapshot first failure.

- [ ] Step 2: Implement redacted diagnostics and UI copy

    Map each category to a short Chinese message and retry action. Never log task正文、令牌、账号标识、路径 or JSON-RPC parameters.

- [ ] Step 3: Run integration tests and commit

    Run: cargo test integration
    Run: npm run test:ui -- --grep "过期|失败|未登录"
    Expected: every failure is explicit、recoverable where possible, and never presents stale data as fresh.

        git add src-tauri/src/diagnostics.rs src-tauri/src/integration_tests.rs src-tauri/src/codex_client.rs src-tauri/src/snapshot_store.rs
        git commit -m "feat: add explicit stale, error and protocol diagnostics"

### Task 9: 打包、开机自启和更新入口

**Files:**
- Modify: src-tauri/tauri.conf.json
- Create: src-tauri/icons/icon.ico
- Create: src-tauri/updater/update-manifest.json
- Create: docs/release/windows.md
- Create: tests/packaging-smoke.ps1

**Interfaces:**
- Produces a Windows installer and an in-app 检查更新 menu action that reports current version、available version or 暂无更新.

- [ ] Step 1: Add packaging smoke test

    The PowerShell test installs the built artifact in a temporary user directory, launches it, asserts a tray process exists, and uninstalls it without touching Codex credentials.

- [ ] Step 2: Configure Tauri bundle and updater metadata

    Use a versioned release manifest, verify signatures before applying updates, and keep update download disabled when no configured release endpoint exists. The first local build reports 未配置更新源.

- [ ] Step 3: Build and verify

    Run: npm run build
    Run: npm run tauri build
    Run: powershell -File tests/packaging-smoke.ps1
    Expected: installer is produced, tray/autostart work after installation, and update checks fail safely when no release source is configured.

        git add src-tauri/tauri.conf.json src-tauri/icons src-tauri/updater docs/release tests/packaging-smoke.ps1
        git commit -m "feat: package Windows app and add safe update check"

### Task 10: iPhone/Kindle 交付与设备验收

**Files:**
- Create: mobile/capacitor.config.ts
- Create: mobile/package.json
- Create: mobile/README.md
- Modify: src/web/index.html
- Modify: public/manifest.webmanifest
- Create: tests/device-checklist.md

**Interfaces:**
- Produces a Capacitor iOS project configuration pointing to the LAN web endpoint; the web build remains the single source of UI and data contracts.

- [ ] Step 1: Add responsive and e-ink checks

    Test 320px iPhone width、768px Kindle-like width、reduced animation、high-contrast text and a no-JavaScript pairing message.

- [ ] Step 2: Generate the Capacitor iOS shell

    Run npm install @capacitor/core @capacitor/cli, npx cap init and npx cap add ios inside mobile/. On Windows, verify the generated project and document that npx cap open ios and signing require macOS/Xcode.

- [ ] Step 3: Run device checklist and commit

    Verify Windows Chrome、iPhone Safari and Kindle browser on the same Wi‑Fi, including pairing、stale/error state、refresh、task acknowledgement and offline recovery.

        git add mobile src/web public tests/device-checklist.md
        git commit -m "feat: prepare iPhone shell and Kindle-friendly web dashboard"

## Final Verification

Run the complete suite:

    npm run build
    npm test
    cargo test
    npm run test:ui
    npm run tauri build

Expected: all automated tests pass, the Windows installer is created, the floating island shows real or fixture-backed data with explicit freshness, and a paired phone/Kindle browser can read the same local snapshot.

