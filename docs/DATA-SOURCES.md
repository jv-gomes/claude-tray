# Where usage data can come from

Claude Tray's central unresolved question. There is no documented public API that
reports usage and quota for a consumer Claude subscription, so each candidate
source has to be assessed on its own — for what it covers, how stable it is, and
whether using it is defensible.

This document records what has actually been checked. **Sections marked *not yet
investigated* are open**, and no default provider has been chosen (roadmap P3-08).

Every claim here is dated, because all of it describes undocumented behaviour that
can change without notice.

---

## Summary

| Source | Status | Reports a percentage? | Fragility |
| ------ | ------ | --------------------- | --------- |
| Local Claude Code transcripts | **Implemented** (`local-file`) | **No** — consumption only | Undocumented format, no compatibility promise |
| Anthropic Admin API | Not yet investigated | Unknown | Documented, but may not cover subscriptions |
| claude.ai internal endpoints | Not yet investigated | Unknown | Undocumented; standing under the Terms of Service unassessed |

---

## Local Claude Code transcripts

**Investigated 2026-08-01, against Claude Code as installed on a Linux
workstation. Implemented as the `local-file` provider.**

### What is there

Claude Code keeps its state in `~/.claude` — the home directory on every
platform, not the XDG or Known Folders config location — overridable with
`CLAUDE_CONFIG_DIR`.

| Path | Contents |
| ---- | -------- |
| `projects/<slug>/<session-uuid>.jsonl` | One JSON Lines transcript per session. Assistant records carry `message.usage`. |
| `history.jsonl` | Prompt history: `display`, `project`, `sessionId`, `timestamp`. No usage. |
| `.credentials.json` | OAuth tokens, `expiresAt`, `scopes`, plus `subscriptionType` and `rateLimitTier`. Mode 0600. |
| `settings.json` | `theme`, `model`, `effortLevel`, `enabledPlugins`. No usage. |
| `sessions/`, `session-env/`, `telemetry/`, `tasks/`, `file-history/` | Session bookkeeping, delivery-failure logs, and file snapshots. No usage. |

An assistant record's `message.usage` block:

```json
{
  "input_tokens": 2,
  "output_tokens": 230,
  "cache_creation_input_tokens": 6901,
  "cache_read_input_tokens": 12725,
  "cache_creation": { "ephemeral_1h_input_tokens": 0, "ephemeral_5m_input_tokens": 6901 },
  "server_tool_use": { "web_fetch_requests": 0, "web_search_requests": 0 },
  "service_tier": "standard"
}
```

### The finding that matters

**These files record consumption. They do not record a quota.**

Searching every transcript on a machine with 5,700 assistant records for any key
containing `quota`, `limit`, `rate`, `reset`, `remaining`, `tier`, or `plan`
returns exactly two things: `message.usage.service_tier`, which is the constant
string `"standard"`, and `message.content[].input.limit`, which is an argument to
the file-reading tool. Neither is an allowance.

So a provider built on these files **cannot produce a remaining-usage
percentage**, and therefore cannot drive a battery icon on its own. It reports
tokens consumed over rolling spans, with `limit` unset — which the domain model
renders as an unknown state rather than as a full battery.

The subscription plan *is* on the machine, as `subscriptionType` inside
`.credentials.json`. The `local-file` provider does not read it: that file exists
to hold OAuth tokens, and parsing a secret to obtain a display string is not a
trade worth making. Credential handling belongs to roadmap phase 4, and the plan
can be supplied from there.

### Stability

The format is Claude Code's internal one and carries no compatibility promise.
The parser therefore treats every field as optional, skips any record it does not
recognise rather than failing the reading, and carries a `PARSER_VERSION` so a bug
report can say which reading produced a figure. A scan that opens transcripts and
finds no usage record at all is reported as a parse failure rather than as zero
usage — that is what a format change looks like from here, and silently reporting
"0 tokens" would be worse than saying the source broke.

### Cost

Reading is the expensive part: transcripts accumulate indefinitely, and 120 MB
after roughly a week of daily use is realistic. Because transcripts are
append-only, a refresh parses only the bytes added since the last one, which
takes a steady-state refresh from ~262 ms to ~0.5 ms. See `PERFORMANCE.md` for
the measurements, and for the correctness details that make that safe.

---

## Anthropic Admin API

**Not yet investigated** (roadmap P3-03).

The documented usage and cost report endpoints are scoped to API organisations.
Whether they describe consumer subscription plans at all is the open question; if
they do not, this source covers a different audience from the one the README
describes.

---

## claude.ai internal endpoints

**Not yet investigated** (roadmap P3-05).

Likely the richest data, and the only candidate so far that could plausibly
report an actual quota. Also undocumented, subject to change without notice,
protected against automated access, and of unassessed standing under Anthropic's
Terms of Service.

**Nothing here will be implemented before that assessment is written down**
(roadmap P3-06). If it is pursued at all it will be as a *best-effort* provider
per ADR-0002: behind a non-default cargo feature, disabled in configuration, and
requiring an explicit informed opt-in.

---

## What this means for v1.0

The honest position today is that Claude Tray can report **how much you have
used**, and cannot yet report **how much you have left**. The battery icon —
the project's central idea — needs a source that knows the allowance, and no such
source has been confirmed.

That is the project's existential risk, named as such in ADR-0002. The
abstraction does not remove it; it only ensures the answer arrives without the
user interface work having been wasted. If no source proves viable, the honest
outcome is a tool that reports consumption, and the README must say so plainly.
