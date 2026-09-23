# Rust anti-slop pass

Run after every full coding turn.

Fix, in this order:
1. cargo fmt --all
2. cargo clippy --workspace --all-targets --fix --allow-dirty -- --no-deps
   then cargo clippy --workspace --all-targets -- -D warnings
3. Remove unwrap/expect/panic/todo/unimplemented from non-test code
4. Remove needless .clone(), .to_string(), .to_owned(), .clone() on Copy, clone of Arc/Rc
5. Remove invented 'static lifetimes; prefer borrows or owned values the crate already uses
6. Delete leftover comments: TODO, FIXME, "for now", "simple implementation", "should work"
7. Delete dead code, unused imports, unused deps (do not run udeps unless cargo-udeps is installed)
8. Split any new file > 500 lines
9. Convert any new inline module into a file
10. cargo test --workspace

Do not rewrite working code for taste. Only remove slop.
