# Agent instructions

This is a Rust repo with a zero-tolerance slop gate.

Always:
1. Read README and crate layout before editing.
2. Prefer small files. No inline `mod foo { ... }`. New modules are `foo.rs` or `foo/mod.rs`.
3. After every coding turn run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
4. Do not use unwrap/expect/panic/todo! in non-test code. Use Result + useful error types already in the crate.
5. Do not add .clone() or 'static to silence the compiler. Fix ownership.
6. Do not add dependencies without saying why.
7. Install commands in docs are plain `curl -LsSf <url> | sh`, with no `--proto` or `--tlsv1.2` flags. This covers README and every other doc, whatever cargo-dist suggests.
8. Skills — read only when needed:
   - .agents/skills/rust-anti-slop.md  after every coding turn
   - .agents/skills/rust-architecture.md  before non-trivial changes
   - .agents/skills/rust-errors.md  when touching Result / public APIs
   - .agents/skills/rust-review.md  before opening a PR

Never edit files in target/. Never commit allow(clippy::unwrap_used) without a reason.
