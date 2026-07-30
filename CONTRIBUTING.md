# Contributing to Claude Tray

Thanks for taking an interest. This document covers everything you need to get a
development build running and a pull request merged.

By participating, you agree to abide by the [Code of Conduct](CODE_OF_CONDUCT.md).

---

## Table of contents

- [Before you start](#before-you-start)
- [Development setup](#development-setup)
- [Platform prerequisites](#platform-prerequisites)
- [Running and testing](#running-and-testing)
- [Project layout](#project-layout)
- [Coding standards](#coding-standards)
- [Dependency policy](#dependency-policy)
- [Commit convention](#commit-convention)
- [Branching model](#branching-model)
- [Pull request checklist](#pull-request-checklist)
- [Architecture decisions](#architecture-decisions)
- [Translations](#translations)
- [Licensing of contributions](#licensing-of-contributions)

---

## Before you start

For anything larger than a bug fix or a typo, **open an issue first**. It is much
cheaper to disagree about an approach in an issue than in a finished branch.

The project is early and the architecture is still settling, so some areas move fast.
If an issue has an assignee or an open PR linked, check in before duplicating work.

---

## Development setup

You need a **stable Rust toolchain**. Install via [rustup](https://rustup.rs):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

The repository pins its toolchain in `rust-toolchain.toml`, so rustup will select the
right version automatically. Then:

```sh
git clone https://github.com/jv-gomes/claude-tray.git
cd claude-tray
cargo build
```

### Optional but recommended tooling

| Tool | Purpose | Install |
| ---- | ------- | ------- |
| `just` | Task runner used by this repo | `cargo install just` |
| `cargo-deny` | Licence and advisory checks | `cargo install cargo-deny` |
| `cargo-audit` | Vulnerability scanning | `cargo install cargo-audit` |
| `cargo-llvm-cov` | Coverage reports | `cargo install cargo-llvm-cov` |

---

## Platform prerequisites

Claude Tray talks to native tray APIs, so each platform needs system packages.

### Linux

```sh
# Debian / Ubuntu
sudo apt install build-essential pkg-config libgtk-3-dev \
    libayatana-appindicator3-dev libxdo-dev libssl-dev

# Fedora
sudo dnf install gtk3-devel libappindicator-gtk3-devel libxdo-devel

# Arch
sudo pacman -S base-devel gtk3 libayatana-appindicator xdotool
```

> **GNOME users:** GNOME Shell does not display tray icons without an extension.
> Install [AppIndicator and KStatusNotifierItem Support](https://extensions.gnome.org/extension/615/appindicator-support/)
> or the icon will not appear. This is a GNOME limitation, not a bug in Claude Tray.

Both X11 and Wayland are supported. If you are fixing a platform bug, please say which
you tested on, and which desktop environment.

### macOS

Xcode Command Line Tools:

```sh
xcode-select --install
```

Notifications and distribution require a signed application bundle, which needs a paid
Apple Developer account. You do **not** need one to develop — unsigned local builds work
fine, but notifications may not appear. See `docs/TROUBLESHOOTING.md`.

### Windows

Install the [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/)
with the "Desktop development with C++" workload, which provides the MSVC toolchain.

---

## Running and testing

```sh
just run           # run a debug build
just fmt           # format
just lint          # clippy with warnings denied
just test          # run the test suite
just check-all     # everything CI runs — use this before pushing
```

Without `just`, the equivalents are:

```sh
cargo run
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

### Developing without real credentials

You do not need a Claude account to work on most of the codebase. A mock provider
supplies synthetic data:

```sh
cargo run -- --provider=mock                 # a fixed snapshot
cargo run -- --provider=mock --simulate-percent=12   # force a specific state
```

This is the intended way to work on icon rendering, the tooltip, the menu, and
notifications.

---

## Project layout

```
crates/
  claude-tray-core/       Domain model. Pure logic, no I/O, no GUI.
  claude-tray-provider/   Usage data sources behind a common trait.
  claude-tray-icon/       Battery icon rasterisation.
  claude-tray/            Binary. Tray, event loop, and wiring.
docs/
  adr/                    Architecture Decision Records.
```

The most important rule: **`claude-tray-core` has no I/O dependencies.** All logic that
can be tested without a network, a filesystem, or a display belongs there. This keeps
the majority of the project testable in CI on every platform.

---

## Coding standards

- `cargo fmt` is authoritative. Do not hand-format.
- `cargo clippy` must pass with `-D warnings`.
- **`unsafe` is forbidden** workspace-wide. If you believe you need it, open an issue
  first and explain why.
- **No `unwrap()` or `expect()` in library code.** Return a `Result`. Tests are exempt.
- Library errors use `thiserror`; the binary may use `anyhow` at its outermost boundary.
- Public items need rustdoc. `missing_docs` is a warning and will be caught in CI.
- Every user-facing error message must name a next action — "Sign in", "Check your
  network" — not just describe the failure.

### Security-sensitive rules

These are not style preferences. A PR violating one will not be merged.

- Never log, cache, export, or copy a credential. Diagnostics output must be safe to
  paste into a public issue.
- Never disable TLS certificate validation, not even behind a flag.
- Treat every field of an external payload as optional. Never panic on an unexpected
  shape.
- Test fixtures must contain no real tokens, account identifiers, or personal paths.

---

## Dependency policy

Every new dependency is a permanent maintenance and security commitment, so:

- Justify it in the PR description. "It saves 20 lines" usually is not enough.
- Prefer `rustls` over anything requiring OpenSSL.
- Disable default features and enable only what you use.
- Check the transitive tree with `cargo tree` before proposing it.
- It must pass `cargo deny check` — compatible licence, no known advisories.

---

## Commit convention

This project uses [Conventional Commits](https://www.conventionalcommits.org/). The
changelog is generated from commit history, so the format matters.

```
<type>(<scope>): <short summary>
```

**Types:** `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`,
`revert`.

**Scopes:** `core`, `provider`, `icon`, `tray`, `auth`, `config`, `notify`, `sched`,
`cli`, `docs`, `ci`, `deps`.

Examples:

```
feat(icon): render discrete battery fill levels
fix(tray): re-add icon after Explorer restarts on Windows
docs(readme): document the GNOME extension requirement
```

Breaking changes get a `!` before the colon and a `BREAKING CHANGE:` footer.

---

## Branching model

Trunk-based. `main` is always releasable and is protected — no direct pushes.

Branch from `main`, keep the branch short-lived, and open a PR. Rebase rather than merge
to keep history linear. Squash-merge is the default for PRs.

---

## Pull request checklist

Before marking a PR ready:

- [ ] `just check-all` passes locally.
- [ ] New logic has tests; new user-facing behaviour has documentation.
- [ ] No new `unwrap()` or `expect()` in library code.
- [ ] No secret can reach a log, cache, or export path.
- [ ] `CHANGELOG.md` updated under `[Unreleased]` if the change is user-visible.
- [ ] Commits follow the convention above.
- [ ] For platform-specific changes: stated which OS and desktop environment you tested.

CI runs on Linux, macOS, and Windows. A change that only compiles on your machine will
be caught, so it is worth running `just check-all` first.

---

## Architecture decisions

Significant technical choices are recorded as ADRs in `docs/adr/`. If your PR makes or
overturns such a decision, add or supersede an ADR in the same PR. Copy
`docs/adr/0000-template.md` to start.

---

## Translations

User-facing strings live in a single module so they can be translated. To add a locale,
see the existing translations and open a PR — a translation does not need to be complete
to be useful, as missing keys fall back to English.

---

## Licensing of contributions

Unless you explicitly state otherwise, any contribution you intentionally submit for
inclusion in this work, as defined in the Apache-2.0 license, shall be dual licensed as
MIT OR Apache-2.0, without any additional terms or conditions.

Do not submit code you did not write or do not have the right to license this way. This
includes substantial verbatim output from tools trained on incompatibly licensed code.
