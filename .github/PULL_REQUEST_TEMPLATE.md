<!--
Thanks for contributing! Please fill in what applies and delete what doesn't.
For anything larger than a bug fix, an issue should exist first.
-->

## What does this change?

<!-- A short description of the change and why it is needed. -->

## Related issue

<!-- e.g. Closes #12 -->

## Roadmap task

<!-- If this implements a roadmap task, reference its ID, e.g. P5-04. Otherwise remove. -->

## How was this tested?

<!--
Describe what you actually ran, not what you intended to run.
For UI or tray changes, say which OS *and* desktop environment you verified on,
since a change can work on KDE and fail on GNOME.
-->

- [ ] Automated tests
- [ ] Manually verified on: <!-- e.g. Ubuntu 24.04 / GNOME 46 / Wayland -->

## Checklist

- [ ] `just check-all` passes locally (fmt, clippy `-D warnings`, tests)
- [ ] New logic has tests
- [ ] New user-facing behaviour is documented
- [ ] No new `unwrap()` or `expect()` in library code
- [ ] `CHANGELOG.md` updated under `[Unreleased]` if this is user-visible
- [ ] Commits follow [Conventional Commits](../CONTRIBUTING.md#commit-convention)

### If this touches credentials, logging, network, or file writes

- [ ] No secret can reach a log, cache, config file, export, or the clipboard
- [ ] TLS certificate validation is not weakened anywhere
- [ ] External payloads are treated as untrusted — every field optional, no panics
- [ ] Test fixtures contain no real tokens, account identifiers, or personal paths

### If this adds a dependency

- [ ] Justified below, with default features disabled
- [ ] `cargo deny check` passes

<!-- Justification: -->

## Screenshots

<!-- For any visual change — icon states, tooltip, menu — please include before/after. -->

## Notes for reviewers

<!-- Anything you are unsure about, or would like a second opinion on. -->
