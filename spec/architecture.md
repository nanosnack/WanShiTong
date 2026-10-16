# Architecture

This document describes the **implementation architecture** of Wan Shi Tong (WST).

It focuses on:

* Module and crate boundaries
* Division of scope and responsibility
* Technology choices and constraints
* Deployment and runtime model

Domain concepts are defined separately in `domain-model.md` and are referenced here only
insofar as they affect module boundaries.

---

## Overview

WST is a **single-host, single-process application**. Its three main crates
have a strict dependency direction:

```
wanshitong-web  -->  wanshitong-app  -->  wanshitong-core
```

* Dependencies flow **inward only**
* Each layer has a clearly defined responsibility
* Lower layers have no knowledge of higher layers

Four supporting crates sit beside them:

* `wanshitong-api` holds the JSON API's types. `wanshitong-web` serves them
  and `wst` consumes them, so the contract belongs to neither.
* `wanshitong-client` is a client of the JSON API, with a method for each
  endpoint. It sends its requests through any service that answers HTTP
  requests: `wst` gives it a connection, and `wanshitong-web`'s API tests
  give it the router itself, so they test the client and the endpoints
  together.
* `ffprobe-types` mirrors ffprobe's JSON output. `wanshitong-app` reads it
  from ffprobe, and `wanshitong-core` keeps what it measures from it.
* `wst` is the command-line interface: a client of the JSON API, and
  `wst serve` runs the service.

The system is designed so that:

* `wanshitong-core` defines what is *true*
* `wanshitong-app` defines what is *done*
* `wanshitong-web` defines how it is *presented*

All persistent state is owned by `wanshitong-core`.

---

## Crate responsibilities and boundaries

### `wanshitong-core`

**Role:** Authoritative domain model and persistence boundary.

This crate defines the canonical schema, invariants, and storage-backed APIs for WST.
It represents durable facts about the system, independent of any UI or workflow.

#### Responsibilities

* Define canonical domain entities and identifiers
* Own the SQLite schema and migrations
* Validate and apply domain commands
* Enforce invariants and consistency rules
* Record provenance for all persisted facts
* Provide query APIs over persisted state

#### Non-responsibilities

* No application workflows
* No background task orchestration
* No filesystem scanning
* No heuristics or inference logic
* No HTTP, HTML, or UI concerns

#### Technology choices

* **SQLite** as the single embedded database
* **`sqlx`** with explicit SQL
* No ORM or code-generated schema
* Synchronous, deterministic behavior preferred over callbacks or events

The Core exposes an **in-process Rust API**. Any future HTTP exposure is considered a
transport detail and must not change Core semantics.

---

### `wanshitong-app`

**Role:** Application workflows and projections.

This crate composes Core primitives into user-meaningful operations and long-running tasks.
It is UI-agnostic and may be reused by multiple frontends.

#### Responsibilities

* Orchestrate multi-step workflows (e.g. ingest runs)
* Manage long-running or asynchronous tasks
* Invoke Core commands and queries in the correct order
* Apply policy decisions that are *not* domain invariants
* Project Core data into shapes suitable for consumption

Examples include:

* Filesystem ingest runs
* View materialization triggers
* Task progress tracking

#### Non-responsibilities

* Does not define or enforce domain rules
* Does not own persistence or schema
* Does not render HTML or handle HTTP
* Does not depend on web-specific concepts

#### Technology choices

* **Tokio** for async task execution
* Direct filesystem access where required
* No database access except via Core APIs

---

### `wanshitong-web`

**Role:** HTTP interface and server-rendered web UI.

This crate adapts HTTP requests into application-level calls and renders responses
for a human user.

#### Responsibilities

* Define HTTP routes and handlers
* Translate HTTP input into `wanshitong-app` calls
* Render HTML views
* Handle redirects, forms, and basic validation

#### Non-responsibilities

* No domain logic
* No workflow orchestration
* No persistence
* No background task management

All meaningful operations are delegated to `wanshitong-app`.

#### Rendering model

* **Server-rendered HTML** using **topcoat** views and its module router,
  served through Axum
* Progressive enhancement with **topcoat's client runtime** (signals, shards,
  live regions) first, and **Alpine.js** with **Alpine AJAX** where the
  runtime is lacking (fragment swaps, infinite scroll, DOM-heavy components)
* Minimal, declarative JavaScript only where necessary

There is:

* No client-side routing
* No client-side state beyond small, local UI state
* No SPA framework

The browser is treated as a thin interaction layer.

---

## Cross-cutting subsystems

This section describes **implementation-level subsystems** that span multiple crates.
It intentionally avoids defining domain concepts; those are specified in `domain-model.md`.
The focus here is *where code lives*, *who owns execution*, and *who is allowed to cause side effects*.

---

### Ingest

**Domain ownership:** `wanshitong-core`

**Execution ownership:** `wanshitong-app`

Ingest is a **domain-modeled process with application-level execution**.

The *existence, identity, configuration, and results* of ingests are part of the
Core domain model and are persisted in Core-owned tables. This allows ingest runs
to be audited, queried, re-run, and reasoned about as first-class domain facts.

Architecturally, however:

* Filesystem traversal and measurement execution live in `wanshitong-app`
* Ingest code performs external I/O and scheduling
* All ingest results are submitted to Core via explicit commands

This split allows ingest behavior to evolve operationally without weakening
Core’s authority over what was observed and when.

---

### Heuristics

**Domain ownership:** `wanshitong-core`

**Execution ownership:** `wanshitong-app`

Heuristics are **domain-described but application-executed**.

The Core owns:

* Heuristic identities
* Configuration and enablement
* Ordering and policy-relevant metadata
* Provenance records tying derived facts to heuristic runs

This information is persisted and validated as part of the domain model.

The application layer is responsible for:

* Executing heuristic code
* Submitting heuristic outputs to Core for validation and persistence

Architectural constraints:

* Heuristic execution does not bypass Core APIs
* Heuristic code does not perform persistence
* Core does not depend on heuristic implementation code
* Heuristics read the structure within a release, e.g. its file names and
  its own subdirectories, never how releases are organized in a data source,
  e.g. a file's depth or the directory that groups its release (see *Data
  Source* in `domain-model.md`)

This design makes heuristics replaceable and configurable without embedding
inference logic inside the Core crate.

---

## Persistence and data access

* All persistent data lives in a **single SQLite database**
* Only `wanshitong-core` executes SQL
* Higher layers use Core APIs exclusively

This enforces a single, auditable boundary for all state changes.

---

## Deployment model (MVP)

* Single OS process
* Single SQLite database file
* Local filesystem access
* No authentication or multi-user support

The web server, application workflows, and Core all run in-process.

Distributed operation, clustering, and federation are explicitly out of scope
for the MVP and should not influence current design decisions.

---

## Architectural invariants

The following rules are non-negotiable:

* Core owns all persistence and domain truth
* App composes Core; Web composes App
* No layer reaches around a lower layer
* No UI-driven invariants
* All side effects are explicit and attributable

Violations of these invariants require explicit design review.
