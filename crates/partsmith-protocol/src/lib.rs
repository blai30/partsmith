//! The message protocol between the Partsmith UI (and every other host) and the core.
//!
//! A client sends a [`Request`] holding a [`Command`]; the engine answers with the [`Event`]s the
//! request caused followed by one [`Response`] holding a [`CommandResult`] or an [`ErrorInfo`].
//! Messages travel in a binary [`Frame`]: the magic `PSM1`, a little-endian header length, a
//! compact JSON header ([`FrameHeader`]), zero padding to 8 bytes, then raw little-endian
//! [`Buffer`]s that messages reference by [`BufferIndex`]. [`encode_frame`] and [`decode_frame`]
//! implement the byte layout; the decoder is strict and reports a [`FrameError`] with the
//! [`ErrorCode`] a host answers with.
//!
//! Core data (documents, edits, sketch params, references, printer profiles and reports) travels
//! as the core crates' own types, so the generated [`protocol_schema`] and the UI's TypeScript
//! types carry them typed. Features' fixes and recompute's loss reasons, which their crates do not
//! serialize, have wire forms of their own ([`FixAction`], [`LossReason`]) that the engine projects
//! into. The behavior is specified in `docs/spec/protocol.md`.

mod command;
mod diagnostic;
mod error;
mod event;
mod frame;
mod ghost;
mod loss;
mod message;
mod schema;
mod shared;
mod solution;
mod strict;

pub use partsmith_mesh::{BoundingBox, TessellationTolerance};
pub use partsmith_naming::ElementKind;
pub use partsmith_print::Severity;
pub use partsmith_sketch::{EntityState, Placement, SketchStatus};

pub use command::{
    CatalogEntry, Command, CommandResult, ExportFormat, MeasureTarget, Measurement, Resolution,
    ResolutionStatus,
};
pub use diagnostic::{Cause, Diagnostic, FixAction, FixKind, FixRef};
pub use error::{ErrorCode, ErrorInfo, Span};
pub use event::{
    BodyPrint, Event, FeatureState, FeatureStatus, MeshBuffers, ParameterValue, PrintReport,
    RegionCandidate, Repair, RepairCandidate,
};
pub use frame::{
    Buffer, BufferIndex, BufferInfo, ElementType, Frame, FrameError, FrameHeader, ResponseFrame,
    decode_frame, decode_request_frame, decode_response_frame, encode_frame, encode_request_frame,
    encode_response_frame, error_response_frame,
};
pub use ghost::{Ghost, GhostFrame, GhostKind};
pub use loss::{Absence, InstanceIndex, LossReason, Removal};
pub use message::{Message, Outcome, Request, Response};
pub use schema::protocol_schema;
pub use shared::{
    AssetData, Axis, CurveKind, DocHandle, ElementDescriptor, ElementRef, ExportFile, Layer,
    MeshElement, Orientation, Pick, SurfaceKind, TessellationQuality, Vec2, Vec3,
};
pub use solution::{
    ConstraintSolveState, ConstraintState, RegionInfo, SketchDisplay, SketchSolution, SolvedEntity,
    SolvedPoint,
};

/// Current protocol version; bumped on every breaking change.
pub const PROTOCOL_VERSION: u32 = 1;

/// The four bytes every frame starts with.
pub const MAGIC: [u8; 4] = *b"PSM1";

/// Largest accepted JSON header, in bytes (64 MiB).
pub const MAX_HEADER_LENGTH: usize = 67_108_864;

/// Largest integer a JavaScript number holds exactly; request ids and mesh ids stay at or below it.
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
