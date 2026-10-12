//! The command line grammar of cli.md section 2, parsed with `clap`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::{ArgGroup, Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use partsmith_engine::EngineInfo;

/// The parsed command line.
#[derive(Debug, Parser)]
#[command(
    name = "partsmith",
    about = "Validate, format, rebuild and export Partsmith models.",
    subcommand_required = true,
    arg_required_else_help = true
)]
pub struct Arguments {
    /// The command to run.
    #[command(subcommand)]
    pub command: Command,
}

/// One `partsmith` command.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check documents: loading, expressions, references, sketches and assets.
    Validate(ValidateArguments),
    /// Rewrite documents in canonical form, or check that they are.
    Fmt(FmtArguments),
    /// Rebuild a document and report feature status and body metrics.
    Rebuild(RebuildArguments),
    /// Export the bodies of a document as STL, 3MF, OBJ, STEP or Python.
    Export(ExportArguments),
    /// Print, write or check the JSON Schemas of documents and the protocol.
    Schema(SchemaArguments),
}

/// Arguments of `partsmith validate`.
#[derive(Debug, Args)]
pub struct ValidateArguments {
    /// Also rebuild geometry and report feature errors and warnings.
    #[arg(long)]
    pub rebuild: bool,
    /// Report files that are not in canonical form.
    #[arg(long)]
    pub canonical: bool,
    /// Treat warnings as failures for the exit code.
    #[arg(long)]
    pub deny_warnings: bool,
    /// Print one JSON document instead of text.
    #[arg(long)]
    pub json: bool,
    /// Documents to check; `-` reads standard input.
    #[arg(value_name = "FILE", required = true)]
    pub files: Vec<PathBuf>,
}

/// Arguments of `partsmith fmt`.
#[derive(Debug, Args)]
pub struct FmtArguments {
    /// Write nothing; fail when a file would change.
    #[arg(long)]
    pub check: bool,
    /// Documents to format; `-` formats standard input to standard output.
    #[arg(value_name = "FILE", required = true)]
    pub files: Vec<PathBuf>,
}

/// Arguments of `partsmith rebuild`.
#[derive(Debug, Args)]
pub struct RebuildArguments {
    /// Also run the print checks and report their findings.
    #[arg(long)]
    pub checks: bool,
    /// Use this printer profile instead of the document's (not saved).
    #[arg(long, value_name = "ID")]
    pub printer: Option<String>,
    /// Rotate about the model axis x, y or z by a number of degrees, for example x:90. Repeatable.
    #[arg(long, value_name = "AXIS:DEGREES")]
    pub rotate: Vec<String>,
    /// Treat warnings as failures for the exit code.
    #[arg(long)]
    pub deny_warnings: bool,
    /// Print one JSON document instead of text.
    #[arg(long)]
    pub json: bool,
    /// The document to rebuild.
    #[arg(value_name = "FILE")]
    pub file: PathBuf,
}

/// Arguments of `partsmith export`.
#[derive(Debug, Args)]
pub struct ExportArguments {
    /// The document to export.
    #[arg(value_name = "FILE")]
    pub file: PathBuf,
    /// Output file, or output directory with --per-body.
    #[arg(short, long, value_name = "PATH")]
    pub output: PathBuf,
    /// File format; by default it comes from the output extension.
    #[arg(long, value_enum, value_name = "FORMAT")]
    pub format: Option<Format>,
    /// Export only this body, by id or name. Repeatable.
    #[arg(long, value_name = "BODY")]
    pub body: Vec<String>,
    /// Write one file per body (STL only).
    #[arg(long)]
    pub per_body: bool,
    /// Write ASCII STL instead of binary (STL only).
    #[arg(long)]
    pub ascii: bool,
    /// Chord deviation in mm (mesh formats only).
    // Negative values parse, so the range check names the rule instead of clap guessing a flag.
    #[arg(long, value_name = "MM", allow_negative_numbers = true)]
    pub chord: Option<f64>,
    /// Angular deviation in degrees, at most 90 (mesh formats only).
    #[arg(long, value_name = "DEGREES", allow_negative_numbers = true)]
    pub angle: Option<f64>,
    /// Use this printer profile instead of the document's (not saved).
    #[arg(long, value_name = "ID")]
    pub printer: Option<String>,
    /// Rotate about the model axis x, y or z by a number of degrees, for example x:90. Repeatable.
    #[arg(long, value_name = "AXIS:DEGREES")]
    pub rotate: Vec<String>,
    /// Export even when features failed.
    #[arg(long)]
    pub allow_errors: bool,
    /// Print one JSON document instead of text.
    #[arg(long)]
    pub json: bool,
}

/// Arguments of `partsmith schema`: exactly one of a schema name, `--write` or `--check`.
#[derive(Debug, Args)]
#[command(group = ArgGroup::new("action").required(true).args(["name", "write", "check"]))]
pub struct SchemaArguments {
    /// Print this schema to standard output.
    #[arg(value_enum, value_name = "NAME")]
    pub name: Option<SchemaName>,
    /// Write both schema files.
    #[arg(long)]
    pub write: bool,
    /// Check that both schema files are up to date.
    #[arg(long)]
    pub check: bool,
    /// Directory for --write and --check.
    #[arg(long, value_name = "DIR", default_value = "docs/format")]
    pub out_dir: PathBuf,
}

/// An export format as `--format` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// STL, binary unless --ascii.
    Stl,
    /// 3MF.
    #[value(name = "3mf")]
    ThreeMf,
    /// Wavefront OBJ.
    Obj,
    /// STEP AP214.
    Step,
    /// A Python script.
    #[value(name = "py")]
    Python,
}

impl Format {
    /// The name `--format` takes, which messages use too.
    pub fn name(self) -> &'static str {
        match self {
            Format::Stl => "stl",
            Format::ThreeMf => "3mf",
            Format::Obj => "obj",
            Format::Step => "step",
            Format::Python => "py",
        }
    }

    /// The format of an output path's extension, ignoring case: `.stl`, `.3mf`, `.obj`, `.step`
    /// or `.stp`, and `.py`.
    pub fn from_path(path: &Path) -> Option<Format> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "stl" => Some(Format::Stl),
            "3mf" => Some(Format::ThreeMf),
            "obj" => Some(Format::Obj),
            "step" | "stp" => Some(Format::Step),
            "py" => Some(Format::Python),
            _ => None,
        }
    }

    /// Whether the format writes triangle meshes, so tessellation options apply.
    pub fn is_mesh(self) -> bool {
        matches!(self, Format::Stl | Format::ThreeMf | Format::Obj)
    }
}

/// A schema `partsmith schema <NAME>` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SchemaName {
    /// The `.ptsm` document schema.
    Ptsm,
    /// The protocol schema.
    Protocol,
}

/// What `partsmith schema` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaAction {
    /// Print one schema.
    Print(SchemaName),
    /// Write both schema files.
    Write,
    /// Check both schema files.
    Check,
}

impl SchemaArguments {
    /// The one action the arguments ask for; clap's `action` group makes it exactly one.
    pub fn action(&self) -> SchemaAction {
        match (self.name, self.write) {
            (Some(name), _) => SchemaAction::Print(name),
            (None, true) => SchemaAction::Write,
            (None, false) => SchemaAction::Check,
        }
    }
}

/// The text `--version` prints after the binary name: the engine version, then the protocol
/// version, the format version and the kernel.
pub fn version_text(info: &EngineInfo) -> String {
    format!(
        "{} (protocol {}, format {}, {})",
        info.engine_version, info.protocol_version, info.format_version, info.kernel
    )
}

/// Parses a command line (the binary name first) with the version text of `info`.
pub fn parse_from<I, T>(arguments: I, info: &EngineInfo) -> Result<Arguments, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = Arguments::command()
        .version(version_text(info))
        .try_get_matches_from(arguments)?;
    Arguments::from_arg_matches(&matches)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> EngineInfo {
        EngineInfo {
            protocol_version: 1,
            engine_version: "0.1.0".to_owned(),
            format_version: 1,
            kernel: "OCCT 7.9.3".to_owned(),
        }
    }

    fn parse(arguments: &[&str]) -> Result<Arguments, clap::Error> {
        parse_from(
            std::iter::once("partsmith").chain(arguments.iter().copied()),
            &info(),
        )
    }

    fn schema(arguments: &[&str]) -> SchemaArguments {
        match parse(arguments).expect("the arguments parse").command {
            Command::Schema(schema) => schema,
            other => panic!("not a schema command: {other:?}"),
        }
    }

    #[test]
    fn the_definition_is_consistent() {
        Arguments::command().debug_assert();
    }

    #[test]
    fn version_text_names_engine_protocol_format_and_kernel() {
        assert_eq!(
            version_text(&info()),
            "0.1.0 (protocol 1, format 1, OCCT 7.9.3)"
        );
    }

    #[test]
    fn version_flag_prints_the_version_text() {
        let error = parse(&["--version"]).expect_err("--version stops parsing");

        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayVersion);
        assert_eq!(
            error.to_string(),
            "partsmith 0.1.0 (protocol 1, format 1, OCCT 7.9.3)\n"
        );
    }

    #[test]
    fn no_arguments_is_a_usage_error() {
        let error = parse(&[]).expect_err("a command is required");

        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn validate_needs_a_file() {
        assert!(parse(&["validate"]).is_err());
    }

    #[test]
    fn validate_reads_flags_and_files() {
        let Command::Validate(validate) = parse(&["validate", "--json", "a.ptsm", "-"])
            .expect("the arguments parse")
            .command
        else {
            panic!("not a validate command");
        };

        assert!(validate.json);
        assert!(!validate.rebuild);
        assert_eq!(
            validate.files,
            [PathBuf::from("a.ptsm"), PathBuf::from("-")]
        );
    }

    #[test]
    fn export_needs_an_output() {
        assert!(parse(&["export", "box.ptsm"]).is_err());
    }

    #[test]
    fn export_reads_repeated_bodies_and_rotations() {
        let Command::Export(export) = parse(&[
            "export", "box.ptsm", "-o", "out.stl", "--body", "a", "--body", "b", "--rotate",
            "x:90", "--rotate", "z:45",
        ])
        .expect("the arguments parse")
        .command
        else {
            panic!("not an export command");
        };

        assert_eq!(export.output, PathBuf::from("out.stl"));
        assert_eq!(export.body, ["a", "b"]);
        assert_eq!(export.rotate, ["x:90", "z:45"]);
    }

    #[test]
    fn export_format_names_include_3mf_and_py() {
        let format = |name: &str| match parse(&["export", "a.ptsm", "-o", "x", "--format", name])
            .expect("the arguments parse")
            .command
        {
            Command::Export(export) => export.format,
            other => panic!("not an export command: {other:?}"),
        };

        assert_eq!(format("3mf"), Some(Format::ThreeMf));
        assert_eq!(format("py"), Some(Format::Python));
    }

    #[test]
    fn export_reads_negative_tolerances_for_the_range_check() {
        let Command::Export(export) = parse(&[
            "export", "a.ptsm", "-o", "x.stl", "--chord", "-1", "--angle", "-5",
        ])
        .expect("the arguments parse")
        .command
        else {
            panic!("not an export command");
        };

        assert_eq!(export.chord, Some(-1.0));
        assert_eq!(export.angle, Some(-5.0));
    }

    #[test]
    fn export_rejects_a_chord_that_is_not_a_number() {
        let error = parse(&["export", "a.ptsm", "-o", "x.stl", "--chord", "fine"])
            .expect_err("the chord is a number");

        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn formats_come_from_extensions_ignoring_case() {
        assert_eq!(Format::from_path(Path::new("a.STL")), Some(Format::Stl));
        assert_eq!(Format::from_path(Path::new("a.3mf")), Some(Format::ThreeMf));
        assert_eq!(Format::from_path(Path::new("a.obj")), Some(Format::Obj));
        assert_eq!(Format::from_path(Path::new("a.step")), Some(Format::Step));
        assert_eq!(Format::from_path(Path::new("a.Stp")), Some(Format::Step));
        assert_eq!(
            Format::from_path(Path::new("dir/a.py")),
            Some(Format::Python)
        );
    }

    #[test]
    fn unknown_or_missing_extensions_give_no_format() {
        assert_eq!(Format::from_path(Path::new("out.dat")), None);
        assert_eq!(Format::from_path(Path::new("parts")), None);
    }

    #[test]
    fn only_stl_3mf_and_obj_are_mesh_formats() {
        let mesh: Vec<Format> = Format::value_variants()
            .iter()
            .copied()
            .filter(|format| format.is_mesh())
            .collect();

        assert_eq!(mesh, [Format::Stl, Format::ThreeMf, Format::Obj]);
    }

    #[test]
    fn format_names_match_their_values() {
        for format in Format::value_variants() {
            let value = format
                .to_possible_value()
                .expect("every format has a value");

            assert_eq!(value.get_name(), format.name());
        }
    }

    #[test]
    fn schema_actions() {
        assert_eq!(
            schema(&["schema", "ptsm"]).action(),
            SchemaAction::Print(SchemaName::Ptsm)
        );
        assert_eq!(schema(&["schema", "--write"]).action(), SchemaAction::Write);
        assert_eq!(schema(&["schema", "--check"]).action(), SchemaAction::Check);
    }

    #[test]
    fn schema_out_dir_defaults_to_docs_format() {
        assert_eq!(
            schema(&["schema", "--check"]).out_dir,
            PathBuf::from("docs/format")
        );
    }

    #[test]
    fn schema_needs_exactly_one_action() {
        for arguments in [
            &["schema"][..],
            &["schema", "ptsm", "--write"],
            &["schema", "--write", "--check"],
        ] {
            let error = parse(arguments).expect_err("the action is ambiguous or missing");

            assert_eq!(error.exit_code(), 2, "{arguments:?}");
        }
    }

    #[test]
    fn a_schema_name_takes_an_out_dir_it_does_not_use() {
        let printed = schema(&["schema", "protocol", "--out-dir", "s"]);

        assert_eq!(printed.action(), SchemaAction::Print(SchemaName::Protocol));
    }
}
