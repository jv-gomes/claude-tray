# Security Policy

## Supported versions

Claude Tray is pre-release. Until `1.0.0` is tagged, **only the latest commit on `main`
is supported**. Security fixes are not backported to earlier commits or pre-release tags.

| Version | Supported |
| ------- | --------- |
| `main`  | ✅ |
| pre-1.0 tags | ❌ |

Once `1.0.0` ships, this table will list the supported release line.

---

## Reporting a vulnerability

**Please do not report security vulnerabilities through public GitHub issues,
discussions, or pull requests.**

Report privately through GitHub Security Advisories:

👉 **[Open a private security advisory](https://github.com/jv-gomes/claude-tray/security/advisories/new)**

This creates a report visible only to the repository maintainers. It is the preferred
and fastest channel.

### What to include

The more of this you can provide, the faster it can be triaged:

- The type of issue (credential exposure, path traversal, injection, and so on)
- Affected version or commit hash
- Your operating system and desktop environment
- Step-by-step reproduction instructions
- Proof-of-concept, if you have one
- The impact as you see it — what an attacker gains

**Please redact any real credentials from your report.** If a token appears in your
reproduction, replace it with a placeholder.

### What to expect

| Stage | Target |
| ----- | ------ |
| Acknowledgement of your report | within 5 days |
| Initial assessment and severity | within 10 days |
| Fix or mitigation plan communicated | within 30 days |

This is a volunteer-maintained project, so these are good-faith targets rather than
guarantees. You will be kept informed if something takes longer.

If you have not had a response within 10 days, please open a public issue saying only
that you are awaiting a response on a private report — **without any details of the
vulnerability itself.**

### Disclosure

Please give a reasonable window to ship a fix before public disclosure. Coordinated
disclosure is preferred, and credit will be given in the advisory and changelog unless
you would rather stay anonymous.

---

## Scope

### In scope

- Credential exposure through logs, caches, config files, diagnostics output, exports,
  or the clipboard
- Weaknesses in the authentication flow (CSRF, token interception, callback listener
  abuse)
- Insecure credential storage or file permissions
- Code execution through malformed provider responses, config files, or plugins
- TLS validation bypass
- Privilege escalation via the installer, autostart entries, or update mechanism
- Path traversal or symlink attacks in file handling

### Out of scope

- Vulnerabilities in Anthropic's services — please report those to Anthropic
- Issues requiring an attacker who already has full local access to the user's account
  and session, since the operating system's credential store is the trust boundary
- Missing hardening flags with no demonstrated exploit path
- Reports produced solely by an automated scanner with no verified impact
- Denial of service against the user's own machine
- Social engineering of maintainers

---

## Security design commitments

These are the properties the project intends to maintain. A verified break of any one of
them is a valid vulnerability report.

- **Credentials live only in the OS credential store** — Secret Service, Keychain, or
  Windows Credential Manager. Never in a config file, log, cache, or export.
- **Logs are redacted.** Nothing token-shaped is written to disk at any log level.
- **Diagnostics output is safe to paste in public.** The "Copy diagnostics" action must
  never include a secret.
- **TLS validation is always on.** There is no flag to disable it.
- **`unsafe` code is forbidden** across the workspace.
- **No telemetry.** Nothing is transmitted anywhere except the calls required to fetch
  your own usage.
- **Read-only.** The application never performs actions that consume your Claude quota.
- **Best-effort providers are opt-in.** Any provider built on undocumented behaviour is
  disabled by default and requires explicit, informed consent to enable.

The threat model will be documented in full in `docs/SECURITY-MODEL.md`.

---

## Supply chain

- Dependencies are checked with `cargo-deny` and `cargo-audit` in CI, plus a weekly
  scheduled scan.
- Dependabot proposes dependency and GitHub Actions updates.
- All GitHub Actions are pinned to commit SHAs, not floating tags.
- Release artifacts ship with SHA-256 checksums and an SBOM.

If you find a compromised or malicious dependency in the tree, please report it through
the private channel above.
