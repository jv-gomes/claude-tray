# Architecture Decision Records

Significant technical decisions and the reasoning behind them, including the options
rejected. Start from [`0000-template.md`](0000-template.md).

An ADR records *why*, not *how*. If a decision turns out to be wrong, write a new ADR
that supersedes the old one rather than editing history — the reasoning that led to
the mistake is usually the most useful part.

## Index

| # | Decision | Status |
| - | -------- | ------ |
| [0001](0001-rust-and-tray-icon.md) | Rust with the `tray-icon` crate for the tray layer | Accepted |
| [0002](0002-provider-abstraction.md) | Abstract all usage data behind a provider trait | Accepted |
| 0003 | Default usage provider | **Reserved** — roadmap P3-09, pending research |
| 0004 | Authentication model | **Reserved** — roadmap P4-02 |
| 0005 | Icon rendering pipeline | **Reserved** — roadmap P5-02 |
| [0006](0006-chrono-for-time.md) | `chrono` for time, UTC-only in the core crate | Accepted |

Numbers 0003 to 0005 are reserved by roadmap tasks and will be filled in when those
decisions are actually made. ADR-0006 was decided earlier than them, which is why the
sequence is out of order — numbers are identifiers, not a timeline.
