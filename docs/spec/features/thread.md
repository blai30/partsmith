# Feature: thread

Status: M2 spec, wave 2. Type name `thread`. Shared contracts: [features.md](../features.md). Kernel: [kernel.md](../kernel.md) sections 2.15 and 3.4.14. Stage 2 of wave 2's spec round (track 2D) writes this spec's foundations: the thread and clearance-hole tables, the size designations, the modeled print profile, the clearance and the meaning of a cosmetic thread (sections 1 to 9, tests 1 to 20, open issues 1 to 3), by [ADR W2-2D-1](../../adr/W2-2D-1-thread-and-clearance-hole-tables-designations-and-editions.md) to [ADR W2-2D-4](../../adr/W2-2D-4-cosmetic-threads-and-head-sizes-and-inch-clearance-holes-wait-for-m3.md). Track 3A adds the `thread` feature in stage 3: its params, manifest, build steps, element names, messages and examples in sections from 10 on, its tests from 21 and its open issues from 4, and [features/hole.md](hole.md) adds tapped and clearance holes on the same tables (the M2 plan's items 20 and 21). Sections 1 to 9 keep their numbers. The acceptance tests and the open issues follow track 3A's sections, and other files cite them by number, not by the section that holds them. Every choice here is Accepted (owner's rule of 2026-11-05), as the wave 2 decisions of 2026-11-19 accept it ([m2-wave2-decisions.md](../../design/m2-wave2-decisions.md) section 2 items 5, 7 and 16, and section 3).

## 1. Purpose and scope

A thread turns a round face into a screw thread that fits a standard part or another printed part. It is modeled by default and cosmetic on request, external on a shaft or internal in a bore (technical design sections 8 and 11). Tapped and clearance holes use the same tables and the same profile (features/hole.md).

### In M2

- The size tables of section 3, each with its source: ISO metric coarse and fine threads of ISO 262's selected sizes M1 to M64 with ISO 261's pitches, classes 6g and 6H (ISO 965-1); unified UNC threads from #1 to 1-1/2 and UNF threads from #0 to 1-1/2, classes 2A and 2B (ASME B1.1); and ISO 273 clearance holes for M1 to M64, close, normal and loose. These are the M2 plan's choice 16 (m2-plan.md section 7.3 item 16).
- Size designations (`M8x1.25`, `1/4-20 UNC`, `M3`), their lookup in the core's tables and a fix for a size written another way (section 4).
- The modeled profile: the maximum-material limit of the class, the printer's clearance per side and a truncation of every crest and root that does not depend on the printer (section 5).
- The clearance as an expression whose start value is `printer.fit.slip` (section 6, features.md section 3.3).
- Cosmetic threads: the geometry stays as it is, and a cosmetic tapped hole bores at the size's minor diameter (section 7).
- Thread flanks in the overhang check (print.md section 3.9.5).
- What a host needs from the tables (section 8). Track 3A writes the manifest member and the protocol route that carry them.
- Track 3A, stage 3: the `thread` feature on cylindrical faces, its record in format version 3, its names, its messages and its examples; tapped and clearance holes in features/hole.md.

### Later

- Per-screw head presets (ISO 4762 counterbores, ISO 10642 countersinks) and inch clearance holes (ASME B18.2.8): M3, with the print presets (m2-plan.md section 5.1, ADR W2-2D-4). Counterbored and countersunk holes take explicit head sizes in M2.
- Other classes (4h, 6e, 3A, 3B and the rest), and tolerance widths: M2 models one limit of one class per kind (section 5.2). A class choice would be an optional key whose absence means section 5.2's classes.
- Other sizes and standards: ISO 261 sizes outside ISO 262's selection, UNEF and UNS, trapezoidal, Acme and pipe threads.
- Thread decorations in the viewport and thread data in exported files (STEP AP242 PMI).

## 2. Public API

The module `partsmith_features::threads` (`crates/partsmith-features/src/threads/`) holds the tables, the lookups and the profile. The `thread` feature (track 3A) and the `hole` feature use it. It is pure Rust and calls no kernel. Lengths are millimeters. `ThreadKind` is the kernel's `External` or `Internal` (kernel.md section 2.15), re-exported.

```rust
/// A standard of the thread tables, stored in a record as `iso_metric` or `unified`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThreadStandard {
    /// ISO metric threads (ISO 261, ISO 262, ISO 965-1).
    IsoMetric,
    /// Unified inch threads (ASME B1.1).
    Unified,
}

/// A series of a thread table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ThreadSeries {
    /// ISO metric coarse: each D of section 3.1 with its coarse pitch.
    MetricCoarse,
    /// ISO metric fine: each D of section 3.1 with one of its fine pitches.
    MetricFine,
    /// Unified coarse (UNC).
    Unc,
    /// Unified fine (UNF).
    Unf,
}

/// One row of the thread tables (section 3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThreadSize {
    /// The text a record stores (section 4.1).
    pub designation: &'static str,
    /// The standard whose table holds the row.
    pub standard: ThreadStandard,
    /// The series of that table the row belongs to.
    pub series: ThreadSeries,
    /// The basic major diameter D.
    pub major_diameter: f64,
    /// The pitch P.
    pub pitch: f64,
    /// Threads per inch of a unified size, `None` for an ISO metric one.
    pub threads_per_inch: Option<u32>,
}

impl ThreadSize {
    /// H = sqrt(3) / 2 P, the height of the fundamental triangle.
    pub fn fundamental_height(&self) -> f64;
    /// D1 = D - 5 sqrt(3) / 8 P, the basic minor diameter.
    pub fn basic_minor_diameter(&self) -> f64;
    /// D2 = D - 3 sqrt(3) / 8 P, the basic pitch diameter.
    pub fn basic_pitch_diameter(&self) -> f64;
    /// The class M2 models for `kind`: "6g", "6H", "2A" or "2B" (section 5.2).
    pub fn class(&self, kind: ThreadKind) -> &'static str;
    /// How far the maximum-material limit of that class lies inside the basic profile, on the
    /// diameter (section 5.2); 0 for an internal thread.
    pub fn max_material_deviation(&self, kind: ThreadKind) -> f64;
}

/// Every thread size in table order (section 3.4).
pub fn thread_sizes() -> &'static [ThreadSize];

/// The size of `standard` that `text` names (section 4.2).
pub fn find_thread_size(standard: ThreadStandard, text: &str) -> Result<&'static ThreadSize, SizeError>;

/// An ISO 273 fit, stored in a record as `close`, `normal` or `loose`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClearanceFit {
    /// ISO 273's fine series.
    Close,
    /// ISO 273's medium series.
    Normal,
    /// ISO 273's coarse series.
    Loose,
}

/// One row of the ISO 273 table (section 3.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClearanceHoleSize {
    /// The text a record stores, such as "M3".
    pub designation: &'static str,
    /// The nominal diameter of the screw.
    pub nominal_diameter: f64,
    /// The hole's diameter for each fit, in the order close, normal, loose.
    pub diameters: [f64; 3],
}

impl ClearanceHoleSize {
    /// The table's diameter for `fit`.
    pub fn diameter(&self, fit: ClearanceFit) -> f64;
}

/// Every clearance hole size in table order.
pub fn clearance_hole_sizes() -> &'static [ClearanceHoleSize];

/// The clearance hole size that `text` names (section 4.3).
pub fn find_clearance_hole_size(text: &str) -> Result<&'static ClearanceHoleSize, SizeError>;

/// The modeled diameter of a clearance hole: the table's diameter plus the clearance on each side,
/// or `None` when `clearance` is not a number of at least 0 (section 5.6).
pub fn clearance_hole_diameter(size: &ClearanceHoleSize, fit: ClearanceFit, clearance: f64) -> Option<f64>;

/// Why a text names no row of the table it was looked up in. Its `Display` is for logs and
/// tests. A user sees the feature's message of section 9, with the field's label and text.
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
pub enum SizeError {
    /// The text is, exactly as written, a thread size of the other standard.
    #[error("the text is a thread size of the other standard")]
    OtherStandard {
        /// The standard whose table holds the text.
        standard: ThreadStandard,
    },
    /// The text writes exactly one size of the table another way (section 4.2).
    #[error("the text writes the size {designation} another way")]
    NearMatch {
        /// The designation of that size.
        designation: &'static str,
    },
    /// The text names no size of the table.
    #[error("the text names no size of the table")]
    Unknown,
}

/// The truncation of every crest and root beyond ISO 68-1's flats, as a fraction of the pitch:
/// sqrt(3) / 24, or H / 12 (section 5.4).
pub const TRUNCATION: f64 = 0.07216878364870322;

/// The modeled profile of one size, kind and clearance (section 5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeledProfile {
    /// The radius of the kernel tooth's inner side: the root of an external thread, the crest
    /// (the bore) of an internal one.
    pub inner_radius: f64,
    /// The radial distance from the inner side to the outer side.
    pub depth: f64,
    /// The width of the inner side along the axis.
    pub inner_width: f64,
    /// The width of the outer side along the axis.
    pub outer_width: f64,
    /// The modeled major diameter: the crest of an external thread, the root of an internal one.
    pub major_diameter: f64,
    /// The modeled minor diameter: the root of an external thread, the bore of an internal one.
    pub minor_diameter: f64,
    /// The modeled pitch diameter.
    pub pitch_diameter: f64,
}

/// The modeled profile, or `None` when `clearance` is not a number from 0 to
/// `max_clearance(size, kind)` (section 5.6).
pub fn modeled_profile(size: &ThreadSize, kind: ThreadKind, clearance: f64) -> Option<ModeledProfile>;

/// The largest clearance that leaves an external thread a root (section 5.6); `None` for an
/// internal thread: it has no bound of its own.
pub fn max_clearance(size: &ThreadSize, kind: ThreadKind) -> Option<f64>;

/// The bore of a cosmetic tapped hole: the size's basic minor diameter D1 (section 7).
pub fn cosmetic_bore_diameter(size: &ThreadSize) -> f64;
```

The tables are constant data compiled into the crate. No function reads a file, the document or the printer. `ThreadStandard` and `ClearanceFit` are stored in documents. They derive serde's traits and `JsonSchema` in snake_case (features.md section 2), and track 3A's params can hold them. A `ModeledProfile` gives the kernel's `ThreadTooth` its `depth`, `inner_width` and `outer_width`, and the radius of its `surface` (kernel.md section 2.15). Track 3A sets the tooth's frame, helix and length from the face, and the construction from spike 8.9's Results (kernel.md section 8.9).

## 3. Tables

### 3.1 ISO metric threads

ISO 262's selected sizes for screws, bolts and nuts, with ISO 261's coarse pitch and ISO 262's fine pitches for each, in millimeters. Each pitch of a row is one size, and its designation is `M{D}x{P}` (section 4.1).

| D | Coarse P | Fine P |
| --- | --- | --- |
| 1 | 0.25 | 0.2 |
| 1.2 | 0.25 | 0.2 |
| 1.4 | 0.3 | 0.2 |
| 1.6 | 0.35 | 0.2 |
| 1.8 | 0.35 | 0.2 |
| 2 | 0.4 | 0.25 |
| 2.5 | 0.45 | 0.35 |
| 3 | 0.5 | 0.35 |
| 3.5 | 0.6 | 0.35 |
| 4 | 0.7 | 0.5 |
| 5 | 0.8 | 0.5 |
| 6 | 1 | 0.75 |
| 7 | 1 | 0.75 |
| 8 | 1.25 | 1, 0.75 |
| 10 | 1.5 | 1.25, 1 |
| 12 | 1.75 | 1.5, 1.25 |
| 14 | 2 | 1.5 |
| 16 | 2 | 1.5 |
| 18 | 2.5 | 2, 1.5 |
| 20 | 2.5 | 2, 1.5 |
| 22 | 2.5 | 2, 1.5 |
| 24 | 3 | 2 |
| 27 | 3 | 2 |
| 30 | 3.5 | 2 |
| 33 | 3.5 | 2 |
| 36 | 4 | 3 |
| 39 | 4 | 3 |
| 42 | 4.5 | 3 |
| 45 | 4.5 | 3 |
| 48 | 5 | 3 |
| 52 | 5 | 4 |
| 56 | 5.5 | 4 |
| 60 | 5.5 | 4 |
| 64 | 6 | 4 |

That is 34 coarse sizes and 40 fine ones. The basic dimensions follow ISO 68-1's basic profile and ISO 724's formulas: `H = sqrt(3) / 2 P`, `D1 = D - 5 sqrt(3) / 8 P` (ISO 724 prints `D - 1.082532 P`), `D2 = D - 3 sqrt(3) / 8 P` (`D - 0.649519 P`). The external thread's d, d1 and d2 equal D, D1 and D2 in the basic profile.

### 3.2 Unified threads

ASME B1.1's UNC series from #1 to 1-1/2 and UNF series from #0 to 1-1/2. A numbered size N has the basic major diameter `0.060 + 0.013 N` in.

| Size | D (in) | UNC threads per inch | UNF threads per inch |
| --- | --- | --- | --- |
| #0 | 0.060 | | 80 |
| #1 | 0.073 | 64 | 72 |
| #2 | 0.086 | 56 | 64 |
| #3 | 0.099 | 48 | 56 |
| #4 | 0.112 | 40 | 48 |
| #5 | 0.125 | 40 | 44 |
| #6 | 0.138 | 32 | 40 |
| #8 | 0.164 | 32 | 36 |
| #10 | 0.190 | 24 | 32 |
| #12 | 0.216 | 24 | 28 |
| 1/4 | 0.2500 | 20 | 28 |
| 5/16 | 0.3125 | 18 | 24 |
| 3/8 | 0.3750 | 16 | 24 |
| 7/16 | 0.4375 | 14 | 20 |
| 1/2 | 0.5000 | 13 | 20 |
| 9/16 | 0.5625 | 12 | 18 |
| 5/8 | 0.6250 | 11 | 18 |
| 3/4 | 0.7500 | 10 | 16 |
| 7/8 | 0.8750 | 9 | 14 |
| 1 | 1.0000 | 8 | 12 |
| 1-1/8 | 1.1250 | 7 | 12 |
| 1-1/4 | 1.2500 | 7 | 12 |
| 1-3/8 | 1.3750 | 6 | 12 |
| 1-1/2 | 1.5000 | 6 | 12 |

That is 23 UNC sizes and 24 UNF ones. Each is stored in millimeters: `major_diameter` is D (in) times 25.4 and `pitch` is 25.4 divided by the threads per inch, both computed in `f64` from these decimals. ASME B1.1's basic form is ISO 68-1's. H, D1 and D2 follow section 3.1's formulas (B1.1 prints `D - 1.082532 P` for the basic minor diameter).

### 3.3 ISO 273 clearance holes

ISO 273's clearance holes, in millimeters, for the diameters of section 3.1. The fine, medium and coarse series are the fits close, normal and loose. A row's designation is `M{D}`.

| Screw | Close | Normal | Loose |
| --- | --- | --- | --- |
| M1 | 1.1 | 1.2 | 1.3 |
| M1.2 | 1.3 | 1.4 | 1.5 |
| M1.4 | 1.5 | 1.6 | 1.8 |
| M1.6 | 1.7 | 1.8 | 2 |
| M1.8 | 2 | 2.1 | 2.2 |
| M2 | 2.2 | 2.4 | 2.6 |
| M2.5 | 2.7 | 2.9 | 3.1 |
| M3 | 3.2 | 3.4 | 3.6 |
| M3.5 | 3.7 | 3.9 | 4.2 |
| M4 | 4.3 | 4.5 | 4.8 |
| M5 | 5.3 | 5.5 | 5.8 |
| M6 | 6.4 | 6.6 | 7 |
| M7 | 7.4 | 7.6 | 8 |
| M8 | 8.4 | 9 | 10 |
| M10 | 10.5 | 11 | 12 |
| M12 | 13 | 13.5 | 14.5 |
| M14 | 15 | 15.5 | 16.5 |
| M16 | 17 | 17.5 | 18.5 |
| M18 | 19 | 20 | 21 |
| M20 | 21 | 22 | 24 |
| M22 | 23 | 24 | 26 |
| M24 | 25 | 26 | 28 |
| M27 | 28 | 30 | 32 |
| M30 | 31 | 33 | 35 |
| M33 | 34 | 36 | 38 |
| M36 | 37 | 39 | 42 |
| M39 | 40 | 42 | 45 |
| M42 | 43 | 45 | 48 |
| M45 | 46 | 48 | 52 |
| M48 | 50 | 52 | 56 |
| M52 | 54 | 56 | 62 |
| M56 | 58 | 62 | 66 |
| M60 | 62 | 66 | 70 |
| M64 | 66 | 70 | 74 |

ISO 273 also lists sizes outside ISO 262's selection, such as M4.5. They are not in the table. The hole's clearance adds to these diameters (section 6).

### 3.4 Order, sources and provenance

`thread_sizes()` lists the ISO metric sizes first, by D, each D's coarse size before its fine sizes, fine sizes by decreasing pitch (`M8x1.25`, `M8x1`, `M8x0.75`). The unified sizes follow by D, the UNC size before the UNF size of the same D (`#0-80 UNF`, `#1-64 UNC`, `#1-72 UNF`). `clearance_hole_sizes()` lists section 3.3's rows in order.

The rows were written from ISO 68-1:1998 (basic profile), ISO 261:1998 (general plan), ISO 262:1998 (selected sizes), ISO 724:1993 (basic dimensions), ISO 965-1:2013 (tolerances: the fundamental deviations of section 5.2), ASME B1.1-2019 (unified inch screw threads: sizes, basic form and the allowance of section 5.2) and ISO 273:1979 (clearance holes). ISO has since replaced some of these editions (ISO 68-1:2023, ISO 262:2023 and ISO 724:2023 among them), and ISO 20273:2017 replaced ISO 273. Open issue 2 checks every row against the current edition of each standard before the edition of section 3.5 is fixed. The dimensions are facts. They are kept in the repository under Apache-2.0 with these sources named, as the M2 plan's choice 16 states. Nothing is copied from another library's tables. Tests 4 to 6 recompute every derived value from its formula and check it against values the standards print. Test 20 pins every row. The sourcing goes to human review at the M2 gate.

### 3.5 Editions

These rows and section 5's formulas are the tables' M2 edition. The edition is fixed from the first merge of code that builds a stored size: track 3A's `thread` or tapped hole, whichever merges first. From then on a version 3 build writes records that store designations, and file-format.md section 3.9 rule 2 keeps the meaning of every value such a build has written. A stored designation then keeps its geometry in every later release. The M2 gate's review of format version 3 freezes the edition for good (m2-plan.md gate 19). Open issue 2's check of every row against the standards' texts comes before that first merge. Once the edition is fixed:

- A new row (a size the edition refuses) may be added in any release. A record whose size failed as unknown builds once its row exists. That is the one change a new row makes. An older build reports section 9's message for an unknown size on such a record.
- A corrected row or formula is a new edition, before the gate as after it. A record selects it through an optional key, added with the correction, whose absence means the M2 edition. Records without the key keep building the M2 edition's geometry. Before the gate the key joins version 3's row (file-format.md section 3.9 rules 1 and 2). After the gate it is a new key in the params of an existing feature type. Such a key needs a new format version (file-format.md section 3.9, what needs a new version after the gate).

Until a correction exists, no record holds an edition key.

## 4. Designations

### 4.1 Spelling

- ISO metric: `M`, the major diameter, `x`, the pitch: `M8x1.25`, `M1.6x0.35`, `M30x2`, `M10x1`. Numbers are written in decimal with no trailing zeros and no trailing point. Every designation names its pitch, the coarse one included.
- Unified: the size, `-`, the threads per inch, a space, the series: `1/4-20 UNC`, `#10-24 UNC`, `#0-80 UNF`, `1-8 UNC`, `1-1/2-6 UNC`, `1-12 UNF`. A numbered size starts with `#`. A size of a whole inch and a fraction writes the two joined by `-`.
- ISO 273: `M` and the screw's major diameter: `M3`, `M1.6`, `M64`.

The class is not part of a designation: section 5.2 gives one class per standard and kind.

### 4.2 Looking up a thread size

`find_thread_size(standard, text)` applies these rules in order:

1. `text` equals a designation of `standard`, byte for byte: that row.
2. `text` equals a designation of the other standard, byte for byte: `OtherStandard` with that standard.
3. `text` writes exactly one row of `standard` another way: `NearMatch` with that row's designation. The text is first normalized: ASCII letters in upper case, every whitespace character (`char::is_whitespace`) removed, `×` (U+00D7) written `X`, and `,` written `.`. Then:
   - ISO metric: the normalized text is `M`, a decimal number D, and optionally `X` and a decimal number P (each a run of digits with at most one `.` among them, at least one digit). Without P it means D's coarse size. It matches the row whose `major_diameter` and `pitch` equal the numbers parsed as `f64`. `M8.0X1.250` matches `M8x1.25`, and `M8` matches `M8x1.25`.
   - Unified: the normalized text is an optional `#`, a size S, `-`, a whole number N of threads per inch, and optionally `UNC` or `UNF`. S is a whole number, a fraction `a/b`, or a whole number, `-` and a fraction. The last `-` before N separates S from N. With `#`, S is the number of a numbered size and matches the row `#S`. Without `#`, S is first read as inches: its value in `f64` (a whole number, `a / b`, or the whole number plus `a / b`) times 25.4, computed as section 3.2 computes `major_diameter`, equals the `major_diameter` of the rows it matches. Every D of section 3.2 above #12 is a whole number of sixteenths, exact in `f64`. The product is then the stored value bit for bit (`3/8` gives 9.524999999999999, as `3/8-16 UNC` holds). Dividing `major_diameter` by 25.4 would not give S back for `3/8`, `3/4` or `1-1/2`. When no row has that D and N, S is read as the number of a numbered size. The text matches the row of that size and N. A series given must be that row's series. `1/4-20` and `1/4-20unc` match `1/4-20 UNC`, `10-24 UNC` and `#10-24` match `#10-24 UNC`, `1-12` matches `1-12 UNF`, and `1-64` matches `#1-64 UNC`.
4. Otherwise `Unknown`.

Rule 3 offers a row only where the text names it unambiguously: the size and pitch of an ISO thread, the size and threads per inch of a unified one. A size that is not in the table (`M9`, `M8x1.5`, `1/4-32 UNF`) gets no guess.

### 4.3 Looking up a clearance hole size

`find_clearance_hole_size(text)`: `text` equal to a designation of section 3.3 is that row. Otherwise the text, normalized as in section 4.2, of the form `M`, D and optionally `X` and a pitch, matches the row whose `nominal_diameter` equals D, whatever the pitch: `NearMatch` with its designation (`M3x0.5` gives `M3`). Otherwise `Unknown`. `OtherStandard` is never returned: ISO 273 has no unified sizes (section 1, Later).

## 5. The modeled profile

### 5.1 The basic profile

ISO 68-1's basic profile, also ASME B1.1's basic form: a fundamental triangle of 60 degrees and height H, truncated at the major diameter to a flat of P/8 and at the minor diameter to a flat of P/4. In each half-plane through the axis, the external thread's tooth is 3P/4 wide at the minor diameter and P/8 wide at the major diameter, and 5H/8 deep. The internal thread's groove has the same shape: 3P/4 wide at the minor diameter and P/8 at the major diameter. Each flank makes 30 degrees with a radius of that half-plane: along the axis it moves `tan(30°) = 1 / sqrt(3)` per millimeter of radius.

### 5.2 The maximum-material limit of the class

A thread is modeled at the maximum-material limit of its class (the wave 2 decisions, section 2 item 5): the basic profile moved inward as a whole by half the deviation δ on the radius, flanks included. Every diameter of an external thread shrinks by δ.

| Standard | External | δ external | Internal | δ internal |
| --- | --- | --- | --- | --- |
| ISO metric | 6g | `(15 + 11 P) / 1000` mm, P in mm: ISO 965-1's fundamental deviation `es` of position g, `-(15 + 11 P)` µm | 6H | 0: position H's `EI` is 0 |
| Unified | 2A | ASME B1.1's allowance of class 2A in inches, times 25.4: 0.300 times the class 2A pitch diameter tolerance `T = 0.0015 D^(1/3) + 0.0015 sqrt(LE) + 0.015 P^(2/3)`, with D and P in inches and the length of engagement LE equal to D | 2B | 0: class 2B has no allowance |

These are the standards' formulas, computed unrounded. ISO 965-1 prints whole micrometers that differ from its formula by up to 1.5 µm on the pitches of section 3.1 (test 5), and B1.1 rounds to 0.0001 in (test 6). Tolerance widths are not modeled or stored. A thread is modeled at the limit where it holds the most material, the one at which it still fits a mating part of its class. `ThreadSize::class` gives the class's name, and `max_material_deviation` gives δ.

### 5.3 The clearance

The clearance `c` (section 6) moves the profile away from its mate, as a whole, by `c` on the radius: every diameter of an external thread shrinks by `2c` and every diameter of an internal thread grows by `2c`, flanks included. Each flank moves `c / sqrt(3)` along the axis (`tan(30°)` per millimeter of radius). At every radius a tooth is then `2c / sqrt(3)` narrower along the axis than at a clearance of 0, and a groove as much wider. Test 16 holds the gap between two modeled threads. Normal to the flanks, the gap between a flank and a standard mate's flank is `c / 2` (`sin(30°)` of the radial move).

### 5.4 The truncation

On top of the clearance, every crest is cut back and every root is cut deeper, each by `t = TRUNCATION x P = sqrt(3) / 24 P = H / 12` on the radius, beyond ISO 68-1's flats. Both remove material. The profile never crosses the maximum-material limit of section 5.2 and never meets a standard mate. The flanks do not move. The truncation does not depend on the printer. It holds on FDM and on resin alike, where `printer.nozzle` has no value (print.md section 3.5).

- An external thread's crest moves `t` toward the axis: the major diameter shrinks by `2t` and its flat widens to `5P/24`. Its root moves `t` toward the axis: the minor diameter shrinks by `2t`, the flat between two teeth narrows to `P/6` and the tooth's foot widens to `5P/6`. The root then lies at ISO 965-1's design minor diameter of an external thread, `d3 = d1 - H/6`, the source of the fraction ([ADR W2-2D-3](../../adr/W2-2D-3-the-modeled-thread-profile.md)).
- An internal thread's crest moves `t` away from the axis: the bore grows by `2t`, its flat widens to `P/3` and the groove's mouth narrows to `2P/3`. Its root moves `t` away from the axis: the major diameter grows by `2t` and the flat at the groove's bottom narrows to `P/24`.

### 5.5 The tooth

With D, P, D1 and D2 of the size, H its fundamental height, δ of section 5.2, `c` the clearance and `t = sqrt(3) / 24 P`, `modeled_profile` gives:

| Field | External (the tooth fused to a shaft) | Internal (the groove cut from a bore) |
| --- | --- | --- |
| `inner_radius` | `D1 / 2 - δ / 2 - c - t` (the root) | `D1 / 2 + c + t` (the crest, the bore) |
| `depth` | `5H / 8` | `5H / 8` |
| `inner_width` | `5P / 6` | `2P / 3` |
| `outer_width` | `5P / 24` | `P / 24` |
| `major_diameter` | `D - δ - 2c - 2t` | `D + 2c + 2t` |
| `minor_diameter` | `D1 - δ - 2c - 2t` | `D1 + 2c + 2t` |
| `pitch_diameter` | `D2 - δ - 2c` | `D2 + 2c` |

Every value is computed in `f64` in the order the formula writes it, with `sqrt(3)` as `3.0_f64.sqrt()`. The widths meet kernel.md section 3.4.14 item 1.3 for every size of section 3: the outer side is at least `P/24` wide, `inner_width - outer_width = 5P/8`, and `P - inner_width` is `P/6` or `P/3`, each far above `LINEAR_TOLERANCE` (test 17). The `thread` feature still checks the widths before `modeled_thread` (track 3A). A profile with a sharp crest or root then gets the feature's own message, not the kernel's `A thread needs an outer side wider than 0 mm.` (kernel.md section 3.4.14 item 1.3 and section 4.2's row "Thread outer width (M2)"). The kernel's tooth is the external thread's material or the internal thread's groove. The same function feeds `modeled_thread` with `ThreadKind::External` or `ThreadKind::Internal`.

M8x1.25 at `c = 0.2` mm, for example: an external thread with δ = 0.02875 mm has the major diameter 7.3908 mm, the minor diameter 6.0377 mm and the pitch diameter 6.7594 mm. An internal one has the bore 7.2273 mm and the major diameter 8.5804 mm. A standard 6g screw of 7.972 mm then reaches 0.372 mm into the printed bore on the radius, and the modeled external thread reaches 0.082 mm into the modeled internal one (open issue 1).

### 5.6 The range of the clearance

`modeled_profile` and `clearance_hole_diameter` return `None` for a clearance that is not finite or is below 0. A clearance below 0 would move a thread past its class's maximum-material limit into a standard mate, and make an ISO 273 hole smaller than the standard's. For an external thread, `max_clearance` is `D1 / 2 - δ / 2 - t - LINEAR_TOLERANCE`: the largest clearance that leaves the root at least `LINEAR_TOLERANCE` (1e-6 mm) from the axis. A larger clearance returns `None`. An internal thread and a clearance hole have no upper bound of their own, and the feature's modeling limit applies to the field (features.md section 3.4). The `thread` and `hole` features check the clearance against these bounds before they call `modeled_profile` or `clearance_hole_diameter`, with `OutOfRange` and the bound the check applies: `AtLeast(0)`, `AtMost` of `max_clearance` for an external thread, or the modeling limit (features.md section 3.4, track 3A). The bounds hold whatever the expression names. print.md section 3.1 allows a negative fit class (an interference press fit). On a profile whose `slip` is below 0, the start value `printer.fit.slip` gets that `OutOfRange` on the clearance field.

## 6. The clearance

[ADR W2-2D-2](../../adr/W2-2D-2-the-printer-clearance-of-threads-and-holes.md) amends [ADR 0065](../../adr/0065-feature-help-start-values-and-new-body-fields-in-the-manifests.md):

1. A modeled thread, a modeled tapped hole and an ISO 273 clearance hole take their clearance from an expression field with the unit `length`. Track 3A's params hold it, and its feature declares it as an `ExprUse` (features.md section 2.4). Printer values reach a build only through such declared expressions ([ADR 0069](../../adr/0069-component-scopes-by-containment-and-occurrence-paths.md) rule 7). A change of the document's printer or of its fit values rebuilds the feature through recompute's context names (recompute.md test 31).
2. The field's manifest start value is `{"kind": "expr", "expr": "printer.fit.slip"}`. Registration accepts it (features.md section 3.3, since this ADR). A user who types `0` models the class's maximum-material limit with the truncation. Any length expression of at least 0 works, `printer.fit.snug` or `0.15 mm` among them (section 5.6).
3. The value is the clearance per side, as print.md section 3.2 defines a fit class: the gap on the radius between a modeled surface and a standard part. It moves a modeled thread's major, minor and pitch diameters by `2c` (section 5.3), and adds `2c` to an ISO 273 hole's diameter (`clearance_hole_diameter`). It is at least 0 (section 5.6). `printer.fit.slip` is 0.2 mm on `generic-fdm` and 0.1 mm on `generic-resin` (print.md section 3.2).
4. Each printed thread is made to mate a standard part. Two printed threads that mate each other, each with its own clearance `c`, leave `2c` between them on the radius. A user who wants `c` between two printed parts types half of it on each, for example `printer.fit.slip / 2`.
5. A cosmetic thread has no clearance: it changes no geometry, and a cosmetic tapped hole bores at D1 (section 7).

## 7. Cosmetic threads

[ADR W2-2D-4](../../adr/W2-2D-4-cosmetic-threads-and-head-sizes-and-inch-clearance-holes-wait-for-m3.md):

1. **Geometry unchanged.** A cosmetic thread on a face leaves its body as it was: the body's shape after the feature is the shape before it, byte for byte in BRep, with the same volume, faces and element names. It adds no element names. References to the face keep resolving as before. Track 3A gives its `BodyChange`.
2. **The same checks.** A cosmetic thread holds the size, the face and the length that a modeled one holds, and fails on the same mistakes: an unknown size (section 9), a face that is not cylindrical (newcomer error corpus NC15), a size that does not fit its face (track 3A). It never calls `modeled_thread` and builds wherever its face exists. That is what kernel.md section 4.3's hint offers when a modeled thread fails ([ADR 0074](../../adr/0074-modeled-threads-lofted-between-helical-wires-and-divided-per-turn.md) decision 4).
3. **Display.** A cosmetic thread shows in the timeline and in the inspector. The viewport shows its face as it was, with no decoration and no display data of its own. The protocol does not change.
4. **Export and print checks.** STL, 3MF, OBJ and STEP files carry nothing of it: STEP AP242's thread data waits (section 1, Later). `analyze_print` checks the plain face.
5. **A cosmetic tapped hole** (features/hole.md, track 3A) bores at the size's basic minor diameter D1 (`cosmetic_bore_diameter`), with no clearance and no truncation: the hole a tap or a thread-forming screw needs after printing. An ISO 273 clearance hole has no cosmetic form.

## 8. What a host needs from the tables

The wave 2 decisions of 2026-11-19 (section 2 item 7) give track 3A the `FieldHint` member that names a core table and the protocol route that carries a table's rows to the UI and Python, under the protocol review. This section states what that route must carry:

1. **Which table a field takes.** The hint of a thread's or a tapped hole's size field names the thread table and the path of the field that holds the record's standard (the field `standard` of delivery.md test 167's `thread_demo`). The record stores the standard as the text `iso_metric` or `unified` ([ADR W2-2D-1](../../adr/W2-2D-1-thread-and-clearance-hole-tables-designations-and-editions.md) decision 3), and the size is looked up among the rows of that standard. The hint of a clearance hole's size field names the ISO 273 table and no standard path.
2. **The rows.** For a thread size: its designation, its standard as a record writes it (`iso_metric` or `unified`), its series (one value for each `ThreadSeries`), its major diameter D in mm, its pitch in mm, and for a unified size its threads per inch. For a clearance hole size: its designation, nominal diameter and its three diameters. A host shows the designation and may show the numbers beside it. Runner test 7 of delivery.md section 2.8 reads the designation, standard and series of each row through this route.
3. **The order** of section 3.4. A host keeps it.
4. **Once per session.** The rows depend on no document, printer or edit, and are the same in every session of one core build (section 3.5). A host fetches them once, as it fetches the feature catalog.
5. **Text in, geometry out.** A host stores the designation a user picks as the field's text, and computes nothing from the rows: the core turns a designation into geometry. A typed text that is no designation reaches the core unchanged, and the core reports section 9's message with its fix.
6. **Python** writes designations as strings, with no copy of the tables, and gets the core's messages.

## 9. Errors and messages

Track 3A adds the `FeatureError` variants that render these texts to features.md section 4.1, with their codes, for the `thread` and `hole` features. `{label}` is the size field's manifest label, `{text}` the stored text. Each passes the message lint of features.md section 3.12.

| Lookup | Message | Fix |
| --- | --- | --- |
| `find_thread_size` gives `Unknown`, ISO metric | `{label}: '{text}' is not an ISO metric thread size. Sizes look like M8x1.25.` | none |
| `find_thread_size` gives `Unknown`, unified | `{label}: '{text}' is not a unified thread size. Sizes look like 1/4-20 UNC.` | none |
| `NearMatch`, ISO metric | `{label}: '{text}' is not an ISO metric thread size. Did you mean '{designation}'?` | `SetField` of the size field to the designation |
| `NearMatch`, unified | `{label}: '{text}' is not a unified thread size. Did you mean '{designation}'?` | `SetField` of the size field to the designation |
| `find_thread_size` with `IsoMetric` gives `OtherStandard` | `{label}: '{text}' is a unified thread size, not an ISO metric one.` | `SetField` of the standard field to `unified` |
| `find_thread_size` with `Unified` gives `OtherStandard` | `{label}: '{text}' is an ISO metric thread size, not a unified one.` | `SetField` of the standard field to `iso_metric` |
| `find_clearance_hole_size` gives `Unknown` | `{label}: '{text}' is not an ISO 273 clearance hole size. Sizes look like M3.` | none |
| `find_clearance_hole_size` gives `NearMatch` | `{label}: '{text}' is not an ISO 273 clearance hole size. Did you mean '{designation}'?` | `SetField` of the size field to the designation |

With the label `Size`: `Size: 'M8' is not an ISO metric thread size. Did you mean 'M8x1.25'?` and `Size: 'M9' is not an ISO metric thread size. Sizes look like M8x1.25.` A clearance outside section 5.6's range is features.md's `OutOfRange` on the clearance field.

## 10. Acceptance tests

Tests 1 to 20 are unit tests of `partsmith_features::threads` in `crates/partsmith-features/tests/thread_tables.rs`, a module of `tests/integration.rs` ([ADR 0035](../../adr/0035-one-integration-test-binary-per-crate.md)). Computed values compare within 1e-12 mm unless a test says otherwise. Track 3A's tests of the feature start at 21. They include a modeled thread, external and internal, of the first and the last size of each series in section 3.4's order (`M1x0.25` and `M64x6`, `M1x0.2` and `M64x4`, `#1-64 UNC` and `1-1/2-6 UNC`, `#0-80 UNF` and `1-1/2-12 UNF`) at a clearance of 0.2 mm, `printer.fit.slip` on `generic-fdm` ([ADR W2-2D-1](../../adr/W2-2D-1-thread-and-clearance-hole-tables-designations-and-editions.md) decision 6).

1. **ISO metric rows.** `thread_sizes()` holds 74 rows of `IsoMetric`: 34 of `MetricCoarse`, one per row of section 3.1's table, and 40 of `MetricFine`. Their designations, in order, start `M1x0.25`, `M1x0.2`, `M1.2x0.25`, `M1.2x0.2` and end `M60x5.5`, `M60x4`, `M64x6`, `M64x4`. `M8x1.25`, `M8x1` and `M8x0.75` follow one another, and so do `M10x1.5`, `M10x1.25` and `M10x1`. Each row's `major_diameter` and `pitch` are the two numbers of its designation, and its `threads_per_inch` is `None`.
2. **Unified rows.** `thread_sizes()` holds 47 rows of `Unified` after the ISO metric ones: 23 of `Unc` and 24 of `Unf`, in section 3.4's order. The first three are `#0-80 UNF`, `#1-64 UNC` and `#1-72 UNF`, and the last two `1-1/2-6 UNC` and `1-1/2-12 UNF`. `1/4-20 UNC` has `major_diameter` 6.35, `pitch` 1.27 and `threads_per_inch` `Some(20)`. `#10-24 UNC` has 4.826 and 1.0583333333333333. `1-1/2-12 UNF` has 38.1 and 2.1166666666666667.
3. **Designations name their rows.** No two rows share a designation. For every row, `find_thread_size(row.standard, row.designation)` returns that row, and `find_thread_size` with the other standard returns `OtherStandard` with the row's standard.
4. **Basic dimensions.** For every row: `fundamental_height` is `sqrt(3) / 2 P`, `basic_minor_diameter` is `D - 5 sqrt(3) / 8 P` and `basic_pitch_diameter` is `D - 3 sqrt(3) / 8 P`. Each is within `1e-6 P` of ISO 724's and ASME B1.1's printed coefficients: `0.866025 P`, `D - 1.082532 P` and `D - 0.649519 P`. Against printed values: M8x1.25 has D1 6.647 and D2 7.188, M10x1.5 8.376 and 9.026, M6x1 4.917 and 5.350, M3x0.5 2.459 and 2.675, M12x1.75 10.106 and 10.863, M20x2.5 17.294 and 18.376, M30x2 27.835 and 28.701, each within 0.0005 mm. 1/4-20 UNC has D1 0.1959 in and D2 0.2175 in, 1/2-13 UNC 0.4167 in and 0.4500 in, and #10-24 UNC 0.1449 in and 0.1629 in, each within 0.00005 in (the millimeter values divided by 25.4).
5. **ISO 965-1's deviation.** `max_material_deviation(External)` of an ISO metric row is `(15 + 11 P) / 1000`. For every pitch of section 3.1 it is within 0.002 mm of `-es`, ISO 965-1's printed fundamental deviation of position g made positive. The largest major diameter of a 6g screw shows it (`D + es`): P 0.2 gives 0.017, 0.25 0.018 (M1, 0.982), 0.3 0.018, 0.35 0.019, 0.4 0.019, 0.45 0.020, 0.5 0.020 (M3, 2.980), 0.6 0.021, 0.7 0.022, 0.75 0.022, 0.8 0.024, 1 0.026, 1.25 0.028 (M8, 7.972), 1.5 0.032, 1.75 0.034, 2 0.038, 2.5 0.042, 3 0.048, 3.5 0.053, 4 0.060, 4.5 0.063, 5 0.071, 5.5 0.075 and 6 0.080. `max_material_deviation(Internal)` of every row, ISO metric and unified, is 0.
6. **ASME B1.1's allowance.** `max_material_deviation(External)` of a unified row, divided by 25.4, is within 0.00005 in of B1.1's printed class 2A allowance (basic major diameter minus the largest 2A major diameter): `1/4-20 UNC` 0.0011, `5/16-18 UNC` 0.0012, `3/8-16 UNC` 0.0013, `1/2-13 UNC` 0.0015, `1-8 UNC` 0.0020, `1-1/2-6 UNC` 0.0024, `#10-24 UNC` 0.0010 and `1/4-28 UNF` 0.0010. For `1-8 UNC` it is 0.002025 in within 1e-12 in (0.051435 mm).
7. **Classes and stored texts.** `class(External)` is `6g` and `class(Internal)` is `6H` for every ISO metric row, `2A` and `2B` for every unified row. `serde_json::to_string` of `ThreadStandard::IsoMetric` and `Unified` is `"iso_metric"` and `"unified"`, of `ClearanceFit::Close`, `Normal` and `Loose` `"close"`, `"normal"` and `"loose"`, and `serde_json::from_str` of each text gives its value back. `"IsoMetric"` and `"medium"` fail to deserialize.
8. **ISO 273 rows.** `clearance_hole_sizes()` holds 34 rows, `M1` to `M64` in section 3.1's order of D, each `nominal_diameter` equal to a D of section 3.1. In every row `nominal_diameter < close < normal < loose`. M3 is 3.2, 3.4 and 3.6, M8 8.4, 9 and 10, M12 13, 13.5 and 14.5, M30 31, 33 and 35, and M64 66, 70 and 74. `clearance_hole_diameter` of M3 and `Normal` is `Some(3.8)` with a clearance of 0.2, `Some(3.4)` with 0 and `Some(203.4)` with 100, and `None` with -1e-9, `f64::NAN` and `f64::INFINITY`.
9. **Exact lookups.** `find_thread_size(IsoMetric, "M8x1.25")` and `find_thread_size(Unified, "1/4-20 UNC")` return their rows. `find_thread_size(Unified, "M8x1.25")` is `OtherStandard { standard: IsoMetric }` and `find_thread_size(IsoMetric, "1/4-20 UNC")` is `OtherStandard { standard: Unified }`.
10. **ISO metric near matches.** With `IsoMetric`, each of `m8x1.25`, ` M8 x 1.25 `, `M8X1.25`, `M8×1.25`, `M8x1,25`, `M8.0x1.250` and `M8` gives `NearMatch { designation: "M8x1.25" }`, and `M10` gives `M10x1.5`. `M8x1` is exact. Each of `M8x1.5`, `M9`, `M9x1.25`, `8x1.25`, `M`, `Mx1.25`, `M8x` and the empty text gives `Unknown`.
11. **Unified near matches.** With `Unified`: `1/4-20`, `1/4-20unc` and `1/4 - 20 UNC` give `NearMatch` with `1/4-20 UNC`; `10-24 UNC` and `#10-24` with `#10-24 UNC`; `1-8` with `1-8 UNC`; `1-12` with `1-12 UNF`; `1-64` with `#1-64 UNC`; `1-1/2-6` with `1-1/2-6 UNC`. Each of `1/4-20 UNF`, `1/4-32 UNF`, `#11-24`, `1/4` and `20 UNC` gives `Unknown`.
12. **Clearance hole lookups.** `find_clearance_hole_size("M3")` and `("M3.5")` return their rows. `m3`, `M3x0.5` and `M3 x 0.5` give `NearMatch { designation: "M3" }`. `M4.5`, `M3x`, `3` and `1/4-20 UNC` give `Unknown`.
13. **M8x1.25, external.** `modeled_profile(M8x1.25, External, 0.2)` has `inner_radius` 3.0188316737325285, `depth` 0.6765823467065926, `inner_width` 1.0416666666666667, `outer_width` 0.2604166666666667, `major_diameter` 7.390828040878242, `minor_diameter` 6.037663347465057 and `pitch_diameter` 6.759351183952089. With a clearance of 0, `inner_radius` is 3.2188316737325287 and `major_diameter` 7.790828040878242.
14. **M8x1.25, internal.** `modeled_profile(M8x1.25, Internal, 0.2)` has `inner_radius` 3.6136286328542866, `depth` 0.6765823467065926, `inner_width` 0.8333333333333334, `outer_width` 0.052083333333333336, `minor_diameter` 7.227257265708573, `major_diameter` 8.58042195912176 and `pitch_diameter` 7.588101183952089. With a clearance of 0, `inner_radius` is 3.4136286328542864.
15. **The cap and an inch size.** At a clearance of 0.2: `M30x2` external has `inner_radius` 13.554630677972048 and `major_diameter` 29.27432486540519, and internal has `inner_radius` 14.261805812566857 and `major_diameter` 30.68867513459481; `1/4-20 UNC` external has `inner_radius` 2.181723807772754, `depth` 0.6874076642538982 and `pitch_diameter` 5.096682457416332, and internal has `inner_radius` 2.779246690979955 and `pitch_diameter` 5.925110802895322.
16. **Flanks and flats.** For every row and the clearances 0 and 0.2, with E the external and I the internal profile, `t = TRUNCATION x P` and δ the row's external deviation: `I.major_diameter - E.major_diameter` and `I.minor_diameter - E.minor_diameter` are both `4c + 4t + δ` within 1e-9 mm; the width along the axis of I's groove and that of E's tooth, each the linear function of the radius through its two sides (`inner_width` at `inner_radius`, `outer_width` at `inner_radius + depth`), both shrink by `2 / sqrt(3)` per millimeter of radius, and at any radius the groove's is the larger by `(2c + δ / 2) 2 / sqrt(3)` within 1e-9 mm (for M8x1.25 at 0.2: 0.47847903559090244); and `P - E.inner_width`, `E.outer_width`, `P - I.inner_width` and `I.outer_width` are `P/6`, `5P/24`, `P/3` and `P/24` within 1e-12 P. `TRUNCATION` is `3.0_f64.sqrt() / 24.0`.
17. **The kernel's limits.** For every row, both kinds and the clearances 0, 0.2 and 0.4 (where `max_clearance` allows it): `depth`, `outer_width`, `inner_width - outer_width` and `P - inner_width` are each at least 1e-6 (kernel.md section 3.4.14 item 1.3).
18. **The clearance's range.** `modeled_profile` of M8x1.25 is `None` for the clearances -1e-9, `f64::NAN` and `f64::INFINITY`, of either kind. `max_clearance(M8x1.25, External)` is `Some(3.2188306737325285)`; at that clearance the external profile exists and its `inner_radius` is 1e-6 within 1e-12, and at 3.21884 it is `None`. `max_clearance(M8x1.25, Internal)` is `None`, and `modeled_profile(M8x1.25, Internal, 100.0)` exists.
19. **The cosmetic bore.** `cosmetic_bore_diameter` is 6.646835306586815 for M8x1.25 and 4.975184671492204 for 1/4-20 UNC: each size's `basic_minor_diameter`.
20. **The M2 edition is pinned.** The text with one line per row of `thread_sizes()`, `{designation} {standard} {series} {major_diameter} {pitch}`, then one line per row of `clearance_hole_sizes()`, `{designation} {nominal_diameter} {close} {normal} {loose}`, each line ending in `\n`, equals the committed file `crates/partsmith-features/tests/data/thread-tables-m2.txt` byte for byte. `{standard}` and `{series}` are the `Debug` names (`IsoMetric`, `Unified`, `MetricCoarse`, `MetricFine`, `Unc`, `Unf`). Each number is Rust's `Display` text of the `f64` that section 3's arithmetic gives: the shortest decimal that reads back as that value, with no point for a whole number. The test author computes each value from section 3's tables, never from the code, in the same `f64` arithmetic (D in inches times 25.4, and 25.4 divided by the threads per inch). Among the file's lines are `M8x1.25 IsoMetric MetricCoarse 8 1.25`, `#1-64 UNC Unified Unc 1.8541999999999998 0.396875`, `1-1/2-6 UNC Unified Unc 38.099999999999994 4.233333333333333` and `M3 3 3.2 3.4 3.6`. A change to a row then fails this test until the file changes with it, in the same pull request and under section 3.5's rule.

## Open issues

1. **Engagement left by two clearances.** The clearance moves every diameter. It takes radial engagement: for M8x1.25 at `generic-fdm`'s slip of 0.2 mm, a standard 6g screw reaches 0.372 mm into a modeled internal thread (of the basic 0.677 mm), and two modeled M8x1.25 threads reach 0.082 mm into each other. For M3x0.5, a standard screw reaches 0.025 mm into a modeled internal thread. The truncation takes `H/12` of each crest (section 5.4) and the clearance `c` of each side. Small threads on FDM and printed pairs then hold little or nothing at the start value. Track 3A's choice "small threads on FDM" settles the message side (no feature warning in M2, the print checks report small details). The physical prints of gate 20 (the threaded cap and the tapped block, FDM and resin) judge the start value. Options for a change by a later ADR: half of `printer.fit.slip` on each side of a printed pair, a smaller start for internal threads, or a different fraction for the crest. Owner: the M2 gate's print record.
2. **Provenance and the current editions.** Section 3's rows are written from the editions named in section 3.4. ISO has since published ISO 68-1:2023, ISO 262:2023 and ISO 724:2023, and ISO 20273:2017 replaced ISO 273:1979. Before the first merge of code that builds a stored size (section 3.5), each row and each printed value of tests 4 to 6 is checked against the current edition of ISO 68-1, ISO 261, ISO 262, ISO 724, ISO 965-1, ASME B1.1 and ISO 20273, and the check names the edition it used for each. A row that differs is corrected in this spec before that merge, while the edition is not yet fixed. A size that a current edition adds, beyond ISO 262:1998's selection, may come later as a new row (section 3.5). Owner: track 3A's code pull request of item 20 or 21, whichever merges first. It records the editions and the result, and the human review of choice 16's sourcing at the M2 gate reads that record.
3. **A size a later release adds, on an older build.** Once the edition is fixed, a new row may come in any release (section 3.5). An older build that opens a document holding that size reports section 9's unknown-size message. The message does not say to update. file-format.md section 3.9 gives a new format version to the changes whose message on an older build does not say to update. Options: keep it, since nothing is misread and the message names the size; word section 9's unknown-size message for a text that has the form of a designation of its standard to say that a newer version may have the size; or let new rows wait for a format version. Owner: track 3A, with section 9's `FeatureError` variants.
