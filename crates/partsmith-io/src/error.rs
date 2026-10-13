//! The crate's error type (spec sections 2.5 and 4).

use partsmith_occt_sys::KernelError;

/// Every failure of this crate. `Display` is the user-facing message.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum IoError {
    /// The body slice is empty.
    #[error("There is nothing to export.")]
    NoBodies,
    /// A written body has no triangle (after welding, for 3MF and OBJ).
    #[error("Body \"{name}\" has no triangles to export.")]
    EmptyBody {
        /// Index of the body in the slice passed to the writer.
        body: usize,
        /// The body's display name.
        name: String,
    },
    /// A written body uses a vertex with a coordinate outside the `f32` range.
    #[error(
        "Body \"{name}\" has a point too far from the origin for a mesh file (more than 3.4e38 mm)."
    )]
    CoordinateOutOfRange {
        /// Index of the body in the slice passed to the writer.
        body: usize,
        /// The body's display name.
        name: String,
    },
    /// A binary STL would hold more than `u32::MAX` triangles.
    #[error(
        "The export has {count} triangles, more than the 4294967295 a binary STL file can hold. \
         Export one file per body or use 3MF."
    )]
    TooManyTriangles {
        /// The total triangle count of all bodies.
        count: u64,
    },
    /// No body passed to `write_3mf` has the role `part`.
    #[error(
        "None of the bodies is a printed part, and 3MF export writes only printed parts in this \
         version. Export modifier bodies as STL instead."
    )]
    NoPrintedParts,
    /// The 3MF model part would exceed `u32::MAX` bytes before compression.
    #[error(
        "The 3MF model is larger than 4 GiB, which this version cannot write. Export fewer bodies \
         or use a coarser tolerance."
    )]
    PackageTooLarge,
    /// An output buffer could not grow: the file is larger than this build can hold in memory
    /// (any buffer above `isize::MAX` bytes, 2 GiB, on WebAssembly).
    #[error(
        "The export is too large to hold in memory. Export fewer bodies per file, use a coarser \
         tolerance, or use binary STL or 3MF."
    )]
    OutputTooLarge,
    /// The zip writer failed for another reason (an internal error).
    #[error("The 3MF package could not be written ({detail}). Please report this.")]
    Package {
        /// The zip writer's error text.
        detail: String,
    },
    /// The kernel failed to write or read STEP; displays the kernel's user message.
    #[error(transparent)]
    Kernel(#[from] KernelError),
}

#[cfg(test)]
mod tests {
    use partsmith_occt_sys::{KernelErrorKind, Operation};

    use super::*;

    #[test]
    fn empty_body_message_names_the_body() {
        let error = IoError::EmptyBody {
            body: 2,
            name: "Lid".to_owned(),
        };
        assert_eq!(
            error.to_string(),
            "Body \"Lid\" has no triangles to export."
        );
    }

    #[test]
    fn too_many_triangles_message_gives_the_count() {
        let error = IoError::TooManyTriangles {
            count: 5_000_000_000,
        };
        assert_eq!(
            error.to_string(),
            "The export has 5000000000 triangles, more than the 4294967295 a binary STL file can \
             hold. Export one file per body or use 3MF."
        );
    }

    #[test]
    fn package_message_wraps_the_detail() {
        let error = IoError::Package {
            detail: "i/o error: disk".to_owned(),
        };
        assert_eq!(
            error.to_string(),
            "The 3MF package could not be written (i/o error: disk). Please report this."
        );
    }

    #[test]
    fn kernel_error_displays_the_kernel_user_message() {
        let kernel = KernelError {
            operation: Operation::ReadStep,
            kind: KernelErrorKind::ReadFailed,
            exception: String::new(),
            message: "unparsable".to_owned(),
        };
        let error = IoError::from(kernel.clone());
        assert_eq!(error.to_string(), "The file is not a valid STEP file.");
        assert_eq!(error, IoError::Kernel(kernel));
    }
}
