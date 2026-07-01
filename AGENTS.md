# Agent Instructions

## Verification

Before wrapping up code changes, run the local verification commands that match this repo's CI expectations as closely as possible:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --lib --bins --tests -- -D warnings
cargo test --workspace --lib --bins --tests
```

## Clippy Scope

Prefer the `--lib --bins --tests` Clippy scope above for local verification.

Reason: on this Windows setup, `cargo clippy --workspace --all-targets -- -D warnings` can fail on the `dais` example target because of a filesystem permission error while writing under `target\debug\examples`, even when the code itself is clean.

If that environment issue is resolved later, it is fine to expand the local verification command to include `--all-targets`.

## Documentation

New features should be documented in the `docs/` directory.
All docs should be written as features, not as proposals.
Avoid proposal language like "should" or "will".
Features and bug fixes should contain a short, scannable note in `NEWS.md` in similar style to existing items.

If a feature was introduced in the same version before the final version is released and later a bug is fixed, then these changes should not be documented as bug fixes.

Markdown should be written to be human-readable.
There should be no line breaks within sentences and every sentence should be followed by a line break.
