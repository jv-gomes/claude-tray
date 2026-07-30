# Claude Tray

**Monitor your Claude usage from the system tray, like a laptop battery indicator.**

[![CI](https://github.com/jv-gomes/claude-tray/actions/workflows/ci.yml/badge.svg)](https://github.com/jv-gomes/claude-tray/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Status: experimental](https://img.shields.io/badge/status-experimental-orange.svg)](#project-status)

> **Not affiliated with Anthropic.** Claude Tray is an independent, community-built
> project. It is not endorsed by, sponsored by, or connected to Anthropic PBC.
> "Claude" is a trademark of Anthropic PBC and is used here only to describe what
> this tool monitors.

---

## Project status

**Experimental — not yet usable.** The repository is in early scaffolding. There is
no working build to install yet, and the interface described below is a design target,
not a shipped feature set. Watch the repo or check [Releases](https://github.com/jv-gomes/claude-tray/releases)
for the first tagged version.

An important open question is being worked through first: **which usage data sources
are available and permitted.** See [Data sources](#data-sources) below — the answer
shapes what this tool can honestly report.

---

## What it does

Instead of opening a browser to check how much Claude usage you have left, a glance at
the tray tells you. The icon fills and empties like a battery, and changes colour as
your remaining quota drops.

```text
██████████ 100%
████████░░  80%
██████░░░░  60%
████░░░░░░  40%
██░░░░░░░░  20%
█░░░░░░░░░  10%
```

| Remaining | Colour    | Meaning                   |
| --------- | --------- | ------------------------- |
| 75–100%   | 🟢 Green  | Plenty of usage remaining |
| 40–74%    | 🟡 Yellow | Moderate usage            |
| 15–39%    | 🟠 Orange | Running low               |
| 0–14%     | 🔴 Red    | Almost exhausted          |

Hovering the icon shows the detail: plan, remaining percentage, time used in the
current window, time until reset, and when the data was last refreshed. Optional
desktop notifications fire as you cross configurable thresholds.

<!-- TODO: replace with a real screenshot / GIF once the tray renders (roadmap P16-01) -->

---

## Design goals

- **Native.** Feels like an operating system component, not an app you launch.
- **Lightweight.** Target: under 15 MB idle memory, negligible idle CPU.
- **Fast.** Target: icon visible in under 300 ms from a cold start.
- **Private.** No telemetry, ever. Nothing leaves your machine except calls to Anthropic.
- **Read-only.** Claude Tray observes your usage. It never consumes quota on your behalf.
- **Cross-platform.** Linux, macOS, and Windows.

---

## Installation

Not yet available. Installation instructions for each platform will land with the
first release. Planned distribution: GitHub Releases binaries, AUR, Homebrew, winget,
`.deb`/`.rpm`, and crates.io.

### Building from source

Requires a stable Rust toolchain. Platform prerequisites and the full build procedure
are documented in [CONTRIBUTING.md](CONTRIBUTING.md).

```sh
git clone https://github.com/jv-gomes/claude-tray.git
cd claude-tray
cargo build --release
```

---

## Data sources

Claude Tray reads your usage from a **pluggable provider**. This matters, because there
is no single public API that exposes subscription usage, and the available sources
differ in coverage and stability.

Providers are classified as:

- **Supported** — built on documented or local, stable data. Enabled by default.
- **Best-effort** — built on undocumented behaviour. Disabled by default, opt-in only,
  and may break without warning.

The full analysis of each candidate source, including its legal standing, will be
documented in `docs/DATA-SOURCES.md`. Claude Tray will not enable a best-effort provider
without an explicit, informed opt-in.

---

## Privacy

- No analytics, no telemetry, no crash reporting to any third party.
- Credentials are stored **only** in your operating system's credential store
  (Secret Service, Keychain, or Windows Credential Manager) — never in a config file,
  a log, or the cache.
- Usage history is stored locally, is capped, and can be disabled entirely.
- Logs are redacted: nothing token-shaped is ever written to disk.

A plain-language `docs/PRIVACY.md` will accompany the first release.

---

## Contributing

Contributions are very welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) for development
setup, platform prerequisites, and the pull request checklist. Issues tagged
[`good first issue`](https://github.com/jv-gomes/claude-tray/labels/good%20first%20issue)
are a reasonable place to start.

This project is governed by a [Code of Conduct](CODE_OF_CONDUCT.md). By participating,
you agree to uphold it.

To report a security vulnerability, please follow [SECURITY.md](SECURITY.md) rather than
opening a public issue.

---

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in this work by you, as defined in the Apache-2.0 license, shall be dual
licensed as above, without any additional terms or conditions.
