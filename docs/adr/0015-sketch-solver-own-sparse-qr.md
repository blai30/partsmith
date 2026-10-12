# 0015: Sketch solver uses its own sparse rank-revealing QR instead of faer

Status: Accepted, needs human review. Supersedes the addendum of ADR 0004 and the `faer` row of ADR 0012.

## Context

ADR 0004's addendum chose `faer` for the solver's sparse and dense factorizations, and ADR 0012 logged it. Implementing sketch.md showed that none of faer's QR factorizations gives the rank test the spec defines (section 3.8): a sequential, unpivoted test in row order in which a dependent row gets no reflector. `qr` factors every column, `col_piv_qr` pivots, and the sparse QR reorders columns to reduce fill, so each one changes which rows are accepted. The 4 ms drag budget on the 500-constraint chain also needs the factorization plan (which reflectors reach which rows) computed once from the sparsity pattern and reused across every factorization of a solve and every frame of a drag.

## Decision

`partsmith-sketch` implements its own sparse Householder QR in `src/solver/sparse_qr.rs`. It factors `J^T` column by column in row order without pivoting, accepts or rejects each row with the test of sketch.md section 3.8 (`RANK_TOLERANCE` times the larger of the row's original norm and 1), and reuses its per-column plans across factorizations with the same sparsity pattern. `faer` is not a dependency of any crate.

## Consequences

No new dependency, and the accepted rows match the spec's definition exactly, which the diagnosis tests and the property tests check. The crate owns about one more module of numerical code, covered by unit tests against a dense modified Gram-Schmidt reference and by the solver benchmark (all seven budgets pass). If a later solver needs general sparse factorizations, adding a library needs a new ADR.
