#!/usr/bin/env bash
# Writes Partsmith's own source, the commit checked out in this clone, as <directory>/partsmith-source.tar.gz: the
# same bytes on every run, whatever the Git configuration (delivery.md section 2.13, ADR 0034). The sources jobs of the
# workflows write it into every source artifact, and from there into every web deployment and every release.
#
# Usage: scripts/partsmith-source-archive.sh <directory>
#
# It reads only the commit, never the work tree: uncommitted changes and untracked files stay out.
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: scripts/partsmith-source-archive.sh <directory>" >&2
  exit 2
fi
directory="$1"

# An empty CDPATH keeps cd from searching it and from printing the folder it finds, which would end up in root.
root="$(CDPATH='' cd -P -- "$(dirname -- "$0")/.." >/dev/null && pwd -P)"
archive="$directory/partsmith-source.tar.gz"

# A tree without .git, or a folder inside another clone, has no HEAD of Partsmith's own.
if [ "$(git -C "$root" rev-parse --show-toplevel 2>/dev/null)" != "$root" ]; then
  echo "$root is not the top folder of a Git clone. Run scripts/partsmith-source-archive.sh in a clone of the Partsmith repository, whose checked-out commit it archives (docs/spec/delivery.md section 2.13)." >&2
  exit 1
fi
commit="$(git -C "$root" rev-parse --verify 'HEAD^{commit}')"

# A failure leaves no partial archive behind: the temporary file is removed on any exit before the rename.
temporary="$archive.$$.partial"
trap 'rm -f -- "$temporary"' EXIT
# The -c options and GIT_ATTR_NOSYSTEM keep the configuration from changing the bytes: the line endings of files
# without an eol attribute (core.autocrlf, and core.eol for text files), the file modes, and attributes such as
# export-ignore from a global or system attributes file. git archive of a commit gives every entry the commit's time.
# gzip -n leaves the name and time out of its header. A shallow clone holds the commit and its tree, which is all git
# archive reads.
GIT_ATTR_NOSYSTEM=1 git -C "$root" -c core.autocrlf=false -c core.eol=lf -c tar.umask=0002 \
  -c core.attributesFile=/dev/null archive --format=tar --prefix="partsmith-$commit/" "$commit" |
  gzip -9 -n >"$temporary"
bytes="$(wc -c <"$temporary" | tr -d ' ')"
# sha256sum is GNU's. macOS has shasum instead.
if command -v sha256sum >/dev/null; then
  sha256="$(sha256sum <"$temporary" | cut -d ' ' -f 1)"
else
  sha256="$(shasum -a 256 <"$temporary" | cut -d ' ' -f 1)"
fi
mv -- "$temporary" "$archive"

echo "Wrote $archive from commit $commit: $bytes bytes, SHA-256 $sha256."
