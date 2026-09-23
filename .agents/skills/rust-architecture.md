# Architecture

- One concern per crate / module.
- Public API small. Keep internals private.
- No mega-structs (>16 fields) without a written reason.
- No god files. Feature folders over kitchen-sink lib.rs.
- Errors: use the crate's existing error type. Do not add anyhow in a library unless it already uses anyhow. Do not add thiserror if the crate already has an error enum.
- Do not introduce new async runtimes or HTTP clients if one already exists.

Read Cargo.toml members and src/lib.rs / src/main.rs before adding files.
