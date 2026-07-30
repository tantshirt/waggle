# Security Policy

## Supported versions

Security fixes target the default branch of this repository. Check [`BUZZ_VERSION`](BUZZ_VERSION) for the Buzz and BMAD ranges Waggle is designed to operate within.

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security-sensitive reports.

Prefer one of:

1. GitHub **Security Advisories** / private vulnerability reporting on this repository (if enabled), or
2. Contact the maintainers via the private channel listed in the repository’s GitHub security settings.

Include:

- Description of the issue and impact
- Steps to reproduce or a proof of concept (if safe)
- Affected component (e.g. identity provisioning, gate attribution, sync skill linking)

## Sensitive areas

Treat these as high sensitivity when reviewing changes:

- Agent Nostr secret keys (`keys/`, never commit)
- Gate approval attribution (who signed the record)
- Relay membership / admin allowlists
- Environment files and deploy secrets

Waggle must never print secret key material in logs, compile reports, or generated configuration.
