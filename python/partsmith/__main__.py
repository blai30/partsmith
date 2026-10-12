"""`python -m partsmith`: build a document from a script (python-api.md section 2.10).

Exit codes follow cli.md: 0 success, 1 the script raised or did not create exactly one document,
2 usage error, 3 I/O error.
"""

import argparse
import sys
import traceback
from pathlib import Path

from . import _core
from .running import run_code

SUCCESS = 0
"""The document was built and saved."""

SCRIPT_FAILED = 1
"""The script raised, or did not create exactly one document."""

IO_ERROR = 3
"""The script could not be read, or the document could not be written."""

VERSION = f"partsmith {_core.ENGINE_VERSION} (protocol {_core.PROTOCOL_VERSION}, format {_core.FORMAT_VERSION})"
"""The text of `--version`."""


def main(arguments: list[str] | None = None) -> int:
    """Parses the command line and runs the command; returns the exit code."""
    parser = argparse.ArgumentParser(
        prog="python -m partsmith", description="Build Partsmith documents from Python scripts."
    )
    parser.add_argument("--version", action="version", version=VERSION)
    commands = parser.add_subparsers(dest="command", required=True)
    build_parser = commands.add_parser("build", help="run a script and save the one document it creates")
    build_parser.add_argument("script", type=Path, help="the Python script")
    build_parser.add_argument(
        "-o", "--output", type=Path, help="the .ptsm file to write (default: the script with .ptsm)"
    )
    options = parser.parse_args(arguments)
    return build(options.script, options.output)


def build(script: Path, output: Path | None) -> int:
    """Runs `script` and saves the one document it creates to `output`."""
    try:
        code = script.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        print(f"error: Cannot read {script}: {error}", file=sys.stderr)
        return IO_ERROR
    try:
        documents = run_code(code, filename=str(script))
    except Exception:
        traceback.print_exc()
        return SCRIPT_FAILED
    if len(documents) != 1:
        print(f"error: The script created {len(documents)} documents; build needs exactly one.", file=sys.stderr)
        return SCRIPT_FAILED
    target = script.with_suffix(".ptsm") if output is None else output
    try:
        documents[0].save(target)
    except OSError as error:
        print(f"error: Cannot write {target}: {error}", file=sys.stderr)
        return IO_ERROR
    return SUCCESS


if __name__ == "__main__":
    sys.exit(main())
