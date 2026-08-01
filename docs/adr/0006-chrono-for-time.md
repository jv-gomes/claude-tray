# ADR-0006: Use `chrono` for time, with UTC-only types in the core

- **Status:** Accepted
- **Date:** 2026-07-30
- **Deciders:** @jv-gomes

> ADR numbers 0003 (default usage provider), 0004 (authentication model), and 0005
> (icon pipeline) are reserved by roadmap tasks P3-09, P4-02, and P5-02. This
> decision was made earlier than those, hence the out-of-order number. See
> [`README.md`](README.md) in this directory for the index.

## Context

Claude Tray needs timestamps for four things: when a snapshot was captured, when a
quota window resets, how long until that reset, and when the data was last refreshed
for display in the tooltip.

Two constraints shape the choice.

**The core crate must have no I/O.** `claude-tray-core` is required to be free of
network, filesystem, display, and ambient clock access, so that the domain logic is
testable in CI without a desktop session. Reading the system clock or the local
timezone is I/O.

**The application is multi-threaded.** A Tokio runtime handles refreshes on
background threads while the tray event loop runs on the main thread. This turns out
to be decisive, because obtaining the *local* UTC offset on Unix is only sound in a
single-threaded process — the underlying `localtime_r` reads a process-global
timezone that another thread may mutate concurrently.

The tooltip needs local time ("Last updated 10:42:30"), so local conversion has to
happen somewhere.

## Decision

We will use **`chrono`**, and we will confine the core crate to **UTC-only types** by
depending on it with `default-features = false, features = ["serde", "std"]` —
deliberately **omitting the `clock` feature**.

Local-time conversion happens only at the display boundary, in the binary crate,
which enables `clock`.

## Options considered

### Option A — `chrono`, core restricted to UTC (chosen)

- **Pros:** `Local::now()` works correctly in a multi-threaded process; modern chrono
  determines the timezone itself rather than calling `localtime_r`, so the historic
  soundness problem does not apply. Large ecosystem and familiar API, which matters
  for contributors. Critically, **omitting the `clock` feature makes the no-I/O rule
  mechanically enforced rather than merely documented**: `Utc::now()` and `Local` do
  not exist in the core crate, so a function that wants the current time is forced to
  take it as a parameter, which is exactly what makes the logic testable.
- **Cons:** Larger than `time`. Carries API surface the project will never use.

### Option B — `time`

- **Pros:** Smaller, cleaner API, no legacy baggage.
- **Cons:** Fatal for this application: `UtcOffset::current_local_offset()`
  deliberately **fails in a multi-threaded process**, returning `IndeterminateOffset`
  rather than risking unsoundness. Claude Tray is unavoidably multi-threaded, so
  rendering a local timestamp in the tooltip — a feature in the spec — would need a
  workaround such as capturing the offset before any thread is spawned and hoping it
  does not change. That is fragile, and it would break across a daylight-saving
  transition while the app is running.

### Option C — `std::time` only

- **Pros:** No dependency at all.
- **Cons:** `SystemTime` cannot render a calendar date or a local time without a
  timezone database, so this only moves the problem. Formatting "resets at 14:30" by
  hand is not work worth doing.

## Consequences

### Positive

- The no-I/O rule for `claude-tray-core` is enforced by the compiler, not by review.
  Any function needing "now" must accept it, which makes time-dependent logic —
  staleness, reset detection, notification cooldowns — trivially testable with fixed
  instants and no mock clock infrastructure.
- Local time renders correctly on every platform and survives a daylight-saving
  change mid-session.
- `serde` support comes from the same crate, so persisted snapshots round-trip
  without a custom serialiser.

### Negative

- Two feature configurations of the same dependency exist in the tree: the core
  without `clock`, the binary with it. Cargo unifies these, so the binary's build
  enables `clock` for the core too. **The restriction is therefore a compile-time
  guarantee only when the core crate is built or tested on its own** — which CI does,
  and which is the case that matters for keeping the logic honest.
- Slightly larger binary than `time` would produce.

### Risks

- If chrono were to regress on multi-threaded local-time handling, the tooltip would
  be affected. The blast radius is small: local conversion happens at one boundary in
  the binary, not scattered through the codebase.

## Revisit when

The `time` crate gains a sound way to obtain the local offset in a multi-threaded
process, or binary size becomes a measured problem attributable to chrono.
