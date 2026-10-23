# CLAUDE.md

Remindr: local-only Notion alternative. Rust, GPUI-kit (`gpui-kit = "=0.8.0"`, gpui-pre 0.3.9), SQLite.

## Crates

- `crates/core` (`remindr-core`): domain and application. Depends on `uuid` and `thiserror` only. No gpui, no SQLite, no serde.
- `crates/store` (`remindr-store`): SQLite adapter of `WorkspaceStore` (rusqlite, synchronous). DTOs and JSON codecs live here.
- `crates/desktop` (`remindr-desktop`, binary `remindr`): GPUI-kit app and composition root.

Direction: `desktop → store → core`, `desktop → core`. Nothing points into `desktop`.

## Where things live

- Domain types: `crates/core/src/domain/`. Errors: `domain/error.rs`.
- Every change is a `Transaction` of `Operation`s: `domain/transaction.rs`. `Workspace::apply` returns the inverse transaction.
- User intents: `Command` in `application/command.rs`, turned into transactions by `application/blocks.rs` and `application/databases.rs`, executed by `Session` in `application/session.rs`.
- Ports: `crates/core/src/ports.rs`. In-memory store, manual clock and the store contract suite: `crates/core/src/testing/` (feature `test-support`).
- Migrations: `crates/store/migrations/`, version in `PRAGMA user_version`.
- UI seams: `crates/desktop/src/app/` (model, navigator, store enum), `embed.rs` (database view embedding).

## Rules

- No code comments of any kind. Names carry meaning; the why goes in the commit or PR.
- No `async-trait`. Static dispatch first, enum second, `dyn` only with a stated reason.
- Never commit anything under `docs/`.
- Branches `feat/<topic>` open PRs into `next`. `main` is never touched directly.

## Verify (exit)

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo test --doc --workspace
! cargo tree -p remindr-desktop -e normal | grep -E ' (reqwest|hyper|ureq|isahc) v'
```

Loop: `cargo test -p remindr-core <module>::`, `cargo test -p remindr-store`, `cargo test -p remindr-desktop <module>::`.
