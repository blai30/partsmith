//! Exit statuses (cli.md section 2.1).

use std::process::ExitCode;

/// How a run, or one file of it, ended. The order is the exit code order, so the status of a
/// run over several files is the largest status of any file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    /// Exit 0: success.
    Success,
    /// Exit 1: the model has problems, or a schema file is out of date.
    Problems,
    /// Exit 2: a usage error.
    Usage,
    /// Exit 3: a file or directory cannot be read or written.
    Io,
    /// Exit 4: an internal error.
    Internal,
}

impl Status {
    /// The process exit code.
    pub fn code(self) -> u8 {
        match self {
            Status::Success => 0,
            Status::Problems => 1,
            Status::Usage => 2,
            Status::Io => 3,
            Status::Internal => 4,
        }
    }
}

impl From<Status> for ExitCode {
    fn from(status: Status) -> ExitCode {
        ExitCode::from(status.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_follow_the_spec_table() {
        let codes = [
            Status::Success,
            Status::Problems,
            Status::Usage,
            Status::Io,
            Status::Internal,
        ]
        .map(Status::code);

        assert_eq!(codes, [0, 1, 2, 3, 4]);
    }

    #[test]
    fn the_largest_status_wins() {
        let statuses = [Status::Success, Status::Io, Status::Problems];

        assert_eq!(statuses.into_iter().max(), Some(Status::Io));
    }
}
