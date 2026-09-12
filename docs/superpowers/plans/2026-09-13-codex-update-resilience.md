# Codex Update Resilience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 发布 1.0.3，使 Codex 更新后仍能显示可用额度，并支持自动检测与用户指定 Codex 路径。

**Architecture:** 保留现有 Tauri/Rust app-server 客户端。将 Windows 二进制解析改为“用户覆盖路径 → npm vendor → PATH → 系统回退”的顺序；将额度限制读取与 Token 用量读取解耦，额度成功时先发布快照，用量失败只保留旧值并等待下一轮重试。错误分类继续集中在 `diagnostics.rs`，前端沿用现有状态提示通道。

**Tech Stack:** Rust/Tauri 2, Tokio, serde_json, TypeScript, Vite, Vitest, Cargo tests。

## Global Constraints

- 版本统一为 `1.0.3`。
- 不绑定某一个 Codex CLI 版本；必须保留自动检测和回退路径。
- 用量接口失败不得清空或阻止额度字段更新。
- 用户选择的路径必须在保存前通过文件存在性和 app-server 握手验证。
- 不改动既有桌面端视觉布局；仅增加必要的设置项和错误文案。

---

### Task 1: Stabilize Codex binary resolution

**Files:**
- Modify: `src-tauri/src/lib.rs:366-408`
- Modify: `src-tauri/src/settings.rs` or the existing settings persistence module identified by `rg -n "settings|preferences|app_data_dir" src-tauri/src`
- Test: `src-tauri/src/lib.rs` test module or a focused resolver test module

**Interfaces:**
- Consumes: `CODEX_BINARY`, persisted user-selected path, `APPDATA`, `PATH`.
- Produces: one validated `PathBuf` passed to `CodexClient::spawn`.

- [ ] **Step 1: Write a failing resolver test**

  Add a pure candidate-order test that supplies a temporary user override, npm vendor path, and PATH path, then asserts the override wins, followed by npm vendor, followed by PATH.

- [ ] **Step 2: Run the focused Rust test and verify it fails**

  Run `cargo test --manifest-path src-tauri/Cargo.toml resolver_prefers_user_override -- --nocapture`.
  Expected: FAIL because the current resolver has no persisted override and scans PATH before the Windows npm vendor path.

- [ ] **Step 3: Implement the minimal resolver change**

  Extract candidate construction into a deterministic helper. On Windows order candidates as: non-empty persisted override, `CODEX_BINARY`, npm vendor path, PATH entries, and final `codex.exe`. Do not run arbitrary commands to validate a path during startup; validation is performed by the app-server handshake.

- [ ] **Step 4: Add the settings persistence contract**

  Store only the selected executable path under the existing application data directory. Add Tauri commands to read, validate, save, and clear the override. Validation must spawn the selected binary with `app-server --stdio`, complete `initialize`, and close the child without exposing stderr or credentials.

- [ ] **Step 5: Run focused tests**

  Run `cargo test --manifest-path src-tauri/Cargo.toml resolver_ -- --nocapture`.
  Expected: PASS, including empty and nonexistent override cases.

- [ ] **Step 6: Commit**

  `git add src-tauri/src/lib.rs src-tauri/src/settings.rs && git commit -m "fix: resolve and validate configurable Codex binary"`

### Task 2: Make diagnostics distinguish timeout from protocol errors

**Files:**
- Modify: `src-tauri/src/diagnostics.rs:26-57`
- Test: `src-tauri/src/diagnostics.rs` test module

**Interfaces:**
- Consumes: `CodexError::Response` text from JSON-RPC error responses.
- Produces: `DiagnosticCategory::Timeout`, `NetworkUnavailable`, `NotLoggedIn`, or `ProtocolMismatch`.

- [ ] **Step 1: Write failing classification tests**

  Cover `token usage profile fetch timed out` → `Timeout`, `connection reset` → `NetworkUnavailable`, `unauthenticated` → `NotLoggedIn`, and an unknown method error → `ProtocolMismatch`.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run `cargo test --manifest-path src-tauri/Cargo.toml diagnostics -- --nocapture`.
  Expected: timeout and network cases currently classify as `ProtocolMismatch`.

- [ ] **Step 3: Implement keyword classification**

  Normalize response text to lowercase and classify timeout phrases before the generic protocol branch. Keep login detection ahead of network/protocol detection, and retain the generic protocol fallback for malformed or unknown methods.

- [ ] **Step 4: Run focused tests**

  Run the same diagnostics command; expected PASS.

- [ ] **Step 5: Commit**

  `git add src-tauri/src/diagnostics.rs && git commit -m "fix: classify Codex backend timeouts accurately"`

### Task 3: Degrade gracefully when usage is unavailable

**Files:**
- Modify: `src-tauri/src/poller.rs:32-92` and metrics refresh branches
- Modify: `src-tauri/src/codex_client.rs` only if a bounded retry helper is needed
- Test: `src-tauri/src/poller.rs` test module and `src-tauri/src/codex_protocol.rs` tests where pure merge helpers belong

**Interfaces:**
- Consumes: rate-limit result, usage result, previous `Snapshot`.
- Produces: a fresh snapshot when rate limits succeed, with previous usage fields retained when usage fails.

- [ ] **Step 1: Write failing pure snapshot-merge tests**

  Add tests for: both requests succeed → fresh snapshot with new quota and usage; rate succeeds/usage fails → fresh quota with old `today_tokens` and `usage_date`, plus a retryable user-facing error; rate fails → existing stale/error behavior.

- [ ] **Step 2: Run the focused tests and verify they fail**

  Run `cargo test --manifest-path src-tauri/Cargo.toml poller -- --nocapture`.
  Expected: the current tuple match marks any usage failure stale and does not publish the new quota.

- [ ] **Step 3: Implement the split refresh flow**

  Read rate limits first. If rate limits succeed, build the next snapshot from the previous snapshot, replace quota fields, and then optionally replace usage fields when usage succeeds. On usage failure retain previous usage fields, set a retryable message equivalent to “额度已更新，用量统计暂时不可用，正在重试”, publish the snapshot, and allow the next metrics tick to retry.

- [ ] **Step 4: Keep app-server startup errors fatal**

  Do not mask `Spawn`, `ProcessExited`, or initialize failures; those still enter the reconnecting state. Only the optional usage request is degraded.

- [ ] **Step 5: Run focused and full Rust tests**

  Run `cargo test --manifest-path src-tauri/Cargo.toml poller -- --nocapture`, then `cargo test --manifest-path src-tauri/Cargo.toml`.
  Expected: PASS.

- [ ] **Step 6: Commit**

  `git add src-tauri/src/poller.rs src-tauri/src/codex_client.rs && git commit -m "fix: keep quota usable when Codex usage fetch fails"`

### Task 4: Add path selection UI and copy

**Files:**
- Modify: `src/main.ts` and the existing settings renderer identified by `rg -n "renderSettings|设置|Check for updates" src`
- Modify: `src/i18n.ts` or the existing translation source identified by `rg -n "settings|updates" src`
- Modify: `src-tauri/src/lib.rs` command registration for settings commands
- Test: existing settings component test file and a new focused interaction test if no settings test exists

**Interfaces:**
- Consumes: Tauri path read/save/clear commands.
- Produces: settings control showing selected path, browse button, validate/save feedback, and reset-to-auto action in Chinese and English.

- [ ] **Step 1: Write the failing UI test**

  Assert that settings renders automatic detection as the default, shows the selected executable path after loading, and exposes choose, validate/save, and clear actions.

- [ ] **Step 2: Run the focused UI test and verify it fails**

  Run `npm test -- --run src/components/SettingsPanel.test.ts` or the repository’s existing settings test path.
  Expected: FAIL because no Codex path control exists.

- [ ] **Step 3: Implement the smallest settings control**

  Use the platform file picker already available in the project or a native Tauri dialog dependency only if already present. Keep the selected path compact on mobile, show the full path in a title/tooltip, and provide an automatic mode without requiring manual configuration.

- [ ] **Step 4: Add bilingual copy**

  Add concise Chinese/English strings for automatic detection, selected path, choose path, validate, clear, invalid executable, and “quota updated; usage retrying”.

- [ ] **Step 5: Run frontend tests and build**

  Run `npm test -- --run`, then `npm run build`.
  Expected: PASS and a successful Vite build.

- [ ] **Step 6: Commit**

  `git add src/main.ts src/components src/i18n.ts src-tauri/src/lib.rs && git commit -m "feat: allow users to choose Codex executable"`

### Task 5: Bump and document 1.0.3

**Files:**
- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `CHANGELOG.md`
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `.github/workflows/release-macos.yml` default release input if required by the workflow

**Interfaces:**
- Consumes: completed compatibility changes and test results.
- Produces: consistent `1.0.3` package metadata and release notes.

- [ ] **Step 1: Update all version metadata**

  Change every current application `1.0.2` value to `1.0.3` in package, Cargo, Tauri, and release workflow defaults; leave historical release links and old changelog entries unchanged.

- [ ] **Step 2: Add release notes**

  Document automatic/user-selected Codex path resolution, Windows version selection fix, usage timeout degradation, retry behavior, and corrected diagnostics in both languages.

- [ ] **Step 3: Run consistency checks**

  Run `rg -n '1\\.0\\.2|1\\.0\\.3' package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json CHANGELOG.md .github/workflows/release-macos.yml` and verify only historical references remain at 1.0.2.

- [ ] **Step 4: Commit**

  `git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json CHANGELOG.md README.md README.zh-CN.md .github/workflows/release-macos.yml && git commit -m "release: prepare v1.0.3 Codex compatibility"`

### Task 6: End-to-end verification

**Files:**
- Modify: none unless verification exposes a regression
- Test: existing test suites and release build output

- [ ] **Step 1: Run Rust and frontend tests**

  Run `cargo test --manifest-path src-tauri/Cargo.toml`, `npm test -- --run`, and `npm run build`.

- [ ] **Step 2: Run the app-server compatibility smoke test**

  Against the installed Codex binary, verify initialize, rate limits, and a usage timeout response. Confirm the resulting snapshot keeps quota data and marks usage as retrying.

- [ ] **Step 3: Build the Windows package**

  Run the repository’s existing Tauri build command and verify the generated installer reports version 1.0.3.

- [ ] **Step 4: Review the final diff**

  Run `git status --short`, `git diff --check`, and inspect the changed files for secrets, accidental screenshots, and unrelated visual changes.

