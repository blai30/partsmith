# partsmith-message-lint

The message lint of the error guidelines (`docs/guidelines/errors.md`, "Checklist"), shared by the tests of `partsmith-occt-sys` (kernel.md test 130), `partsmith-sketch` (sketch.md test 247), `partsmith-features` (features.md section 3.12 and test 14) and `partsmith-recompute` (the messages of its document tests, recompute.md test 19 and the explanation clauses of section 3.10). Dev-only (ADR 0027): no dependencies, `publish = false`, used only as a dev-dependency of tests.

## Public interface (crate root)

- `lint_failures(message) -> Vec<&'static str>`: the rules a message breaks, each by the pattern it checks (`repair panel`, `\bop (new|join|cut|intersect)\b`, `\b[fbec]-[0-9a-z]+\b`, `(Edge|Face|Vertex|Curve) [0-9]`, `[0-9]+% match`, `a backtick`, `os error`), in that order; empty when the message passes. Word boundaries are those of a regular expression's `\b`, with `\w` as ASCII letters, digits and `_`.
- `assert_passes_lint(message)` (`#[track_caller]`): panics with the message and its failures when `lint_failures` is not empty.

## Rules

- No dependencies; the crate is a dev-dependency used by tests only, never a dependency of a library or binary.
- The patterns are the guidelines' checklist item word for word; a change to them is a change to the guidelines and to every spec test that runs the lint.
- Words a crate's own spec test adds to the lint (kernel.md test 130's exception names, sketch.md test 247's ids of the test's sketch) stay in that crate's tests.

## Commands

```sh
cargo test -p partsmith-message-lint
cargo clippy -p partsmith-message-lint --all-targets -- -D warnings
cargo fmt -p partsmith-message-lint
```
