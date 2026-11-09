# Security policy

## Supported versions

Tracewise is pre-1.0. Only the latest release gets security fixes.

## Reporting a vulnerability

Please report vulnerabilities privately through GitHub: **Security → Report a vulnerability** on this repository. Do not open a public issue, and do not include credentials, private source code or real hook payloads.

A useful report names the version or commit, the macOS version, a minimal reproduction in a disposable repository, and the impact (for example: history or files changed, a secret stored, a path written outside the repository).

You can expect a first answer within 7 days.

## Scope

Tracewise's locks and hooks coordinate cooperating tools; they are not a sandbox against arbitrary processes of the same user. The local hash chain detects changed or missing records but is not an independent audit authority. Sharing and the privileged sensor are opt-in. See [How it works](docs/how-it-works.md) for what is stored. These limits do not excuse path traversal, secret leakage, privilege escalation or silent data loss; please report those.
