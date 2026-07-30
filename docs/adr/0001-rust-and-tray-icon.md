# ADR-0001: Build in Rust using the `tray-icon` crate

- **Status:** Accepted
- **Date:** 2026-07-30
- **Deciders:** @jv-gomes

## Context

Claude Tray is a long-running background agent. A user will have it running every waking
hour of their working day, so its idle cost is the feature that determines whether people
keep it installed. The stated budget is under 15 MB resident memory, negligible idle CPU,
and a visible icon within 300 ms of a cold start.

It must also render a native system tray icon on Linux (X11 and Wayland, across GNOME,
KDE, XFCE, and tiling window managers), macOS, and Windows. These three platforms expose
completely different tray APIs:

- Linux — the StatusNotifierItem D-Bus specification, in practice via AppIndicator
- macOS — `NSStatusItem` in AppKit
- Windows — `Shell_NotifyIcon` in the Win32 shell API

The application has no main window, minimal interaction surface, and is
security-sensitive: it handles authentication credentials for a user's Claude account.

## Decision

We will implement Claude Tray in **Rust**, using the **`tray-icon`** crate for the tray
layer, paired with a compatible windowing event loop.

## Options considered

### Option A — Rust with `tray-icon` (chosen)

- **Pros:** No runtime or garbage collector, so the idle footprint is genuinely small and
  predictable. Single self-contained binary per platform, which makes distribution simple.
  Memory safety and a strong type system suit a credential-handling application, and
  `unsafe` can be forbidden outright in our own code. `tray-icon` abstracts all three
  platform APIs and is maintained as part of the Tauri project, so it sees real-world use.
  Strong ecosystem for the rest of the stack: `keyring` for OS credential stores,
  `reqwest` with `rustls`, `tokio`, `notify-rust`.
- **Cons:** Longer compile times. A smaller contributor pool than a scripting language.
  `tray-icon` is not a complete abstraction — platform-specific handling still leaks
  through (main-thread requirements, macOS template images, Windows `TaskbarCreated`).

### Option B — Electron or Tauri with a web front end

- **Pros:** Fastest UI development, easy theming, large contributor pool.
- **Cons:** Electron's baseline memory is well over 100 MB, which fails the core
  constraint by an order of magnitude — and the constraint *is* the product for a tray
  utility. Tauri is far lighter but still bundles a webview we would never render into,
  since the entire interface is an icon, a tooltip, and a native menu. All cost, no
  benefit.

### Option C — Go with a tray binding

- **Pros:** Fast compilation, easy cross-compilation, single binary, gentle learning curve.
- **Cons:** Garbage-collected runtime gives a larger and less predictable idle footprint.
  The mature tray bindings are cgo wrappers, which undercuts the cross-compilation
  advantage and complicates the build. Weaker story for OS credential-store integration.

### Option D — Three native applications, one per platform

- **Pros:** Best possible platform integration and the smallest possible binaries.
- **Cons:** Three codebases, three languages, three times the maintenance, for a project
  with one maintainer. Guarantees platform drift.

## Consequences

### Positive

- The idle-cost budget is achievable rather than aspirational.
- Distribution is a single binary per platform, easing packaging across every channel.
- Forbidding `unsafe` in our own code is realistic and enforceable in CI.
- The credential, HTTP, and notification layers all have well-maintained crates.

### Negative

- Contributors need Rust and per-platform system development packages installed, raising
  the barrier to a first contribution. Mitigated by documenting prerequisites precisely in
  `CONTRIBUTING.md` and by the mock provider, which lets people work on most of the code
  without credentials.
- CI must build and test on all three operating systems, so the pipeline is slower and
  more complex than a single-platform project's.
- Platform quirks must still be handled individually despite the abstraction.

### Risks

- **`tray-icon` becomes unmaintained.** The abstraction is confined to the binary crate,
  so a replacement would not touch `core`, `provider`, or `icon`. Signal to watch: no
  releases and unanswered platform bugs for two or more quarters.
- **GNOME does not display tray icons without a user-installed extension.** This is a
  desktop-environment limitation affecting every tray application, not a consequence of
  this choice. It is handled by detecting the absence of a StatusNotifier host and
  printing an actionable message, and by documenting the requirement prominently.

## Revisit when

The tray layer needs functionality `tray-icon` cannot express, or a platform's tray
support regresses badly enough that a native implementation for that platform becomes
worthwhile.
