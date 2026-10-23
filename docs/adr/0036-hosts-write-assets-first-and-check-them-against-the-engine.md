# 0036: Hosts write a document's assets before the document and check them against the engine's bytes

Status: Accepted

## Context

`save_document` returns the canonical text and every asset the document references, with their bytes (engine.md section 3.14). The host writes them: the text to the document's file, each asset to `assets/<64 hex digits of its BLAKE3 hash>.<extension>` next to it (file-format.md section 3.11). Python was the only host that wrote assets. Its `Document.save` replaced the `.ptsm` first and then wrote each asset whose name was not in `assets/` yet, trusting that a file under a hash name holds those bytes. That fails in two ways:

- An asset write that fails after the document was replaced (a full disk, a read-only folder, a killed process) leaves a document whose import opens with `asset_missing`.
- A file under the right name with other bytes (a truncated copy, a Git LFS pointer in a clone without LFS, a file edited by hand) is kept, so the next `open_document` of the folder fails with `asset_mismatch` for the whole document.

The web and desktop hosts do not write assets yet (gap list B-5) and will follow the same rule, so it is written once, in file-format.md section 3.11, for every host to cite.

## Decision

1. **Order.** A save writes the assets, then the document, then sends `mark_saved`. A document on disk then never names an asset that is missing from its folder, whichever step fails, and a crash between the steps leaves the old document with its old assets, which a save never removes.
2. **Check against the engine's bytes.** A file already under an asset's name is kept when its bytes equal the bytes `save_document` returned, and replaced otherwise. Those bytes hash to the name (the engine refuses any others, engine.md section 3.14), so this is the BLAKE3 check of the name without a BLAKE3 in the host.
3. **Atomic, flushed writes.** Each file goes to a new temporary file in the target's folder, whose name never matches the asset name pattern, is flushed to disk (`os.fsync` in Python, `File::sync_all` on the desktop, as desktop.md section 3.4 already does) and is renamed over the target. A platform whose file interface replaces a file whole (the browser's writable stream, which changes the file only when it is closed) uses that interface. Folders are not flushed.
4. **Failure.** The first failed step stops the save, removes its temporary file, reports the error for the file or folder it was writing (never the temporary file, docs/guidelines/errors.md), and sends no `mark_saved`, so the document stays unsaved. Assets already written stay.
5. **No deletion.** A save never deletes, renames or changes other files in `assets/`: other documents in the folder may use them (file-format.md section 3.11).

## Alternatives considered

- **Hash the existing file in the host.** Python's `hashlib` has no BLAKE3. The `blake3` package from PyPI would be a new dependency of the Python package (an ADR row and human review). A `blake3` function exported from `partsmith._core` would add public API to the extension module (python-api.md section 2.2) to compute what the comparison already decides. The web host would need BLAKE3 in JavaScript or a new protocol command. Every host already holds the asset's bytes from `save_document`.
- **Trust the name (the old rule).** It reads nothing, but a bad file under the right name is kept forever and the document cannot be opened from that folder.
- **Write the document first (the old order).** A failed or interrupted asset write leaves a document that names a missing asset.
- **Also flush the folders** (an `fsync` of the directory on POSIX after each rename). It would make the order durable by the POSIX rules rather than by the in-order metadata journaling of the common file systems. Windows cannot flush a folder, and desktop.md section 3.4 flushes files only, so both hosts would differ. It can be added if a file system shows the problem.
- **Delete unreferenced assets on save.** A save cannot know that no other document in the folder uses a file.

## Consequences

- A save reads each asset that is already in the folder, so a large shared asset costs a read per save (hashing it would cost the same). A host may replace a file whose size differs from the asset's without reading it.
- Python's atomic write, which the export functions share, flushes each file before its rename and reports a failed write under the name of the file being written (python-api.md sections 3.1 and 3.7).
- python-api.md tests 97 to 101 test the order in Python. ui.md and desktop.md cite file-format.md section 3.11 when B-5 makes those hosts write assets.
