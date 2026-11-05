#!/usr/bin/env bash
# Checks that <archive> is a whole gzip file of the tar that git archive wrote from <commit> (delivery.md section 2.13,
# The archive check). It prints nothing: it exits 0 when the archive passes and 1 when it does not, a missing file
# included. The workflow steps that run it print their own error.
#
# Usage: scripts/check-source-archive.sh <archive> <commit>
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: scripts/check-source-archive.sh <archive> <commit>" >&2
  exit 2
fi
archive="$1"
commit="$2"

# gzip -t reads all of the file: a cut-off archive still names the commit in its first header.
gzip -t "$archive" 2>/dev/null || exit 1
# git get-tar-commit-id exits after that header, and the gzip that feeds it may then report a broken pipe where SIGPIPE
# is ignored. Only what git prints counts.
[ "$(gzip -dc "$archive" 2>/dev/null | git get-tar-commit-id 2>/dev/null)" = "$commit" ]
