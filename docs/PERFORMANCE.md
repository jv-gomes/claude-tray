# Performance budget

Claude Tray runs all day, every day. Its idle cost is what decides whether people
keep it installed, so the budget below is treated as a requirement rather than an
aspiration, and it is **measured** rather than assumed.

## Budget

| Metric | Budget | Rationale |
| ------ | ------ | --------- |
| Idle memory (RSS) | < 15 MB | Comparable to a native panel applet. Anything larger is noticeable in a process list and undermines the "lightweight" claim. |
| Idle CPU | < 0.1% | The app is asleep between refreshes. Sustained CPU at idle drains laptop battery for no benefit. |
| Cold start to visible icon | < 300 ms | The icon should be present before the user finishes logging in. |
| Release binary size | < 10 MB | Keeps downloads and package sizes reasonable across every distribution channel. |
| Refresh network cost | 1 request / 30 s | Default interval, with jitter and backoff. Minimum configurable interval is clamped to protect the upstream. |

## How to measure

```sh
just sizes        # binary size, release and dist profiles
```

Idle memory and CPU, once the app runs continuously:

```sh
# Linux
ps -o rss=,pcpu= -p "$(pgrep -x claude-tray)"

# macOS
ps -o rss=,%cpu= -p "$(pgrep -x claude-tray)"

# Windows (PowerShell)
Get-Process claude-tray | Select-Object WorkingSet64, CPU
```

Cold start, measured to first painted icon rather than to process exit — the
skeleton timing below is process lifetime only, and stops being meaningful as soon
as there is a tray to paint.

## Baselines

Recorded so regressions are visible. Re-measure whenever a dependency is added.

### 2026-07-30 — Phase 1 skeleton

Empty workspace, no dependencies, no tray. This is a floor, not a prediction: it
measures the cost of the Rust runtime and nothing else.

| Metric | Release | Dist (`opt-level = "z"`) |
| ------ | ------- | ------------------------ |
| Binary size | 337,096 B (329 KB) | 321,232 B (314 KB) |
| Process lifetime | ~2 ms | — |

Environment: Debian 13 (trixie), x86_64, rustc 1.97.1, `lto = "thin"`,
`codegen-units = 1`, `strip = "symbols"`, `panic = "abort"`.

**Idle memory and CPU are not yet measurable** — the binary prints a line and exits.
Those rows get filled in once phase 6 produces a process that stays resident.

### 2026-08-01 — Phase 3, local-file provider

Fetches from the `local-file` provider, against this machine's real Claude Code
directory: 19 transcripts, 120.3 MB, 5,828 usage records inside the 30-day window.

| Metric | Measured | Budget |
| ------ | -------- | ------ |
| Binary size (release) | 915,480 B (894 KB) | < 10 MB |
| First fetch (cold cache) | ~262 ms | — |
| Steady-state refresh | ~0.5 ms | — |
| Implied idle CPU at a 30 s interval | ~0.002% | < 0.1% |
| Records retained in memory | 5,828 (~140 KB) | — |

Environment: Debian 13 (trixie), x86_64, warm page cache, release profile.

Reproduce with:

```sh
cargo run --release --example scan-timing -p claude-tray-provider
```

**Why the steady state is nearly free.** Transcripts are append-only, so what a
refresh has already parsed cannot change. The provider remembers how many bytes
of each file it has consumed and reads only what has been added since — for an
idle machine, nothing. What remains per refresh is one `stat` per transcript plus
a sum over the retained records. Three details make that safe:

- Reading stops at the **last complete line**. The session being written right
  now always ends mid-record, and consuming that partial line would lose the
  record and leave the offset inside the next one.
- A file **shorter** than what was consumed from it was replaced rather than
  appended to, so its state is discarded and it is re-read.
- Records that have aged out of every window are **dropped**, so the cache is
  bounded by the longest window rather than by process lifetime.

Two filters bound the cold cost as well:

- **Modification-time filter**, applied only to files never read before: one last
  touched before the longest window began cannot hold a record inside it. It
  saved nothing on the machine measured — all 19 transcripts were younger than 30
  days — and pays off on a machine with genuinely old history.
- **Substring pre-filter**: only lines containing `"usage"` reach the JSON
  parser, which is what keeps the 120 MB first pass at 262 ms rather than seconds.

The first fetch is still proportional to history inside the longest window. It
happens once, off the UI thread, and phase 8 should not block the first icon
paint on it.

## Profile comparison (P1-22)

The `dist` profile saves **15,864 bytes, about 4.7%**, over `release` on the skeleton.

That number should not be trusted yet. With no dependencies, almost all of the
binary is Rust runtime and panic machinery that `opt-level = "z"` cannot compress,
so the comparison mostly measures noise. The decision on whether `dist` earns its
place — and whether the size saving costs anything at runtime — is deferred until
the real dependency tree exists, most meaningfully after phase 6 brings in the tray
and GTK bindings.

Both profiles are kept for now so the comparison can be repeated cheaply.

## Notes

- `panic = "abort"` is set on `release` and inherited by `dist`. It removes unwinding
  tables, which is a real size win, but it means **`cargo test --release` does not
  work** — libtest needs unwinding. Tests run on the dev profile.
- `strip = "symbols"` means release binaries produce no useful stack traces. If a
  crash needs diagnosis, rebuild without stripping rather than guessing.
- Binary size is measured unpacked. Distribution formats compress further, so the
  download is smaller than the numbers here.
