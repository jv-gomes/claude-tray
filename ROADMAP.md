# Claude Tray — Implementation Roadmap

> **Local file — not committed.** Ignored via `.gitignore` alongside `claude.md` and `.claude/`.

A task-by-task plan to take `claude-tray` from an empty repo to a polished, open-source,
cross-platform Rust tray app. Tasks are deliberately **tiny** — most should be one sitting,
many one commit. Each has a stable ID so you can reference it in commits and issues
(`feat(icon): render battery states (P5-04)`).

---

## Legend

| Marker | Meaning |
| ------ | ------- |
| `[ ]`  | Not started |
| `[~]`  | In progress |
| `[x]`  | Done |
| 🔴     | Blocker — later work depends on it |
| 🧪     | Spike / research, output is a decision not code |
| ⚠️     | Carries legal, security, or stability risk |

---

## Milestones

| Milestone | Meaning | Phases |
| --------- | ------- | ------ |
| **M0 — Scaffolding** | Repo is a good open-source citizen; `cargo build` works | P0, P1 |
| **M1 — Walking skeleton** | A tray icon appears and shows *fake* usage data | P2, P5, P6 |
| **M2 — Real data** | Reads genuine usage from a real provider | P3, P4, P8 |
| **M3 — Feature complete** | Tooltip, menu, notifications, config, history | P7, P9, P10, P11, P12 |
| **M4 — Trustworthy** | Tests, CI, security review, docs | P13, P14, P16, P17 |
| **M5 — v1.0.0** | Installable on all three platforms from a release page | P15, P18, P19, P20 |
| **M6+** | Post-1.0 ecosystem | P21+ |

---

## Guiding constraints

Keep these visible; they are the tie-breakers for every design decision.

- **Idle cost budget:** < 15 MB RSS, < 0.1% CPU when idle. Measured, not assumed (P19).
- **Startup budget:** tray icon visible in < 300 ms cold.
- **Never block the UI thread.** All I/O is async; the tray event loop only paints.
- **Fail quiet, log loud.** A failed refresh degrades the icon to "unknown", never a popup storm.
- **Zero secrets on disk in plaintext.** OS keyring only.
- **No telemetry.** Ever. It's a privacy tool for a privacy-sensitive signal.
- **Read-only against Claude.** The app observes usage; it never spends quota.

---

# Phase 0 — Repository foundation & OSS hygiene

Do this before any code. An open repo is judged on these files first.

### Legal & identity
- [x] **P0-01** Add `LICENSE` with the MIT text, correct year and copyright holder.
- [x] **P0-02** Decide and document whether contributions are MIT-only or dual MIT/Apache-2.0 (Rust ecosystem norm is dual — pick one now, changing later requires contributor consent). 🧪
- [x] **P0-03** Add `NOTICE` or a licence header policy if dual-licensing.
- [x] **P0-04** ~~Reserve the crate name on crates.io by publishing a `0.0.0` placeholder.~~ **Deferred to P1-28** (2026-07-30) — publishing before the manifests exist would permanently claim the name with empty metadata. Publish is irreversible, so it waits for real `Cargo.toml` metadata.
- [x] **P0-05** Confirm the project name doesn't imply official Anthropic affiliation; add a disclaimer line to `README.md`. ⚠️

### Core docs
- [x] **P0-06** Rewrite `README.md`: one-sentence pitch, screenshot placeholder, install, build-from-source, status badge row.
- [x] **P0-07** Add a "Not affiliated with Anthropic" disclaimer near the top of `README.md`. ⚠️
- [x] **P0-08** Add a **Project status** badge (`alpha` / `experimental`) so users calibrate expectations.
- [x] **P0-09** Write `CONTRIBUTING.md`: dev setup, per-OS system dependencies, how to run, how to test, commit convention, PR checklist.
- [x] **P0-10** Write `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1) with a real contact address.
- [x] **P0-11** Write `SECURITY.md`: supported versions, private disclosure channel, expected response time.
- [x] **P0-12** Create `CHANGELOG.md` seeded with an `## [Unreleased]` section (Keep a Changelog format).
- [x] **P0-13** Add `docs/ARCHITECTURE.md` stub — the crate graph and data flow, one diagram.
- [x] **P0-14** Add `docs/adr/0000-template.md` for Architecture Decision Records.
- [x] **P0-15** Write ADR-0001: "Rust + `tray-icon` for the tray layer" with alternatives considered.
- [x] **P0-16** Write ADR-0002: "Provider abstraction for usage data" (see P3).

### GitHub configuration
- [x] **P0-17** Add `.github/ISSUE_TEMPLATE/bug_report.yml` with OS, desktop environment, version, and log fields.
- [x] **P0-18** Add `.github/ISSUE_TEMPLATE/feature_request.yml`.
- [x] **P0-19** Add `.github/ISSUE_TEMPLATE/config.yml` pointing questions to Discussions.
- [x] **P0-20** Add `.github/PULL_REQUEST_TEMPLATE.md` with a self-review checklist.
- [x] **P0-21** Add `.github/dependabot.yml` for `cargo` and `github-actions` ecosystems, weekly.
- [x] **P0-22** Add `CODEOWNERS`.
- [x] **P0-23** ~~Add `.github/FUNDING.yml` (optional).~~ **Declined** (2026-07-30) — no sponsor button for now. Trivial to add later, and better placed once the project works and has users.
- [x] **P0-24** Enable branch protection on `main`: require PR, require green CI, no force-push.
- [x] **P0-25** Enable GitHub secret scanning and push protection.
- [x] **P0-26** Enable private vulnerability reporting in repo settings.
- [x] **P0-27** Create issue labels: `good first issue`, `help wanted`, `platform:linux|macos|windows`, `area:tray|icon|auth|provider`.
- [x] **P0-28** Add a `.github/workflows/` directory placeholder (filled in P14).

### Repo mechanics
- [x] **P0-29** Add `.gitignore` covering Rust artifacts, OS junk, secrets, and local planning files.
- [x] **P0-30** Add `.gitattributes`: force LF for text, mark `Cargo.lock` as generated, mark binary assets.
- [x] **P0-31** Add `.editorconfig` (4-space Rust, 2-space YAML/TOML, LF, final newline).
- [x] **P0-32** Verify `git status` is clean and that `claude.md`, `.claude/`, and `ROADMAP.md` are ignored. 🔴
- [x] **P0-33** Double-check no prior commit contains those files (`git log --all --name-only | grep -i claude.md`); if it does, decide on history rewrite before the repo goes public. ⚠️ 🔴
- [x] **P0-34** Adopt Conventional Commits; document the allowed scopes in `CONTRIBUTING.md`.
- [x] **P0-35** Decide the branching model (trunk-based with short-lived branches recommended) and document it.

---

# Phase 1 — Rust project skeleton & tooling

### Workspace
- [x] **P1-01** `cargo init` a workspace at the repo root with a virtual manifest.
- [x] **P1-02** Create crate `crates/claude-tray-core` (pure domain logic, **no I/O, no GUI**).
- [x] **P1-03** Create crate `crates/claude-tray-provider` (usage data sources).
- [x] **P1-04** Create crate `crates/claude-tray-icon` (icon rasterisation).
- [x] **P1-05** Create binary crate `crates/claude-tray` (composition root: tray, event loop, wiring).
- [x] **P1-06** Add `[workspace.package]` with shared `version`, `edition`, `license`, `repository`, `rust-version`.
- [x] **P1-07** Add `[workspace.dependencies]` and make every crate inherit from it.
- [x] **P1-08** Set an explicit MSRV in `rust-version` and document the support policy (e.g. "last 3 stable releases").
- [x] **P1-09** Add `rust-toolchain.toml` pinning `stable` with `rustfmt` and `clippy` components.

### Lints & formatting
- [x] **P1-10** Add `rustfmt.toml` (start with defaults + `imports_granularity`; avoid nightly-only options unless CI pins nightly for fmt).
- [x] **P1-11** Add `clippy.toml` (e.g. `msrv`, cognitive complexity threshold).
- [x] **P1-12** Add `[workspace.lints.rust]`: `unsafe_code = "forbid"`, `missing_docs = "warn"`, `rust_2018_idioms = "warn"`.
- [x] **P1-13** Add `[workspace.lints.clippy]`: `pedantic = "warn"`, `unwrap_used = "deny"`, `expect_used = "warn"`, `panic = "deny"` in library crates.
- [x] **P1-14** Carve out lint exceptions for test modules so `unwrap` stays ergonomic in tests.
- [x] **P1-15** Verify `cargo clippy --workspace --all-targets -- -D warnings` is green on the empty skeleton. 🔴

### Supply chain
- [x] **P1-16** Add `deny.toml` for `cargo-deny`: licence allowlist, duplicate-version warnings, advisory DB, source allowlist.
- [x] **P1-17** Run `cargo deny check` locally and resolve findings.
- [x] **P1-18** Commit `Cargo.lock` (this is a binary crate — always commit it).
- [x] **P1-19** Document how to run `cargo audit` locally in `CONTRIBUTING.md`.
- [x] **P1-20** Add a dependency policy to `CONTRIBUTING.md`: justify every new dep, prefer `rustls` over OpenSSL, avoid heavy transitive trees.

### Release profile
- [x] **P1-21** Add `[profile.release]`: `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`, `panic = "abort"`.
- [x] **P1-22** Add a `[profile.dist]` inheriting release with `opt-level = "z"` and compare binary size. 🧪
- [x] **P1-23** Record baseline release binary size and cold-start time in `docs/PERFORMANCE.md`.

### Local ergonomics
- [x] **P1-24** Add a `justfile` (or `Makefile.toml`) with `fmt`, `lint`, `test`, `run`, `check-all`, `audit`.
- [x] **P1-25** Add a `just check-all` target that mirrors CI exactly, so contributors can pre-flight locally.
- [x] **P1-26** Optional: add a `pre-commit` config running `cargo fmt --check` and `cargo clippy`.
- [x] **P1-27** Document per-OS build prerequisites in `CONTRIBUTING.md` (Linux: `libayatana-appindicator3-dev`, `libgtk-3-dev`, `libxdo-dev`; macOS: Xcode CLT; Windows: MSVC toolchain).
- [ ] **P1-28** Reserve `claude-tray` on crates.io: publish a `0.0.0` placeholder once the manifests carry real metadata (was P0-04). ⚠️ **Irreversible** — versions can be yanked but never deleted, and the name is claimed permanently. Requires the maintainer's own `cargo login` token. Run `cargo publish --dry-run` first. 🔴 *depends on P1-06, P15-24*

---

# Phase 2 — Core domain model (`claude-tray-core`)

Pure, dependency-light, 100% unit-testable. Build this *before* touching any GUI or network code.

### Types
- [x] **P2-01** Define `UsageSnapshot { captured_at, plan, windows, model, source }`.
- [x] **P2-02** Define `UsageWindow { kind, used, limit, resets_at }` where `kind` is session / weekly / monthly.
- [x] **P2-03** Define `Percent` as a newtype clamped to `0.0..=100.0` with a fallible constructor.
- [x] **P2-04** Define `Plan` enum: `Free`, `Pro`, `Max`, `Team`, `Api`, `Unknown(String)`.
- [x] **P2-05** Define `UsageState` enum: `Unknown`, `Loading`, `Ok(UsageSnapshot)`, `Stale(UsageSnapshot, since)`, `Error(ErrorKind)`.
- [x] **P2-06** Define `ErrorKind`: `NotAuthenticated`, `NetworkUnavailable`, `RateLimited`, `ProviderUnavailable`, `ParseFailed`, `Internal`.
- [x] **P2-07** Derive `serde::{Serialize, Deserialize}` on the persistable types only.
- [x] **P2-08** Use `time` or `chrono` consistently for timestamps — decide once, document in an ADR. 🧪
- [x] **P2-09** Store all timestamps as UTC internally; convert to local only at the display boundary.

### Behaviour
- [x] **P2-10** Implement `UsageSnapshot::remaining_percent()` picking the *most constrained* window.
- [x] **P2-11** Implement `time_until_reset()` returning `Option<Duration>`.
- [x] **P2-12** Implement `humanize_duration()` → `"1h 48m"`, `"12m"`, `"<1m"`, `"—"`.
- [x] **P2-13** Define `ColorState` enum `{ Green, Yellow, Orange, Red, Unknown }`.
- [x] **P2-14** Implement `ColorState::from_percent()` with the thresholds from the spec (75/40/15).
- [x] **P2-15** Make thresholds injectable via a `Thresholds` struct rather than hardcoded constants.
- [x] **P2-16** Implement `BatteryLevel::from_percent()` quantising to the 10 discrete icon steps.
- [x] **P2-17** Add hysteresis so a value oscillating on a boundary doesn't flicker the icon.
- [x] **P2-18** Implement `is_stale(now, max_age)` for grey-out logic.

### Notification rules
- [x] **P2-19** Define `NotificationRule { threshold_percent, enabled }`.
- [x] **P2-20** Implement an edge-triggered `NotificationEngine`: fires only on *downward* crossing.
- [x] **P2-21** Add per-rule cooldown so a flapping value can't spam.
- [x] **P2-22** Reset the fired-state when a quota window resets.
- [x] **P2-23** Detect a quota reset by comparing `resets_at` between snapshots.
- [x] **P2-24** Emit a `QuotaReset` notification event.

### Tests
- [x] **P2-25** Unit-test `ColorState` boundaries exactly at 75, 74.9, 40, 39.9, 15, 14.9, 0, 100.
- [x] **P2-26** Unit-test hysteresis with a hand-written oscillating sequence.
- [x] **P2-27** Unit-test the notification engine: crossing down fires, crossing up doesn't, re-crossing after reset fires again.
- [x] **P2-28** Property-test `Percent` construction rejects NaN, negatives, and > 100.
- [x] **P2-29** Property-test that `from_percent` is monotonic across the whole range.
- [x] **P2-30** Test `humanize_duration` across seconds, minutes, hours, days, and zero.
- [x] **P2-31** Test reset-detection against a snapshot pair spanning a window boundary.
- [x] **P2-32** Confirm `claude-tray-core` has zero I/O dependencies (`cargo tree` review). 🔴

---

# Phase 3 — Usage data providers (`claude-tray-provider`)

The single riskiest area. Design for the data source changing under you.

### Research spikes — do these first, write findings down
- [ ] **P3-01** 🧪 Inventory `~/.claude/`: what files exist, what they contain, which are stable across versions.
- [ ] **P3-02** 🧪 Determine whether local Claude Code session files expose a usable quota/limit signal, or only token counts.
- [ ] **P3-03** 🧪 Investigate the documented Anthropic Admin usage/cost report endpoints — what they cover, what auth they need, whether they apply to subscription plans at all.
- [ ] **P3-04** 🧪 Determine what the `/usage` view in Claude Code reads from, and whether that path is reusable.
- [ ] **P3-05** 🧪 ⚠️ Evaluate claude.ai internal endpoints: stability, ToS position, rate limits, bot protection. **Assume this is unsupported and may break or be disallowed.**
- [ ] **P3-06** 🧪 Read Anthropic's Terms of Service and Usage Policies on automated access; write the conclusion into `docs/DATA-SOURCES.md`. ⚠️ 🔴
- [ ] **P3-07** Write `docs/DATA-SOURCES.md`: for each candidate source — availability, plan coverage, fields provided, fragility, legal standing.
- [ ] **P3-08** Decide the **default** provider for v1.0 based on P3-07. 🔴
- [ ] **P3-09** Write ADR-0003 recording that decision and its rejected alternatives.

### Abstraction
- [x] **P3-10** Define `#[async_trait] trait UsageProvider { fn id(&self); async fn fetch(&self) -> Result<UsageSnapshot, ProviderError>; async fn health(&self) -> Health; }`.
- [x] **P3-11** Define `ProviderError` with a `retryable: bool` and an optional `retry_after`.
- [x] **P3-12** Define `ProviderCapabilities` (which windows/fields it can populate) so the UI hides what's unavailable.
- [x] **P3-13** Implement a `ProviderRegistry` resolving providers by id from config.
- [x] **P3-14** Implement provider **fallback chaining**: try in order, use the first success.
- [x] **P3-15** Surface *which* provider produced the current snapshot in `UsageSnapshot::source`.

### Mock provider (unblocks all UI work) 🔴
- [x] **P3-16** Implement `MockProvider` returning a fixed snapshot.
- [x] **P3-17** Add a scripted mode: walk a sequence of percentages on each fetch.
- [x] **P3-18** Add fault injection modes: timeout, 401, 429, malformed payload.
- [x] **P3-19** Wire `MockProvider` behind a `--provider=mock` CLI flag and a `mock` cargo feature.
- [x] **P3-20** Add a `--simulate-percent=<n>` dev flag for screenshotting every icon state.

### Local-file provider
- [ ] **P3-21** Resolve the Claude config directory per-platform via `directories`, honouring `CLAUDE_CONFIG_DIR`.
- [ ] **P3-22** Implement read-only, non-locking parsing of the relevant local files.
- [ ] **P3-23** Treat every field as optional; never panic on an unexpected shape. 🔴
- [ ] **P3-24** Version-tag the parser and log loudly when the shape is unrecognised.
- [ ] **P3-25** Aggregate token counts into a `UsageWindow` where limits are known.
- [ ] **P3-26** Handle "file absent" as `NotAuthenticated`, not as an error.
- [ ] **P3-27** Add fixture files under `tests/fixtures/` and snapshot-test the parser.
- [ ] **P3-28** Scrub all fixtures of real tokens, account ids, and project paths. ⚠️ 🔴

### HTTP provider scaffolding
- [ ] **P3-29** Add `reqwest` with `rustls-tls`, default features off. 🔴
- [ ] **P3-30** Build a single shared `Client` with connection pooling and a sane `User-Agent` identifying the app and version.
- [ ] **P3-31** Set connect and total request timeouts.
- [ ] **P3-32** Implement exponential backoff with jitter for retryable errors.
- [ ] **P3-33** Honour `Retry-After` on 429 responses. 🔴
- [ ] **P3-34** Implement a circuit breaker: after N consecutive failures, back off hard and show `Error` state.
- [ ] **P3-35** Never log response bodies at default level; redact tokens in all logs. ⚠️ 🔴
- [ ] **P3-36** Add `wiremock`-based tests covering 200 / 401 / 429 / 500 / timeout / garbage-body.
- [ ] **P3-37** Add a conditional-request path (ETag / `If-None-Match`) if the source supports it, to reduce load.

### Provider plugin boundary
- [ ] **P3-38** Gate any unofficial/scraping provider behind a non-default cargo feature. ⚠️
- [ ] **P3-39** Require an explicit config opt-in *and* a first-run warning dialog before it activates. ⚠️
- [ ] **P3-40** Document clearly in the README which providers are supported vs best-effort.

---

# Phase 4 — Authentication & credential storage

### Design
- [ ] **P4-01** 🧪 Enumerate viable auth paths given P3-08: reuse existing local credentials, OAuth device/browser flow, or user-supplied API key.
- [ ] **P4-02** Write ADR-0004 on the chosen auth model.
- [ ] **P4-03** Define an `AuthProvider` trait: `status()`, `login()`, `logout()`, `refresh()`.
- [ ] **P4-04** Define `AuthStatus { Anonymous, Valid { expires_at }, Expired, Revoked }`.

### Keyring integration
- [ ] **P4-05** Add the `keyring` crate; verify it resolves Secret Service, Keychain, and Windows Credential Manager.
- [ ] **P4-06** Implement `store_credential` / `load_credential` / `delete_credential` behind a `SecretStore` trait.
- [ ] **P4-07** Implement an in-memory `SecretStore` for tests. 🔴
- [ ] **P4-08** Handle a locked or absent keyring daemon gracefully (common on minimal Linux WMs).
- [ ] **P4-09** Provide a documented fallback for headless Linux and print exactly what the trade-off is. ⚠️
- [ ] **P4-10** Never write a credential to the config file, the log, or the cache. Add a test asserting this. 🔴
- [ ] **P4-11** Zeroize secret buffers after use (`zeroize` crate) where practical.

### Token lifecycle
- [ ] **P4-12** Parse token expiry and refresh proactively before it lapses.
- [ ] **P4-13** Serialise refreshes so concurrent fetches can't double-refresh.
- [ ] **P4-14** On refresh failure, transition to `NotAuthenticated` and update the tray icon.
- [ ] **P4-15** Show a "Sign in" item in the tray menu whenever unauthenticated.
- [ ] **P4-16** Implement "Sign out" — deletes the keyring entry and clears the cache.

### Browser-based flow (if applicable)
- [ ] **P4-17** Bind a `localhost` loopback listener on an ephemeral port for the callback.
- [ ] **P4-18** Implement PKCE (S256); never embed a client secret in the binary. ⚠️ 🔴
- [ ] **P4-19** Generate and verify the `state` parameter against CSRF.
- [ ] **P4-20** Open the system browser via the `open` crate.
- [ ] **P4-21** Serve a minimal "you can close this tab" success page.
- [ ] **P4-22** Time the flow out after ~5 minutes and shut the listener down.
- [ ] **P4-23** Bind to `127.0.0.1` only, never `0.0.0.0`. 🔴
- [ ] **P4-24** Test the callback handler against malformed and hostile query strings.

---

# Phase 5 — Icon rendering (`claude-tray-icon`)

### Approach
- [ ] **P5-01** 🧪 Decide: pre-baked PNGs per state, or runtime SVG rasterisation via `resvg` + `tiny-skia`. (Runtime rendering wins on flexibility and binary size; document the choice.)
- [ ] **P5-02** Write ADR-0005 on the icon pipeline.
- [ ] **P5-03** Define `IconSpec { level, color_state, theme, size, stale }`.

### Rendering
- [ ] **P5-04** Draw the battery shell: rounded body plus the terminal nub.
- [ ] **P5-05** Draw the fill proportional to `level`, snapped to the 10 discrete steps.
- [ ] **P5-06** Apply the colour from `ColorState`.
- [ ] **P5-07** Render to RGBA8 and expose the raw buffer the tray crate expects.
- [ ] **P5-08** Support 16/22/24/32/48/64 px and pick by platform + scale factor.
- [ ] **P5-09** Handle HiDPI: render at the device pixel ratio, never upscale a small raster. 🔴
- [ ] **P5-10** Cache rendered icons keyed by `IconSpec`; the cache is small and bounded.
- [ ] **P5-11** Add a distinct `Unknown` icon (dashed outline or `?`) for no-data.
- [ ] **P5-12** Add an `Error` icon variant (a small warning badge overlay).
- [ ] **P5-13** Add a `Loading` variant for the first fetch.
- [ ] **P5-14** Render `Stale` as the normal icon at reduced opacity.

### Themes
- [ ] **P5-15** Implement a `Theme` enum: `Auto`, `Light`, `Dark`, `Monochrome`, `SystemAccent`.
- [ ] **P5-16** Detect the system light/dark preference per platform.
- [ ] **P5-17** Subscribe to theme-change events and re-render live where the platform allows.
- [ ] **P5-18** Build a macOS template-image variant (monochrome + alpha) so the menu bar tints it correctly. 🔴
- [ ] **P5-19** Ensure contrast holds on both light and dark panels; check the worst case (red on dark).
- [ ] **P5-20** Add a colour-blind-safe palette option (don't rely on hue alone — the fill level already carries the signal).

### Verification
- [ ] **P5-21** Add a `cargo run --example icon-sheet` dev tool dumping every state to PNG.
- [ ] **P5-22** Golden-image tests comparing rendered output against committed references.
- [ ] **P5-23** Allow a small per-pixel tolerance so rasteriser updates don't break CI spuriously.
- [ ] **P5-24** Benchmark a single render; assert it stays under ~2 ms.
- [ ] **P5-25** Visually verify on a real Linux panel, the macOS menu bar, and the Windows notification area. 🔴

---

# Phase 6 — Tray integration (`claude-tray` binary)

### Event loop
- [ ] **P6-01** Add `tray-icon` and the matching windowing/event-loop crate.
- [ ] **P6-02** 🧪 Confirm the platform requirement that tray objects live on the main thread (macOS and Windows both care). 🔴
- [ ] **P6-03** Create the event loop and keep the app alive with no visible window.
- [ ] **P6-04** Show a static placeholder icon and confirm it appears on all three platforms. 🔴
- [ ] **P6-05** Define an `AppEvent` enum bridging async workers → the UI thread.
- [ ] **P6-06** Wire a channel from the tokio runtime into the event loop's user-event proxy.
- [ ] **P6-07** Spawn the tokio runtime on a background thread; never block the loop on it. 🔴
- [ ] **P6-08** Implement clean shutdown: cancel tasks, flush state, drop the tray handle.
- [ ] **P6-09** Handle SIGINT/SIGTERM on Unix and the console control handler on Windows.

### Platform specifics
- [ ] **P6-10** Linux: initialise GTK correctly before creating the tray.
- [ ] **P6-11** Linux: 🧪 verify behaviour under GNOME (needs an AppIndicator extension), KDE Plasma, XFCE, and a bare Sway/Waybar setup. 🔴
- [ ] **P6-12** Linux: detect "no StatusNotifier host present" and print an actionable hint instead of failing silently. 🔴
- [ ] **P6-13** Linux: test on Wayland *and* X11.
- [ ] **P6-14** macOS: mark the app `LSUIElement` / `NSApplication` accessory so no Dock icon appears. 🔴
- [ ] **P6-15** macOS: verify menu-bar behaviour in both appearances and with the notch.
- [ ] **P6-16** Windows: hide the console window in release builds (`windows_subsystem = "windows"`). 🔴
- [ ] **P6-17** Windows: handle `TaskbarCreated` to re-add the icon after Explorer restarts. 🔴
- [ ] **P6-18** Windows: verify behaviour when the icon is hidden in the overflow area.

### Single instance
- [ ] **P6-19** Implement a single-instance guard (lock file, named mutex, or a crate).
- [ ] **P6-20** On a second launch, surface the existing instance and exit.
- [ ] **P6-21** Ensure a stale lock from a crash doesn't permanently block startup. 🔴

### Autostart
- [ ] **P6-22** Implement "Start at login" for Linux (XDG autostart `.desktop`).
- [ ] **P6-23** Implement it for macOS (`SMAppService` / LaunchAgent).
- [ ] **P6-24** Implement it for Windows (Run registry key or Startup shortcut).
- [ ] **P6-25** Expose it as a toggle in the tray menu, reflecting the true current state.
- [ ] **P6-26** Make it fully reversible and test the uninstall path.

---

# Phase 7 — Tooltip & tray menu

### Tooltip
- [ ] **P7-01** Implement a `render_tooltip(&UsageState) -> String` in core, fully unit-testable.
- [ ] **P7-02** Match the spec layout: plan, remaining, current window, resets in, model, last updated.
- [ ] **P7-03** Format "last updated" in local time.
- [ ] **P7-04** Omit sections the active provider can't populate (per `ProviderCapabilities`).
- [ ] **P7-05** Show a clear, actionable message in `Error` and `NotAuthenticated` states.
- [ ] **P7-06** 🧪 Check per-platform tooltip constraints — Windows historically truncates around 128 chars. 🔴
- [ ] **P7-07** Provide a compact tooltip variant for platforms with a hard limit.
- [ ] **P7-08** Update the tooltip on every state change, not on a timer.
- [ ] **P7-09** Snapshot-test the tooltip for each `UsageState`.

### Menu
- [ ] **P7-10** Build the menu: a disabled header line showing the plan and percentage.
- [ ] **P7-11** Add "Refresh now".
- [ ] **P7-12** Add "Open Claude" (launches the web app in the browser).
- [ ] **P7-13** Add a "Settings" submenu.
- [ ] **P7-14** Settings → refresh interval options (15 s / 30 s / 1 m / 5 m).
- [ ] **P7-15** Settings → theme selection.
- [ ] **P7-16** Settings → notification thresholds toggles.
- [ ] **P7-17** Settings → "Start at login" toggle.
- [ ] **P7-18** Add "Open config file" revealing it in the file manager.
- [ ] **P7-19** Add "Open log file".
- [ ] **P7-20** Add "Copy diagnostics" — version, OS, provider, last error, **redacted**. 🔴
- [ ] **P7-21** Add "Sign in" / "Sign out", shown conditionally.
- [ ] **P7-22** Add "About" with version, commit hash, and licence.
- [ ] **P7-23** Add "Quit".
- [ ] **P7-24** Rebuild only the menu items whose state changed; don't recreate the whole menu each tick.
- [ ] **P7-25** Debounce "Refresh now" so a click storm can't hammer the provider. 🔴
- [ ] **P7-26** Show a transient "Refreshing…" state while a manual refresh is in flight.

---

# Phase 8 — Refresh scheduler & resilience

- [ ] **P8-01** Implement a `Scheduler` driving fetches on the configured interval.
- [ ] **P8-02** Default to 30 s, per the spec.
- [ ] **P8-03** Clamp the configurable interval to a sane minimum (≥ 10 s) to protect the upstream. 🔴
- [ ] **P8-04** Add jitter to the interval so many installs don't sync up into a thundering herd.
- [ ] **P8-05** Support an out-of-band manual refresh that resets the timer.
- [ ] **P8-06** Guarantee only one in-flight fetch at a time.
- [ ] **P8-07** Cancel an in-flight fetch on quit.
- [ ] **P8-08** Back off automatically when consecutive failures accumulate.
- [ ] **P8-09** Detect system sleep/resume and refresh immediately on wake. 🧪
- [ ] **P8-10** Detect network-down and pause polling instead of burning retries.
- [ ] **P8-11** Resume immediately when the network returns.
- [ ] **P8-12** Slow the interval when the machine has been idle for a long time (optional battery-friendliness).
- [ ] **P8-13** Mark data stale after `3 × interval` without a successful fetch.
- [ ] **P8-14** Keep showing the last good snapshot while stale rather than dropping to `Unknown`. 🔴
- [ ] **P8-15** Test the scheduler with a mocked clock — no `sleep`-based tests. 🔴
- [ ] **P8-16** Test that a provider failure never kills the scheduler task.

---

# Phase 9 — Notifications

- [ ] **P9-01** Add `notify-rust` (Linux/Windows); evaluate a macOS-specific path.
- [ ] **P9-02** Define a `Notifier` trait so it can be mocked in tests. 🔴
- [ ] **P9-03** Implement threshold notifications at 50 / 25 / 10 / 5 % remaining.
- [ ] **P9-04** Make the threshold list fully user-configurable.
- [ ] **P9-05** Implement the quota-reset notification.
- [ ] **P9-06** Add a global "notifications enabled" master switch.
- [ ] **P9-07** Add a "quiet hours" window (optional).
- [ ] **P9-08** Include the app icon in the notification.
- [ ] **P9-09** Add a click action opening the Claude web app where the platform supports actions.
- [ ] **P9-10** macOS: 🧪 determine whether notifications require a signed, bundled `.app` — they generally do. 🔴
- [ ] **P9-11** Linux: degrade gracefully when no notification daemon is running.
- [ ] **P9-12** Windows: verify toast behaviour, including under Focus Assist.
- [ ] **P9-13** Never notify on the very first snapshot after launch (avoids a startup burst). 🔴
- [ ] **P9-14** Suppress notifications while in `Error` or `Unknown` state.
- [ ] **P9-15** Unit-test the full firing matrix with a mock notifier.

---

# Phase 10 — Configuration

- [ ] **P10-01** Choose TOML and resolve the config path via `directories` (XDG on Linux, `Application Support` on macOS, `AppData\Roaming` on Windows).
- [ ] **P10-02** Define a `Config` struct with `#[serde(default)]` on every field. 🔴
- [ ] **P10-03** Write a default config file on first run, richly commented.
- [ ] **P10-04** Add a `version` field to the config for future migrations.
- [ ] **P10-05** Implement a migration hook keyed on that version.
- [ ] **P10-06** Validate on load: interval bounds, threshold ordering, valid theme names.
- [ ] **P10-07** On invalid config, log the specific problem, fall back to defaults, and keep running — never exit. 🔴
- [ ] **P10-08** Support env-var overrides (`CLAUDE_TRAY_*`) for CI and debugging.
- [ ] **P10-09** Support CLI flags overriding both, with a documented precedence order.
- [ ] **P10-10** Add `--config <path>`.
- [ ] **P10-11** Add `--print-config` dumping the effective merged config.
- [ ] **P10-12** Implement live config reload via file watching (`notify` crate).
- [ ] **P10-13** Debounce file-watch events (editors write multiple times per save). 🔴
- [ ] **P10-14** Persist tray-menu setting changes back to the config file atomically (temp file + rename). 🔴
- [ ] **P10-15** Never round-trip-destroy user comments — or clearly document that programmatic writes reformat the file.
- [ ] **P10-16** Document every option in `docs/CONFIGURATION.md`.
- [ ] **P10-17** Test loading an empty, a partial, a malformed, and a future-version config.

---

# Phase 11 — Local persistence & history

- [ ] **P11-01** Define a cache file for the last known good snapshot.
- [ ] **P11-02** Load the cache at startup so the icon is meaningful before the first fetch. 🔴
- [ ] **P11-03** Write the cache atomically after each successful fetch.
- [ ] **P11-04** Ignore a corrupt cache file silently and continue.
- [ ] **P11-05** Store the cache in the OS cache dir, not the config dir.
- [ ] **P11-06** Design a history store — start with an append-only JSONL file; only reach for SQLite if the data justifies it. 🧪
- [ ] **P11-07** Append one record per successful fetch.
- [ ] **P11-08** Cap history size and rotate or prune old entries. 🔴
- [ ] **P11-09** Add a config option to disable history entirely (privacy). 🔴
- [ ] **P11-10** Add "Clear history" to the menu.
- [ ] **P11-11** Ensure history contains **no** tokens, account identifiers, or prompt content. ⚠️ 🔴
- [ ] **P11-12** Add an "Export history to CSV" action.
- [ ] **P11-13** Document the on-disk layout in `docs/DATA-STORAGE.md`, including exactly what is and isn't stored.

---

# Phase 12 — Logging, diagnostics & error UX

- [ ] **P12-01** Add `tracing` + `tracing-subscriber`.
- [ ] **P12-02** Default to `warn`; allow `RUST_LOG` / `--verbose` to raise it.
- [ ] **P12-03** Write logs to a rotating file in the OS log/cache directory.
- [ ] **P12-04** Cap total log size and file count. 🔴
- [ ] **P12-05** Add a redaction layer scrubbing anything token-shaped before it reaches a sink. ⚠️ 🔴
- [ ] **P12-06** Test the redaction layer against realistic token formats. 🔴
- [ ] **P12-07** Instrument each fetch with a span carrying provider id and duration.
- [ ] **P12-08** Log state transitions at `info`.
- [ ] **P12-09** Use `thiserror` for library errors, `anyhow` only at the binary boundary.
- [ ] **P12-10** Ensure every error surfaced to the user names a next action ("Sign in", "Check your network").
- [ ] **P12-11** Add a `--doctor` subcommand: checks keyring access, config validity, provider health, tray host presence.
- [ ] **P12-12** Have `--doctor` output be safe to paste into an issue (already redacted). 🔴
- [ ] **P12-13** Install a panic hook that logs, notifies the user, and exits cleanly rather than vanishing. 🔴
- [ ] **P12-14** Never let a background task panic take the whole app down silently.

---

# Phase 13 — Testing strategy

- [ ] **P13-01** Set a coverage target for `claude-tray-core` (aim ≥ 90%) and document it.
- [ ] **P13-02** Add `cargo-llvm-cov` and a `just coverage` target.
- [ ] **P13-03** Add integration tests for the provider chain against `wiremock`.
- [ ] **P13-04** Add `insta` snapshot tests for tooltip and menu-label rendering.
- [ ] **P13-05** Add `proptest` for the percentage/level/colour pipeline.
- [ ] **P13-06** Add a test asserting no secret ever reaches a log sink. 🔴
- [ ] **P13-07** Add a test asserting the config round-trips losslessly.
- [ ] **P13-08** Add a headless smoke test: build, launch with the mock provider, verify a clean exit.
- [ ] **P13-09** Set up `xvfb` in Linux CI for anything needing a display.
- [ ] **P13-10** Add `cargo test --workspace --all-features` to CI. 🔴
- [ ] **P13-11** Add a doc-test pass (`cargo test --doc`).
- [ ] **P13-12** Write a manual QA checklist in `docs/QA.md` for things CI can't cover (real tray appearance, real notifications).
- [ ] **P13-13** Run the manual checklist on each of the three platforms before every release. 🔴

---

# Phase 14 — CI/CD

### CI workflow
- [ ] **P14-01** `ci.yml` triggering on push to `main` and on all PRs.
- [ ] **P14-02** Job: `cargo fmt --all --check`.
- [ ] **P14-03** Job: `cargo clippy --workspace --all-targets --all-features -- -D warnings`. 🔴
- [ ] **P14-04** Job: `cargo test` on a `{ubuntu, macos, windows}` matrix. 🔴
- [ ] **P14-05** Job: `cargo build --release` on the same matrix to catch platform-specific breakage. 🔴
- [ ] **P14-06** Install Linux system deps in CI (GTK, AppIndicator, xdo).
- [ ] **P14-07** Add `Swatinem/rust-cache` for build caching.
- [ ] **P14-08** Job: MSRV check building with the pinned minimum toolchain.
- [ ] **P14-09** Job: `cargo deny check` (advisories, licences, bans, sources).
- [ ] **P14-10** Scheduled weekly `cargo audit` run that opens an issue on failure.
- [ ] **P14-11** Job: `cargo doc --no-deps` with `-D warnings` to catch broken doc links.
- [ ] **P14-12** Job: typo check (`typos-cli`).
- [ ] **P14-13** Pin all GitHub Actions to a commit SHA, not a floating tag. ⚠️ 🔴
- [ ] **P14-14** Set `permissions: contents: read` at the workflow level by default. 🔴
- [ ] **P14-15** Make CI required for merge in branch protection.
- [ ] **P14-16** Add a concurrency group cancelling superseded PR runs.

### Release workflow
- [ ] **P14-17** `release.yml` triggered on `v*` tags.
- [ ] **P14-18** Cross-compile targets: `x86_64` and `aarch64` for Linux (gnu), macOS, and Windows.
- [ ] **P14-19** Evaluate `cargo-dist` to generate the whole release pipeline. 🧪
- [ ] **P14-20** Generate SHA-256 checksums for every artifact. 🔴
- [ ] **P14-21** Publish artifacts to a GitHub Release with generated notes.
- [ ] **P14-22** Generate an SBOM (`cargo-sbom` / CycloneDX) and attach it.
- [ ] **P14-23** Add build provenance attestation. 
- [ ] **P14-24** Automate `CHANGELOG.md` updates from conventional commits.
- [ ] **P14-25** Add a dry-run release path testable without cutting a real tag. 🔴

---

# Phase 15 — Packaging & distribution

### Linux
- [ ] **P15-01** Ship a `.desktop` file with correct categories and icon name.
- [ ] **P15-02** Install icons into the hicolor theme at standard sizes.
- [ ] **P15-03** Build a `.deb` (`cargo-deb`) with the right runtime dependencies declared.
- [ ] **P15-04** Build an `.rpm` (`cargo-generate-rpm`).
- [ ] **P15-05** Write and submit an AUR `PKGBUILD` (`claude-tray` and `-bin`).
- [ ] **P15-06** Evaluate Flatpak packaging and the keyring/portal implications. 🧪
- [ ] **P15-07** Evaluate an AppImage build. 🧪
- [ ] **P15-08** Add a Nix flake (optional, but the Nix crowd will ask).
- [ ] **P15-09** Test install → run → uninstall on Debian/Ubuntu, Fedora, and Arch. 🔴

### macOS
- [ ] **P15-10** Produce a proper `.app` bundle with `Info.plist` (`LSUIElement = true`).
- [ ] **P15-11** Build a universal binary (`lipo` x86_64 + arm64).
- [ ] **P15-12** Codesign with a Developer ID certificate. ⚠️ 🔴
- [ ] **P15-13** Notarize and staple the ticket. ⚠️ 🔴
- [ ] **P15-14** Package as a `.dmg` with a drag-to-Applications layout.
- [ ] **P15-15** Store signing credentials as GitHub secrets, never in the repo. ⚠️ 🔴
- [ ] **P15-16** Write a Homebrew cask.
- [ ] **P15-17** Document the unsigned-build workaround for contributors without a paid developer account.

### Windows
- [ ] **P15-18** Embed an application icon and a version resource in the `.exe`.
- [ ] **P15-19** Build an MSI or NSIS installer.
- [ ] **P15-20** Evaluate code signing (SmartScreen will flag unsigned builds). ⚠️ 🧪
- [ ] **P15-21** Submit a `winget` manifest.
- [ ] **P15-22** Add a Scoop manifest (optional).
- [ ] **P15-23** Verify install → autostart → uninstall leaves no orphaned registry keys. 🔴

### crates.io
- [ ] **P15-24** Fill in complete metadata: description, keywords, categories, readme, repository.
- [ ] **P15-25** Add `exclude` to keep assets and fixtures out of the published package.
- [ ] **P15-26** `cargo publish --dry-run` for every crate in dependency order. 🔴
- [ ] **P15-27** Publish the workspace crates in the correct order.

---

# Phase 16 — Documentation

- [ ] **P16-01** README: real screenshots or a GIF of the tray in all four colour states. 🔴
- [ ] **P16-02** README: per-platform install instructions with copy-pasteable commands.
- [ ] **P16-03** README: an explicit "What data does this read, and where does it go?" section. 🔴
- [ ] **P16-04** README: a clear statement that nothing leaves the machine except calls to Anthropic.
- [ ] **P16-05** README: supported-platform matrix with known caveats (GNOME extension requirement, etc.).
- [ ] **P16-06** `docs/INSTALL.md` with the long-form per-platform guide.
- [ ] **P16-07** `docs/CONFIGURATION.md` — every option, its default, and its effect.
- [ ] **P16-08** `docs/TROUBLESHOOTING.md` — icon not appearing, keyring locked, no notifications, auth expired.
- [ ] **P16-09** `docs/ARCHITECTURE.md` completed with a real crate/data-flow diagram.
- [ ] **P16-10** `docs/DATA-SOURCES.md` finalised from the P3 spikes.
- [ ] **P16-11** `docs/PRIVACY.md` — plain language, no legalese.
- [ ] **P16-12** `docs/FAQ.md`.
- [ ] **P16-13** Rustdoc on every public item in the library crates.
- [ ] **P16-14** A `RELEASING.md` runbook for maintainers.
- [ ] **P16-15** Enable GitHub Discussions and seed a "Show your setup" thread.
- [ ] **P16-16** Tag 5–10 genuinely approachable issues as `good first issue`.

---

# Phase 17 — Security & privacy hardening

- [ ] **P17-01** Confirm `unsafe_code = "forbid"` holds across the whole workspace (audit any exception). 🔴
- [ ] **P17-02** Full audit: no secret is ever logged, cached, exported, or put on the clipboard. ⚠️ 🔴
- [ ] **P17-03** Verify the "Copy diagnostics" output on a real authenticated session. 🔴
- [ ] **P17-04** Enforce TLS certificate validation everywhere; no "danger accept invalid certs" escape hatch. 🔴
- [ ] **P17-05** Pin the set of hosts the app is allowed to contact; refuse anything else. ⚠️
- [ ] **P17-06** Review file permissions on the config, cache, and log files (0600 where secrets could land).
- [ ] **P17-07** Review the atomic-write paths for symlink/TOCTOU issues.
- [ ] **P17-08** Bound every parser input; reject implausibly large payloads.
- [ ] **P17-09** Fuzz the provider response parsers (`cargo-fuzz`).
- [ ] **P17-10** Review all `unwrap`/`expect` in the binary crate; each must be provably unreachable or removed. 🔴
- [ ] **P17-11** Threat-model the browser auth callback listener (port hijack, CSRF, open redirect).
- [ ] **P17-12** Document the threat model in `docs/SECURITY-MODEL.md`.
- [ ] **P17-13** Run a full `cargo audit` + `cargo deny` pass and resolve every finding before 1.0. 🔴
- [ ] **P17-14** Re-verify that `.gitignore` still excludes local planning files, and re-scan history for leaked secrets (`gitleaks`). ⚠️ 🔴

---

# Phase 18 — Accessibility & internationalisation

- [ ] **P18-01** Verify the icon is readable at 100%, 150%, and 200% display scaling.
- [ ] **P18-02** Verify all colour states are distinguishable under deuteranopia and protanopia simulation. 🔴
- [ ] **P18-03** Ensure fill level alone conveys the state, so colour is redundant rather than load-bearing. 🔴
- [ ] **P18-04** Provide a high-contrast icon variant.
- [ ] **P18-05** Ensure the tooltip text is exposed to screen readers where the platform allows.
- [ ] **P18-06** Extract all user-facing strings into a single module. 🔴
- [ ] **P18-07** Evaluate `fluent` / `rust-i18n` for translation. 🧪
- [ ] **P18-08** Add a `pt-BR` translation as the first non-English locale.
- [ ] **P18-09** Detect the system locale, with a config override.
- [ ] **P18-10** Localise durations and timestamps, not just labels.
- [ ] **P18-11** Document the translation contribution process in `CONTRIBUTING.md`.

---

# Phase 19 — Performance & resource budget

- [ ] **P19-01** Measure idle RSS on each platform; record it in `docs/PERFORMANCE.md`. 🔴
- [ ] **P19-02** Measure idle CPU over a 10-minute window.
- [ ] **P19-03** Measure cold-start time to first painted icon.
- [ ] **P19-04** Measure release binary size per platform.
- [ ] **P19-05** Profile the first 5 seconds of startup and cut the biggest cost.
- [ ] **P19-06** Audit dependencies for weight (`cargo bloat`, `cargo tree --duplicates`).
- [ ] **P19-07** Drop default features on every dependency and re-enable only what's used. 🔴
- [ ] **P19-08** Confirm zero allocations in the steady-state idle loop where practical.
- [ ] **P19-09** Confirm the icon cache prevents re-rendering on unchanged state. 🔴
- [ ] **P19-10** Measure the battery impact of a 30 s poll on a laptop; tune the default if it's material.
- [ ] **P19-11** Add the budget numbers to CI as a soft regression check (optional).

---

# Phase 20 — v1.0.0 release

- [ ] **P20-01** Freeze the feature set; open a `v1.0` milestone and triage everything else out.
- [ ] **P20-02** Close or defer every open bug labelled `blocker`.
- [ ] **P20-03** Run the full manual QA checklist on all three platforms. 🔴
- [ ] **P20-04** Verify a clean-machine install works from the published artifact, not a dev build. 🔴
- [ ] **P20-05** Verify uninstall is complete and leaves no credentials behind. 🔴
- [ ] **P20-06** Final docs proofread; check every link resolves.
- [ ] **P20-07** Final security pass (P17-13, P17-14). 🔴
- [ ] **P20-08** Finalise `CHANGELOG.md` for 1.0.0.
- [ ] **P20-09** Bump versions and tag `v1.0.0`.
- [ ] **P20-10** Verify the release workflow produced every artifact plus checksums. 🔴
- [ ] **P20-11** Publish to crates.io.
- [ ] **P20-12** Submit the package-manager manifests (AUR, Homebrew, winget).
- [ ] **P20-13** Write the announcement post.
- [ ] **P20-14** Prepare for the inbound issue wave: pin a "known issues" discussion.

---

# Phase 21+ — Post-1.0

Only start these once 1.0 is stable and the issue queue is calm.

### Statistics & history UI
- [ ] **P21-01** Design a lightweight history view (a small window, or an exported HTML report).
- [ ] **P21-02** Daily usage summary.
- [ ] **P21-03** Weekly usage summary.
- [ ] **P21-04** A sparkline of the last 24 h rendered directly into the tooltip or menu.
- [ ] **P21-05** "Predicted time until exhaustion" from the current burn rate.

### Multi-account
- [ ] **P21-06** Generalise credential storage to a keyed account list.
- [ ] **P21-07** Add an account switcher to the menu.
- [ ] **P21-08** Decide whether to show one aggregate icon or one icon per account. 🧪

### Ecosystem integrations
- [ ] **P21-09** Add a `--json` one-shot output mode for scripting. 🔴 *(enables everything below)*
- [ ] **P21-10** A Waybar custom module example in `contrib/`.
- [ ] **P21-11** A Polybar module example.
- [ ] **P21-12** An i3blocks / i3status-rust snippet.
- [ ] **P21-13** A tmux status-line snippet.
- [ ] **P21-14** A starship / shell-prompt snippet.
- [ ] **P21-15** A GNOME Shell extension (separate repo, linked from the README).
- [ ] **P21-16** A KDE Plasma widget.

### CLI companion
- [ ] **P21-17** `claude-tray status` printing current usage.
- [ ] **P21-18** `claude-tray watch` for a live terminal view.
- [ ] **P21-19** `claude-tray history --since 7d`.
- [ ] **P21-20** Shell completions for bash, zsh, fish, and PowerShell.
- [ ] **P21-21** A man page.

### Local HTTP API
- [ ] **P21-22** An opt-in localhost-only HTTP endpoint serving the current snapshot. ⚠️
- [ ] **P21-23** Bind `127.0.0.1` exclusively, with a configurable port, disabled by default. 🔴
- [ ] **P21-24** Add a Prometheus `/metrics` endpoint (this is the real reason people will want it).
- [ ] **P21-25** Document the API shape and stability guarantees.

### Plugin architecture
- [ ] **P21-26** 🧪 Decide the mechanism: dynamic libraries, WASM, or subprocess + JSON. (Subprocess is the safest starting point.)
- [ ] **P21-27** Define the provider plugin contract.
- [ ] **P21-28** Sandbox and document the trust model — a plugin sees your usage data. ⚠️ 🔴
- [ ] **P21-29** Ship a reference example plugin.

---

## Risk register

| # | Risk | Likelihood | Impact | Mitigation |
| - | ---- | ---------- | ------ | ---------- |
| R1 | No stable, permitted API exposes subscription usage | **High** | **Critical** | P3 spikes first; provider abstraction; ship with whatever source is legitimately available and be honest in the README |
| R2 | An unofficial endpoint breaks or is disallowed | High | High | Feature-gated, opt-in, clearly labelled unsupported (P3-38/39/40) |
| R3 | GNOME hides tray icons without a user-installed extension | Certain | Medium | Detect and print an actionable hint (P6-12); document prominently |
| R4 | macOS notifications and signing require a paid developer account | High | Medium | Decide early (P15-12); document the unsigned path for contributors |
| R5 | Linux keyring absent on minimal WMs | Medium | Medium | Graceful degradation with an explicit trade-off warning (P4-08/09) |
| R6 | Credential leakage via logs or diagnostics | Medium | **Critical** | Redaction layer + tests (P12-05/06, P17-02/03) |
| R7 | Polling load on the upstream service | Medium | Medium | Minimum interval clamp, jitter, backoff, conditional requests (P8-03/04) |
| R8 | Name implies official Anthropic affiliation | Medium | Medium | Prominent disclaimer (P0-05/07) |
| R9 | Local planning files leak into the public repo | Low | Medium | `.gitignore` + history scan (P0-32/33, P17-14) |

---

## Definition of Done (per task)

1. Code compiles clean on all three platforms.
2. `cargo fmt`, `cargo clippy -D warnings`, and `cargo test` all pass.
3. New logic has tests; new user-facing behaviour has docs.
4. No new `unwrap`/`expect` in library code.
5. No secret can reach a log, cache, or export path.
6. `CHANGELOG.md` updated under `[Unreleased]` for user-visible changes.
7. Commit follows Conventional Commits and references the task ID.

---

## Suggested execution order

The fastest path to something real on screen:

```
P0-01…P0-35   →  repo is presentable
P1-01…P1-27   →  workspace builds
P2-01…P2-32   →  domain model, fully tested, no I/O
P3-16…P3-20   →  mock provider  ← unblocks all UI work
P5-01…P5-25   →  icon rendering
P6-01…P6-21   →  tray shows a real, mock-driven icon   ★ M1 walking skeleton
P3-01…P3-15   →  research + real provider              ★ M2
P4, P8        →  auth + scheduler
P7, P9…P12    →  the rest of the UX                    ★ M3
P13, P14, P16, P17 → trust                             ★ M4
P15, P18…P20  →  ship                                  ★ M5
```

Doing the mock provider before the real one is the single highest-leverage
ordering choice here — it decouples every pixel of UI work from the unresolved
question of where the data actually comes from.
