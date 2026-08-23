# 网页端下滑刷新 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the webpage “立即更新” button with a top-of-page pull-to-refresh gesture and immediate refresh on release.

**Architecture:** Keep the existing `refresh()` command and snapshot flow. `DetailsPanel` owns a small touch gesture controller and exposes a top hint; `main.ts` continues to be the single refresh owner. The desktop views and view-cycle logic are unchanged.

**Tech Stack:** TypeScript, DOM touch events, CSS, Vitest, Playwright.

## Global Constraints

- The hint is visible only while pulling down from page top.
- The refresh threshold is 52px.
- Releasing below the threshold does not refresh.
- Releasing at or above the threshold invokes the existing refresh callback once.
- Do not add an update button or change desktop behavior.

---

### Task 1: Add the webpage pull gesture

**Files:**
- Modify: `web/index.html`

**Interfaces:**
- Consumes: existing webpage `refresh()` function and paired snapshot token.
- Produces: the existing `#pull-refresh` element with visible top-of-page gesture state.

- [x] **Step 1: Add the touch regression test**

  Added `tests/ui/pull-refresh.spec.ts`, which dispatches touch events at `scrollY = 0`, asserts the hint text, and verifies the snapshot endpoint is called only after a 52px pull is released.

- [x] **Step 2: Run the focused test**

  Run: `npm.cmd run test:ui -- tests/ui/pull-refresh.spec.ts`

  Result: PASS after the gesture implementation.

- [x] **Step 3: Implement the minimum gesture controller**

  Kept the existing `#pull-refresh` node, tracked the first touch Y only when `window.scrollY <= 0`, updated the hint during downward movement, and on `touchend` called the existing `refresh()` once when the distance is at least 52px. The native details view remains unchanged.

- [x] **Step 4: Add compact hint styles**

  Styled the hint as a top-centered, non-interactive label with the existing cream/sage palette. It stays collapsed when idle and visible while the gesture is active.

- [x] **Step 5: Run the focused tests and confirm they pass**

  Run: `npm.cmd run test:ui -- tests/ui/pull-refresh.spec.ts`

  Result: PASS.

### Task 2: Verify browser behavior and regression safety

**Files:**
- Modify: `tests/pairing.spec.ts` or add `tests/pull-refresh.spec.ts` only if an existing UI harness cannot exercise the gesture.

**Interfaces:**
- Consumes: the rendered webpage and the existing refresh callback/test fixture.
- Produces: automated coverage for hint visibility and one-shot refresh behavior.

- [x] **Step 1: Add a Playwright touch regression test**

  Use a mobile context, navigate to the paired webpage, dispatch a downward touch gesture from the top, and assert the hint appears before release and the snapshot timestamp changes after release.

- [x] **Step 2: Run the full verification suite**

  Run: `npm.cmd test`, `npm.cmd run test:ui`, and `npm.cmd run build`.

  Expected: existing unit tests, UI tests, and production build all pass.

- [x] **Step 3: Check the final diff**

  Run: `git diff --check` and `git status --short`.

  Expected: no whitespace errors; only the pull-refresh implementation, tests, and plan/spec files are changed.
