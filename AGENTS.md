# Quest (Rust)

Quest in Rust: macOS client (Rune UI framework), Android client and sync server. Read `docs/ARCHITECTURE.md` first.

- Crates live in `crates/`. Shared dependency versions go in the root `[workspace.dependencies]`.
- `quest-domain` stays pure (no IO, no clocks: time is passed in). Policy goes there, side effects elsewhere.
- Done means: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass.
- Keep Rust files under ~800 lines; tests in sibling `tests.rs` / `*_tests.rs` modules.
- Rune: pinned git rev in the root manifest; its docs are in its repo (`crates/rune/README.md`, `crates/rune-kit`, `examples/`). Never do file IO on the UI thread.
