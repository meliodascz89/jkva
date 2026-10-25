# Security policy

JKva processes untrusted content from the web. Security bugs are treated as first-class bugs, not as a later phase.

## Reporting

Do **not** open a public issue for a vulnerability. Use GitHub's private vulnerability reporting on the repository ("Security" → "Report a vulnerability") once the repository is public.

Include:

- affected crate and commit,
- a minimal reproduction (an HTML file is ideal),
- what an attacker can achieve,
- whether the issue is memory-safety related.

## Scope

In scope: memory safety, sandbox escapes, origin/SOP bypasses, CSP bypasses, IPC trust issues, denial of service that is trivially triggerable from a page.

Out of scope for now: missing features, missing APIs, visual differences, and anything that requires the user to run untrusted local code outside the engine.

## Response

The project is early and maintained by a small team. We aim to acknowledge a report within seven days and to publish a fix and an advisory once users are protected.

## Supported versions

Only the latest release and `main` receive security fixes until 1.0.
