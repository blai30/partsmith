# W2-1A-2: Fit spline interpolation and the control-point spline tool's defaults

Status: Accepted (owner's rule of 2026-11-05): the M2 plan's open choices "Fit-spline interpolation" and "Defaults of the control-point spline tool", decided as the wave 2 decisions of 2026-11-19 accept them, with the interpolation checked against OCCT 7.9.3's `Geom2dAPI_Interpolate` as the plan asked. It fixes what a stored `fit_spline` record means, and what a `bspline` record the tool writes holds, in format version 3 ([ADR 0061](0061-format-version-3-for-the-records-of-m2.md), [ADR 0066](0066-sketch-entities-of-format-version-3-operations-and-display-polylines.md)). Both go to human review at the M2 gate with format version 3 (m2-plan.md gate 19).

## Context

- **The record.** A `fit_spline` stores its point entities and `periodic` only. ADR 0066 alternative 3 makes it cubic, its degree not stored. sketch.md section 3.20.5 asks item 14's spec for the parameter of each point, the knot rule and the end conditions, exact and deterministic from the points alone.
- **OCCT's interpolator.** `Geom2dAPI_Interpolate` (read in OCCT 7.9.3's `src/Geom2dAPI/Geom2dAPI_Interpolate.cxx`, `BuildParameters`, `BuildTangents` and `PerformNonPeriodic`) uses cumulative chord-length parameters starting at 0. For two points it builds a line, for three a quadratic with knots at the first and last parameter, and from four points a cubic with a knot at every parameter whose end derivatives are those of the cubic Lagrange polynomial through the first four and the last four points. Its periodic path estimates a tangent at the first point only, so its curve depends on which point comes first.
- **The tool.** The control-point spline tool writes a `bspline` with a degree and knots (sketch.md section 3.21.1). Fusion and FreeCAD draw cubic splines with clamped uniform knots by default.

## Decision

1. **Open fit splines interpolate as OCCT does.** Parameters are cumulative chord lengths from 0. Two points give degree 1 and three points degree 2, through the points at their parameters. From four points the spline is cubic with a knot at every parameter, `n + 2` poles, and end derivatives equal to those of the cubic Lagrange polynomial through the first four (last four) points at their parameters. Consecutive points within `PROFILE_TOLERANCE` count once. The solve is an in-house banded elimination in a fixed order (sketch.md section 3.20.5).
2. **Periodic fit splines** are the C2 periodic cubic interpolant with knots at the chord-length parameters of the closed polygon, its period the polygon's length. It needs no end condition, and rotating the list of points gives the same curve.
3. **The control-point spline tool** writes degree `min(3, poles - 1)`, clamped uniform knots on the integers for an open spline, `0, 1, ..., poles` for a periodic one, and no weights. The tool has a periodic option and no degree field.

## Alternatives considered

Each choice is Accepted (owner's rule of 2026-11-05): the most open-source approach, then the safest, then the industry standard, then this work's recommendation. Every option is in-house Rust under Apache-2.0 and deterministic, so the first two criteria tie unless a choice says otherwise.

1. *Parameters.* (a) Chord length; (b) centripetal; (c) uniform. The rule takes (a), the industry standard: OCCT's interpolator and most CAD use it, so a fit spline built here and one built by OCCT from the same points agree.
2. *End conditions of an open spline.* (i) Not-a-knot; (ii) natural; (iii) the end derivatives of the cubic Lagrange polynomial through four end points, OCCT's rule. The rule takes (iii), the industry standard of the kernel this product uses. The plan recommended (i) "checked against OCCT's Geom2dAPI_Interpolate before writing". The check found that OCCT uses (iii). (iii) also gives each end tangent in closed form from four points.
3. *Periodic splines.* (i) The C2 periodic cubic interpolant; (ii) OCCT's periodic path. The rule takes (i), the safest: its curve does not depend on which point is first, which a user cannot see in a closed loop of points. (ii) estimates one tangent at the first point.
4. *Repeated points.* (i) Consecutive points within the tolerance count once; (ii) the record is invalid. The rule takes (i), the safest: a solve that makes two fit points meet does not make a valid record invalid. The tool refuses such points (`RepeatedPoint`).
5. *The tool's degree.* (a) `min(3, poles - 1)` with clamped uniform knots; (b) a degree the user picks in the tool. The rule takes (a), the industry standard of Fusion and FreeCAD, and fewer options on screen (engineering principle 5). The inspector can edit a stored spline's degree and knots once item 15's spec allows it.
6. *The tool's periodic option.* (i) A toggle; (ii) closing on the first pole. The rule takes (i), the recommendation: a click on the first pole stays a snap, as in M1's line tool.

## Consequences

- Specs: sketch.md sections 3.20.5 and 3.21.1, and tests from 342 (exact cases: points on a line, a symmetric periodic case and the knot vectors).
- Gate 19 reviews what `fit_spline` and a tool-written `bspline` mean, with format version 3.
- kernel.md receives the B-spline as poles, weights and knots and never interpolates (m2-plan.md choice 11).
