#!/usr/bin/env bash
# Writes the OCCT source that scripts/build-occt.sh reads, from the tag scripts/occt-version.sh pins, as
# <directory>/occt-<version>-source.tar.gz: the same bytes on every run, whatever the Git configuration
# (delivery.md section 2.13, ADR 0034). The release workflow attaches it to every release.
#
# Usage: scripts/occt-source-archive.sh <directory>
#
# It reads the OCCT clone at $OCCT_WORK_DIR/src, as build-occt.sh does, and clones the tag there first when that
# folder does not exist.
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: scripts/occt-source-archive.sh <directory>" >&2
  exit 2
fi
directory="$1"

source "$(dirname "$0")/occt-version.sh"

work="${OCCT_WORK_DIR:-${TMPDIR:-/tmp}/occt-${OCCT_VERSION}}"
src="$work/src"
archive="$directory/occt-${OCCT_VERSION}-source.tar.gz"

cloned=false
if [ ! -d "$src" ]; then
  git clone --depth 1 --branch "$OCCT_TAG" https://github.com/Open-Cascade-SAS/OCCT.git "$src"
  cloned=true
fi

# Any failure here, a folder that is not a Git clone included, means the clone holds no such tag.
if ! commit="$(git -C "$src" rev-parse --verify --quiet "$OCCT_TAG^{commit}" 2>/dev/null)"; then
  echo "$src holds no tag $OCCT_TAG. Point OCCT_WORK_DIR at a folder whose src is a clone that scripts/build-occt.sh made, or at one without src, where the script clones the tag (docs/spec/delivery.md section 2.13)." >&2
  exit 1
fi
if [ "$commit" != "$OCCT_COMMIT" ]; then
  echo "The tag $OCCT_TAG in $src names the commit $commit, not $OCCT_COMMIT, which scripts/occt-version.sh pins. Upstream may have moved the tag; nothing was built or archived from it (docs/spec/delivery.md section 2.13)." >&2
  # A clone this run made goes, as in build-occt.sh, so that neither script later builds or archives from it.
  if [ "$cloned" = true ]; then
    rm -rf "$src"
  fi
  exit 1
fi

# The files CMake reads with the options of build-occt.sh, the license files it installs and upstream's README.
paths=(.clang-format CMakeLists.txt LICENSE_LGPL_21.txt OCCT_LGPL_EXCEPTION.txt README.md adm src)

# A failure leaves no partial archive behind: the temporary file is removed on any exit before the rename.
temporary="$archive.$$.partial"
trap 'rm -f -- "$temporary"' EXIT
# The -c options keep the configuration from changing the bytes: the line endings of files without an eol
# attribute, the file modes, and attributes such as export-ignore from a global attributes file. git archive of a
# tag gives every entry the commit's time, and gzip -n leaves the name and time out of its header.
git -C "$src" -c core.autocrlf=false -c tar.umask=0002 -c core.attributesFile=/dev/null \
  archive --format=tar --prefix="occt-${OCCT_VERSION}/" "$OCCT_TAG" -- "${paths[@]}" |
  gzip -9 -n >"$temporary"
mv -- "$temporary" "$archive"

echo "Wrote $archive from $OCCT_TAG, commit $commit."
