# ADR-0002: Abstract all usage data behind a provider trait

- **Status:** Accepted
- **Date:** 2026-07-30
- **Deciders:** @jv-gomes

## Context

Claude Tray's entire purpose is to display Claude usage. Yet at the time of this decision,
**there is no single documented, public API that exposes usage and quota for a consumer
Claude subscription.** The candidate sources each have different coverage and different
degrees of stability:

- **Local Claude Code state** under `~/.claude/` — present on the machine, no network
  required, but it is an internal format with no stability guarantee, and it may expose
  only consumption rather than a quota limit.
- **Anthropic Admin API usage and cost reports** — documented and stable, but scoped to
  API organisations, so it may not describe subscription plans at all.
- **claude.ai internal endpoints** — likely the richest data, but undocumented, subject to
  change without notice, protected against automation, and of uncertain standing under the
  Terms of Service.

Two consequences follow. First, the choice of default source is genuinely unresolved and
must wait on research. Second, whichever source is chosen, **it will change** — an
internal file format will be restructured, or an endpoint will move.

Meanwhile the entire user interface — icon rendering, tooltip, menu, notifications,
scheduler — needs to be built, and none of it should be blocked on this question.

## Decision

We will define a **`UsageProvider` trait** that all data sources implement, and the rest of
the application will depend only on that trait and on the domain types it returns. A
**mock provider** will be implemented first, before any real one.

Providers will be classified and treated differently by stability:

- **Supported** — built on documented or local, stable data. Enabled by default.
- **Best-effort** — built on undocumented behaviour. Behind a non-default cargo feature,
  disabled in configuration by default, and requiring an explicit informed opt-in from the
  user before it will run.

## Options considered

### Option A — Provider trait with a mock-first implementation (chosen)

- **Pros:** Decouples all UI work from the unresolved data question, which is the single
  largest schedule risk in the project. Makes the whole pipeline testable without network
  or credentials. Allows several sources to coexist and to fall back to one another.
  Confines the blast radius when a source breaks to one file. Lets us be honest with users
  about which sources are reliable.
- **Cons:** An indirection layer that a single-source design would not need. The trait must
  express a union of what different sources can supply, which forces a capability
  mechanism so the UI can hide fields the active provider cannot populate.

### Option B — Code directly against one chosen source

- **Pros:** Simplest possible implementation; no premature abstraction.
- **Cons:** All UI work stalls until the source question is answered. When that source
  changes, the change propagates through the codebase. Offers users no alternative if their
  plan is not covered. Makes testing require either real credentials or ad-hoc mocking.

### Option C — Delegate to an external tool and parse its output

- **Pros:** Someone else maintains the fragile part.
- **Cons:** Adds an installation dependency, contradicting the single-binary goal. Output
  formats of such tools are themselves unstable. Adds process-spawn latency to every
  refresh.

## Consequences

### Positive

- Phases 5 through 12 of the roadmap — icon, tray, tooltip, menu, notifications, config —
  can be built and fully tested against the mock, in parallel with the data research.
- Fault injection becomes trivial: the mock can produce timeouts, 401s, 429s, and malformed
  payloads on demand, so error handling is genuinely exercised rather than assumed.
- Users on different plans can be served by different providers without forking.
- A broken source degrades to a clear error state instead of a broken application.

### Negative

- A `ProviderCapabilities` mechanism is required so the tooltip and menu can omit fields
  the active provider cannot supply. Without it the UI would show empty rows.
- The domain model must express the union of all sources' data, and therefore many fields
  are `Option`. Every consumer must handle absence.
- Fallback chaining introduces a subtlety: the user must be able to see *which* provider
  produced the number they are looking at, so `UsageSnapshot` carries its source.

### Risks

- **No source proves viable for subscription plans.** This is the project's existential
  risk and the abstraction does not remove it — it only ensures the failure is discovered
  without having wasted the UI work. If it materialises, the honest outcome is a tool that
  covers API organisations, or one that reports local consumption without a percentage,
  and the README must say so plainly.
- **The abstraction could leak the shape of whichever provider is written first.** Mitigated
  by writing the mock first and by defining the domain types in `claude-tray-core` from the
  user's point of view rather than any source's payload shape.

## Revisit when

The data-source research concludes (ADR-0003 will record the default provider choice), or a
documented, officially supported usage API becomes available — which would make the
supported/best-effort distinction unnecessary and could justify simplifying the design.
