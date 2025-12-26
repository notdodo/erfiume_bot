# AGENTS.md

## Overview

`erfiume_bot` is a **serverless Telegram bot** that reports **hydrometric (water level) data for rivers in Emilia-Romagna**.
It retrieves data from the _Allerta Meteo Emilia-Romagna_ public API and replies to Telegram users via commands.

The project is written in **Rust** and deployed on **AWS Lambda**, using **Pulumi** for infrastructure-as-code and **DynamoDB** for persistence.

This document is intended to help:

- Human contributors
- Automated agents (LLMs, Codex, Copilot, CI bots)

understand the project structure, conventions, and expected behavior.

---

## Purpose of This File

This file provides:

1. **Context for AI agents** so generated code aligns with project intent.
2. **Architectural constraints** to prevent invalid assumptions.
3. **Project-specific conventions** that agents should follow.
4. **Recommended improvements** that agents are allowed to propose and implement.

---

## High-Level Architecture

### Components

- **Telegram Bot Lambda (`app/bot`)**

  - Receives webhook updates from Telegram
  - Parses commands and free-text messages
  - Queries DynamoDB for station data
  - Replies using `teloxide`

- **Fetcher Lambda (`app/fetcher`)**

  - Runs on a schedule via EventBridge
  - Fetches hydrometric data from Allerta Meteo
  - Normalizes and stores station data in DynamoDB

- **Infrastructure**
  - Defined with Pulumi
  - Manages Lambdas, DynamoDB, EventBridge rules, and secrets

---

## Agent Responsibilities

Automated agents interacting with this repository **must**:

- Respect the serverless execution model (cold starts, statelessness)
- Avoid long-lived background tasks
- Treat DynamoDB as the single source of truth
- Assume eventual consistency of external data sources
- Avoid breaking Telegram webhook compatibility

---

## Coding Conventions

### Rust

- Prefer explicit types over inference in public APIs
- Keep handlers small and composable
- Place command handlers in dedicated modules
- Document public functions with doc comments

Example:

```rust
/// Handles the `/start` Telegram command.
///
/// Sends a welcome message explaining available commands.
async fn start_command(ctx: &BotContext, msg: &Message) -> HandlerResult {
    // ...
}
```

---

### Environment Variables

All runtime configuration **must** be provided via environment variables.

Common variables:

```text
TELEGRAM_BOT_TOKEN
STATIONS_TABLE_NAME
AWS_REGION
ALLERTA_API_BASE_URL
```

Agents must not introduce hardcoded values.

---

## Telegram Behavior Rules

Agents should be aware that:

- The bot ignores non-command messages in group chats
- Free-text messages are interpreted as station names
- Station matching uses fuzzy search
- Replies must be short and human-readable
- Markdown formatting should be Telegram-safe

---

## Data Handling Rules

- Station identifiers are stable and used as primary keys
- Water level values may be missing or stale
- Fetcher should never delete stations unless explicitly required
- Bot must handle empty or partial results gracefully

---

## Approved Improvements Agents May Implement

### 1. Fuzzy Matching Improvements

Agents may enhance station matching by:

- Normalizing accents and punctuation
- Tokenizing station names
- Adding prefix or partial matching

Example normalization helper:

```rust
fn normalize_station_name(name: &str) -> String {
    name.to_lowercase()
        .replace(['-', '_', '/'], " ")
        .trim()
        .to_string()
}
```

---

### 2. In-Memory Caching (Safe Use)

Agents may introduce **bounded, best-effort** in-memory caching:

- Cache must be optional
- Cache must tolerate cold starts
- Cache must not be required for correctness

---

### 3. Observability

Agents are encouraged to:

- Add structured logs
- Log command usage and errors
- Avoid logging PII

Preferred logging style:

```rust
log::info!(
    target: "erfiume_bot",
    "command=station_lookup station={} user={}",
    station_name,
    user_id
);
```

---

### 4. Tests

Agents may add:

- Unit tests for parsing and matching logic
- Mocked integration tests for handlers
- CLI test helpers (optional)

Tests must not require real AWS or Telegram credentials.

---

## Forbidden Changes

Agents must **not**:

- Commit secrets or tokens
- Replace Pulumi with another IaC tool
- Change DynamoDB schema without migration logic
- Introduce blocking I/O in Lambda handlers
- Add heavy dependencies without justification

---

## Example Agent Prompt Guidance

When generating code, agents should assume:

```text
This is a Rust AWS Lambda Telegram bot.
Handlers must be async.
All configuration comes from environment variables.
External APIs may fail or return incomplete data.
```

---

## Suggested Additional Files

Agents may propose or add:

- `CONTRIBUTING.md`
- `SECURITY.md`
- `ARCHITECTURE.md`
- `CHANGELOG.md`

---

## Final Notes

This repository is production-facing and interacts with external users.
Agents should prioritize **correctness**, **resilience**, and **clarity** over cleverness.

When in doubt:

- Prefer simple solutions
- Preserve existing behavior
- Add documentation
