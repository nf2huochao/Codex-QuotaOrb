# 手机网页刷新反馈 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent mobile safe-area clipping and make pull-to-refresh visibly communicate pull, refresh, and latest-update states.

**Architecture:** Keep the existing LAN snapshot and `refresh()` flow. The webpage owns only presentation state for the pull hint, while the existing `fetched_at`, `source`, and `status` fields remain the single source for the freshness line shared with the desktop view.

**Tech Stack:** Static HTML/CSS/JavaScript, DOM touch events, Playwright.

## Global Constraints

- Respect iOS safe-area insets without changing desktop layout.
- Show `下滑刷新` while pulling below the top, `松开刷新` at 52px, and `正在刷新…` after release.
- Show a short `已更新 · 时间` confirmation after a successful pull refresh.
- Keep the webpage free of an “立即更新” button.
- Do not add a second data source or change desktop window behavior.

---

### Task 1: Fix mobile layout and pull feedback

**Files:**
- Modify: `web/index.html`

**Interfaces:**
- Consumes: existing `refresh()`, `render()`, `fetched_at`, `source`, and `status` values.
- Produces: safe-area-aware page padding and visible pull lifecycle feedback.

- [x] Add `viewport-fit=cover` and safe-area-aware body padding.
- [x] Track pull distance and update the hint text at the 52px threshold.
- [x] Keep the hint visible during refresh and briefly show the latest update time.
- [x] Add the same source label used by the desktop freshness line.

### Task 2: Verify mobile interaction

**Files:**
- Modify: `tests/ui/pull-refresh.spec.ts`

- [x] Assert the hint changes from `下滑刷新` to `松开刷新`.
- [x] Assert release shows `正在刷新…`, then a successful `已更新 · 时间` message.
- [x] Run the full unit tests, UI tests, and production build.
- [x] Run `git diff --check`.
