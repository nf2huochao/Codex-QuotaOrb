# v1.0.6 validation

Release base: published v1.0.5 (57c6af8118c51100fcef0825abca779a8eacd48c).

Scope: Guardian internal approval-review session filtering, regression coverage, version metadata, and bilingual release notes. Unreleased Doumeow development changes are excluded.

Local checks on Windows, 2026-10-15:

- Backend: `cargo test --lib --locked --offline` — 94 passed, 0 failed, 2 ignored. The ignored tests read real session logs and were not run.
- Frontend: `npm test` — 43 passed.
- Browser UI: `npm run test:ui` — 13 passed.
- Production frontend: `npm run build` — passed.
- Patch whitespace: `git -c core.whitespace=cr-at-eol diff --check` — passed.

These checks do not constitute an installed desktop or macOS runtime test. Signed updater artifacts and platform packages are produced by the existing release workflows after an approved tag is pushed. No running installation was replaced during preparation.
