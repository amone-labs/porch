# 0014. Apache License 2.0 for porch

- Status: accepted
- Date: 2026-10-10
- Supersedes: the repository's previous AGPL-3.0-only licensing choice

## Context

The maintainer wants porch to be freely reusable and extensible by engineers,
including in commercial products and self-hosted deployments. The intended
business model is to charge for managed infrastructure and operational convenience.
Apache License 2.0 provides permissive reuse with explicit contributor patent
terms and redistribution requirements.

## Decision

- License the current porch source and future contributions under Apache License 2.0.
- Keep the full license in `LICENSE` and project attribution in `NOTICE`.
- Use `Apache-2.0` consistently in package metadata, contribution instructions,
  README files and the landing page. Bundle `LICENSE` and `NOTICE` with the app.
- Preserve third-party licenses and notices; this decision does not relicense
  dependencies or other third-party material.
- This change does not revoke the AGPL permissions granted for earlier releases.

## Consequences

- Commercial reuse, proprietary modifications and competing hosted services are
  permitted, subject to Apache License 2.0's conditions.
- Downstream users do not have to publish their modifications or contribute them back.
- The license grants no permission to use porch's trademarks beyond its stated exceptions.
