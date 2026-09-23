# Errors

- Library code returns Result. No unwrap/expect/panic on recoverable paths.
- Map errors with context; never map_err(|_| ... ) that drops the source.
- indexing_slicing is denied: use .get() / reliable invariant + expect with reason only if Clippy is not denying expect. Prefer if let / match.
- Document unsafe with a SAFETY comment; undocumented_unsafe_blocks is denied.
