`wanshitong-core` defines the domain model and persistence boundaries for Wan Shi Tong.

This crate is responsible for:
- Defining canonical domain entities (e.g. data sources, files, measurements)
- Enforcing domain invariants and validity rules
- Owning the database schema and persistence logic
- Providing primitive commands and queries over domain state

The core layer contains no application workflows, background tasks, or UI logic.
It exposes a stable, storage-backed API that represents what *is true* about the
system, not how the system is used.

All higher layers depend on `wanshitong-core` for authoritative domain state.
