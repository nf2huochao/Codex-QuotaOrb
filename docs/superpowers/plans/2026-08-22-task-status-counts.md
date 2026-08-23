# Task Status Counts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Show yellow, red, and green task counts in the middle capsule, using gray only when there are no tasks.

**Architecture:** Keep the existing task priority and snapshot model. Add a compact four-state count legend to the native floating island; the gray state is rendered only when all task counts are zero. Route app-server approval requests into the event stream and preserve an event-driven red state when the thread list still reports the task as active. The details view and double-click state machine remain unchanged.

**Tech Stack:** TypeScript, DOM templates, CSS, Vitest.

## Global Constraints

- Red means needs action, yellow means running, green means completed, and gray means no tasks.
- Counts exclude acknowledged completed tasks, matching existing UI behavior.
- Preserve the existing red > green > yellow > gray primary status priority.
- Do not change window transitions, pairing, or remote access.

---

### Task 1: Render status count legend

**Files:**
- Modify: `src/components/FloatingIsland.ts`
- Modify: `src/styles.css`
- Test: `src/components/FloatingIsland.test.ts`

- [x] Add a helper that counts unacknowledged tasks by `TaskStatus`.
- [x] Render compact status count badges in the task segment.
- [x] Use gray as the visible status dot only when all counts are zero.
- [x] Keep the existing primary status summary and priority.
- [x] Add tests for mixed counts and empty-state gray behavior.
- [x] Route JSON-RPC approval requests before response matching.
- [x] Preserve red approval state across active thread-list polling.
- [x] Add parser and poller regression tests.
- [x] Run frontend and backend tests plus production build.
