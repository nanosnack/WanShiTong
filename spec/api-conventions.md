# API Conventions

These conventions apply to all HTTP APIs exposed by WST.

## General

- JSON for data APIs
- HTML for Web UI
- Explicit versioning (e.g. /api/v1)

## Commands vs Queries

- Commands mutate state
- Queries do not
- Commands are validated by the Core

## Error handling

- Errors are explicit
- No silent coercion
- Conflicts are surfaced, not hidden

## Stability

- Internal APIs may change
- External APIs aim for stability
- Breaking changes require spec updates
