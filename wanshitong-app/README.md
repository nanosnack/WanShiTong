`wanshitong-app` defines application-level workflows and projections.

This crate is responsible for:
- Orchestrating domain operations into user-meaningful workflows
- Managing long-running or asynchronous tasks (e.g. ingest runs)
- Projecting core domain entities into shapes suitable for consumption
- Encapsulating policy decisions that are not domain invariants

The app layer does not define persistence or domain rules; it composes the
primitive APIs provided by `wanshitong-core`.

This crate is UI-agnostic and may be used by multiple frontends
(web UI, CLI, etc.).
