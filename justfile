# Task runner for Claude Tray. Install with `cargo install just`.
# Run `just` with no arguments to list every recipe.

default:
    @just --list

# ---------------------------------------------------------------------------
# Everyday
# ---------------------------------------------------------------------------

# Run a debug build.
run *ARGS:
    cargo run --bin claude-tray -- {{ARGS}}

# Run against synthetic usage data — no Claude credentials needed.
run-mock PERCENT="42":
    cargo run --bin claude-tray -- --provider=mock --simulate-percent={{PERCENT}}

# Format the whole workspace.
fmt:
    cargo fmt --all

# Fail if anything is unformatted. This is what CI runs.
fmt-check:
    cargo fmt --all --check

# Lint with warnings denied. This is what CI runs.
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Apply clippy's machine-applicable suggestions, including to dirty files.
lint-fix:
    cargo clippy --workspace --all-targets --all-features --fix --allow-dirty --allow-staged

# Run the test suite (dev profile — release sets panic=abort, which libtest cannot use).
test:
    cargo test --workspace --all-features

# Build the documentation and fail on broken intra-doc links.
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Open the documentation in a browser.
doc-open:
    cargo doc --workspace --no-deps --open

# ---------------------------------------------------------------------------
# Verification
# ---------------------------------------------------------------------------

# Everything CI runs. Use this before pushing.
check-all: fmt-check lint test doc core-purity
    @echo ""
    @echo "✓ all checks passed"

# The domain crate must stay free of I/O, so that the bulk of the logic can be
# tested in CI with no desktop session, no network, and no credentials. See
# docs/adr/0006-chrono-for-time.md.
core-purity:
    #!/usr/bin/env bash
    set -euo pipefail
    forbidden='^(libc|iana-time-zone|tokio|reqwest|hyper|ureq|rustls|keyring|directories|notify-rust|tray-icon|windows-link|windows-sys) '
    tree="$(cargo tree -p claude-tray-core --edges normal --prefix none)"
    if echo "$tree" | grep -Eq "$forbidden"; then
        echo "error: claude-tray-core gained a dependency that can touch the outside world:" >&2
        echo "$tree" | grep -E "$forbidden" >&2
        exit 1
    fi
    if cargo tree -p claude-tray-core -e features | grep -q 'chrono feature "clock"'; then
        echo 'error: claude-tray-core enabled chrono'"'"'s "clock" feature, which puts an ambient clock in the domain model.' >&2
        exit 1
    fi
    echo "✓ claude-tray-core has no I/O dependencies"

# Supply-chain policy: advisories, licences, bans, sources. Needs `cargo install cargo-deny`.
deny:
    cargo deny check

# Vulnerability scan. Requires `cargo install cargo-audit`.
audit:
    cargo audit

# Coverage report. Requires `cargo install cargo-llvm-cov`.
coverage:
    cargo llvm-cov --workspace --all-features --html
    @echo "report: target/llvm-cov/html/index.html"

# Verify the build still satisfies the declared MSRV.
msrv:
    #!/usr/bin/env bash
    set -euo pipefail
    MSRV=$(grep -m1 '^rust-version' Cargo.toml | cut -d'"' -f2)
    echo "checking MSRV ${MSRV}"
    rustup toolchain install "${MSRV}" --profile minimal
    cargo +"${MSRV}" check --workspace --all-features

# ---------------------------------------------------------------------------
# Release
# ---------------------------------------------------------------------------

# Build the size-optimised profile used for release artifacts.
dist:
    cargo build --profile dist --bin claude-tray

# Print binary sizes for the release and dist profiles, for the performance budget.
sizes: dist
    cargo build --release --bin claude-tray
    @echo ""
    @ls -lh target/release/claude-tray target/dist/claude-tray | awk '{print $9, $5}'

# Dry-run publish every crate, in dependency order.
publish-check:
    cargo publish --dry-run -p claude-tray-core
    cargo publish --dry-run -p claude-tray-provider
    cargo publish --dry-run -p claude-tray-icon
    cargo publish --dry-run -p claude-tray

# ---------------------------------------------------------------------------
# Housekeeping
# ---------------------------------------------------------------------------

# Remove build artifacts.
clean:
    cargo clean

# Show the dependency tree, highlighting duplicate versions.
tree:
    cargo tree --workspace --duplicates
