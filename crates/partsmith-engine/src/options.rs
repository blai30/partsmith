//! Options fixed when a session is created (engine.md section 2).

use std::fmt;

use partsmith_model::{IdGenerator, RandomIdGenerator, SequentialIds, UndoLog};
use partsmith_protocol::TessellationQuality;

/// Default soft upper bound for cached results: 512 MiB on wasm32, whose address space is 4 GiB.
#[cfg(target_arch = "wasm32")]
pub const DEFAULT_CACHE_BUDGET: usize = 512 << 20;

/// Default soft upper bound for cached results: 1 GiB.
#[cfg(not(target_arch = "wasm32"))]
pub const DEFAULT_CACHE_BUDGET: usize = 1 << 30;

/// Options fixed when a session is created.
pub struct SessionOptions {
    /// Produce display meshes, sketch display buffers and overhang maps.
    pub display: bool,
    /// Generates ids for records the engine creates (STEP import).
    pub id_generator: Box<dyn IdGenerator + Send>,
    /// Soft upper bound for cached recompute and tessellation results, in bytes, split in halves
    /// between the two caches (engine.md section 3.17).
    pub cache_budget: usize,
    /// Committed transactions kept per document for undo.
    pub undo_limit: usize,
    /// Display tessellation quality for new and opened documents.
    pub tessellation: TessellationQuality,
}

impl Default for SessionOptions {
    /// Display on, random ids, 1 GiB cache (512 MiB on wasm32), 1000 undo entries, standard quality.
    fn default() -> Self {
        SessionOptions {
            display: true,
            id_generator: Box::new(RandomIdGenerator::new()),
            cache_budget: DEFAULT_CACHE_BUDGET,
            undo_limit: UndoLog::DEFAULT_MAX_DEPTH,
            tessellation: TessellationQuality::Standard,
        }
    }
}

impl SessionOptions {
    /// Display off and `SequentialIds`; used by the CLI, Python and tests.
    pub fn headless() -> Self {
        SessionOptions {
            display: false,
            id_generator: Box::new(SequentialIds::new()),
            ..SessionOptions::default()
        }
    }
}

impl fmt::Debug for SessionOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionOptions")
            .field("display", &self.display)
            .field("cache_budget", &self.cache_budget)
            .field("undo_limit", &self.undo_limit)
            .field("tessellation", &self.tessellation)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::{Id, IdKind};

    use super::*;

    #[test]
    fn defaults_show_meshes_at_standard_quality() {
        let options = SessionOptions::default();

        assert!(options.display);
        assert_eq!(options.cache_budget, DEFAULT_CACHE_BUDGET);
        assert_eq!(options.undo_limit, 1000);
        assert_eq!(options.tessellation, TessellationQuality::Standard);
    }

    #[test]
    fn headless_sessions_hide_meshes_and_count_ids() {
        let mut options = SessionOptions::headless();

        let first = options
            .id_generator
            .generate(IdKind::Body, &|_: &Id| false)
            .expect("an id is free");

        assert!(!options.display);
        assert_eq!(first.as_str(), "b-000000000001");
    }

    #[test]
    fn debug_output_leaves_out_the_generator() {
        let text = format!("{:?}", SessionOptions::headless());

        assert!(text.contains("display: false"), "{text}");
    }
}
