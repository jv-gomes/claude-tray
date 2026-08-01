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

One fetch from the `local-file` provider, reading real Claude Code transcripts.

| Metric | Measured |
| ------ | -------- |
| Binary size (release) | 915,480 B (894 KB) |
| One fetch, 120.1 MB across 19 transcripts | ~206 ms |

Environment: Debian 13 (trixie), x86_64, warm page cache.

**This exceeds the idle-CPU budget and must be fixed before the scheduler wires
it up in phase 8.** At the default 30-second interval, 206 ms of work per refresh
averages roughly 0.7% CPU — seven times the 0.1% budget — and the cost grows with
every session recorded, without bound.

Two mitigations are already in place and are not enough on their own:

- **Modification-time filter.** Files not written to within the longest window
  are skipped unopened. On the machine measured it saved nothing, because all 19
  transcripts were younger than the 30-day monthly window. It only pays off on a
  machine with genuinely old history.
- **Substring pre-filter.** Only lines containing `"usage"` reach the JSON
  parser, which is what keeps a 120 MB scan down to 206 ms rather than seconds.

The fix is **incremental reading**: transcripts are append-only, so a refresh
should remember each file's length and parse only the bytes appended since the
last pass, retaining per-record timestamps so that rolling windows can still drop
records as they age out. That turns a steady-state refresh into near-zero work
regardless of how much history has accumulated.

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
