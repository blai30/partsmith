# 0036: Hosts write a document's assets before the document and check them against the engine's bytes

Status: Accepted for Python (PY-1); the rule for the web and desktop hosts is proposed, needs human review with B-5's capability review

## Context

`save_document` returns the canonical text and every asset the document references, with their bytes (engine.md section 3.14). The host writes them: the text to the document's file, each asset to `assets/<64 hex digits of its BLAKE3 hash>.<extension>` next to it (file-format.md section 3.11). Python was the only host that wrote assets. Its `Document.save` replaced the `.ptsm` first and then wrote each asset whose name was not in `assets/` yet, trusting that a file under a hash name holds those bytes. That fails in two ways:

- An asset write that fails after the document was replaced (a full disk, a read-only folder, a killed process) leaves a document whose import opens with `asset_missing`.
- A file under the right name with other bytes (a truncated copy, a Git LFS pointer in a clone without LFS, a file edited by hand) is kept, so the next `open_document` of the folder fails with `asset_mismatch` for the whole document.

The web and desktop hosts do not write assets yet (gap list B-5) and will follow the same rule, so it is written once, in file-format.md section 3.11, for every host to cite. What those hosts may do in the folder is part of B-5's desktop capability review, a human gate, so the rule is decided here for Python only.

## Decision

1. **Order.** A save writes the assets, then the document, then sends `mark_saved`. The document is then replaced only after every asset `save_document` returned is in `assets/`, and a failed or interrupted save leaves the previous document in place with the assets it had, since a save adds or repairs assets and never removes one. A referenced asset the engine does not hold (the document was opened without it) is not in the result and stays missing.
2. **Check against the engine's bytes, never through a link.** The host looks at the entry under an asset's name without following a symbolic link. A regular file of the asset's size is read, and kept when its bytes equal the bytes `save_document` returned. Any other entry (a file of another size or with other bytes, a symbolic link, a pipe) is replaced, and the rename replaces the entry itself, never the file a link points to. A folder under the name fails the save and is never removed. The engine's bytes hash to the name (the engine refuses any others, engine.md section 3.14), so the comparison is the BLAKE3 check of the name without a BLAKE3 in the host. Python reads the file through one descriptor that it first checks is still the file it looked at (python-api.md section 3.1), so an entry swapped for a link or a pipe after the look is not read either.
3. **Atomic, flushed writes.** Each file is written with the host's own atomic write: a new temporary file in the target's folder, whose name never matches the asset name pattern, flushed and renamed over the target (python-api.md section 3.1 with `os.fsync`; desktop.md section 3.4 step 4 with `File::sync_all`). On macOS `os.fsync` flushes to the operating system but not the drive's cache, while Rust's `File::sync_all` uses `F_FULLFSYNC`, so a Python save there is durable against a crash of the process or the system but not against a power cut. A platform whose file interface replaces a file whole (the browser's writable stream, which changes the file only when it is closed) uses that interface, and removes an entry it had to create first (`getFileHandle` with `create: true`) when the write fails. The web's file system API can neither see a link (a handle's `kind` is only `file` or `directory`) nor flush a file; ui.md section 3.19 states what the web does about both with B-5 (file-format.md section 3.11). Folders are not flushed.
4. **Failure.** The first failed step stops the save, removes its temporary file, reports which file or folder failed, never the temporary file, worded as its host's spec says (python-api.md section 3.1 puts the path in `OSError.filename`; the UI words it as docs/guidelines/errors.md says), and sends no `mark_saved`, so the document stays unsaved. Assets already written stay.
5. **No deletion.** A save never deletes, renames or changes other entries in `assets/`: other documents in the folder may use them (file-format.md section 3.11).
6. **Hosts without the folder.** A host that cannot write the document's folder (the web with only a file handle, or a download) does not write assets; its own spec decides what it does instead (ui.md section 3.19, gap list B-5).
7. **A linked `assets/` folder.** Python follows `assets/` when it is a symbolic link to a folder, like any folder of the document's path, so a folder of assets shared by several project folders keeps working, and `Document.open` already reads through the same link. A link that leads to no folder fails the save. The desktop's rule is decided with B-5's capability review.

## Options for the desktop host (B-5, needs human review)

The desktop grants single files (desktop.md section 3.2), so writing assets needs a new grant for the granted document's sibling `assets/` folder. Two options:

- **(a) Create, read and replace.** The host may create `assets/` next to a granted document, read a regular file under an asset's name and replace an entry under that name (never through a link, never a folder), all with names that match the asset name pattern only. This is the entry check of file-format.md section 3.11 step 1, which binds Python.
- **(b) Create only.** The host may create `assets/` and write an asset whose name is not there yet, but never read or replace an existing entry. The grant is smaller, but a wrong file under an asset's name stays, so the `asset_mismatch` case of the Context is not repaired, and the desktop would differ from Python.

Recommended: (a), the only option that repairs a wrong file under an asset's name. Whether the desktop's refusal of a read-only target (desktop.md section 3.4 step 4.2) applies to assets is decided in the same review.

## Alternatives considered

- **Hash the existing file in the host.** Python's `hashlib` has no BLAKE3. The `blake3` package from PyPI would be a new dependency of the Python package (an ADR row and human review). A `blake3` function exported from `partsmith._core` would add public API to the extension module (python-api.md section 2.2) to compute what the comparison already decides. The web host would need BLAKE3 in JavaScript or a new protocol command. Every host already holds the asset's bytes from `save_document`.
- **Trust the name (the old rule).** It reads nothing, but a bad file under the right name is kept forever and the document cannot be opened from that folder.
- **Write the document first (the old order).** A failed or interrupted asset write leaves a document that names a missing asset.
- **Follow a symbolic link under an asset's name** (compare the file it points to, keep a link to the right bytes, and replace a link to other bytes as an entry, never writing through it). It would keep a folder that shares assets through links (made by hand, or by git-annex) as it is. But the comparison reads a file outside `assets/`, which a link in a downloaded or cloned folder can place anywhere and which the desktop's grant for `assets/` (B-5) would not cover, and a kept link leaves the folder depending on a file that another clone or machine may not have. Replacing the link costs one write of the asset's bytes.
- **Use `F_FULLFSYNC` on macOS in Python.** It would match the desktop, but Python's CI runs on Linux only, so the branch would be untested. It can be added with a macOS Python job.
- **Also flush the folders** (an `fsync` of the directory on POSIX after each rename). It would make the order durable by the POSIX rules rather than by the in-order metadata journaling of the common file systems. Windows cannot flush a folder, and desktop.md section 3.4 flushes files only, so both hosts would differ. It can be added if a file system shows the problem.
- **Delete unreferenced assets on save.** A save cannot know that no other document in the folder uses a file.

## Consequences

- A save reads each regular file of an asset's size that is already in the folder, so a large shared asset costs a read per save (hashing it would cost the same). A file of another size is replaced without being read.
- Python's atomic write, which the export functions share, flushes each file before its rename and reports a failed write under the name of the file being written, with the OS's `errno` and `strerror` (python-api.md sections 3.1 and 3.7). `python -m partsmith build` names that file in its error line (python-api.md section 2.10).
- A symbolic link under an asset's name is replaced by a regular file holding the asset's bytes even when it points to the right bytes, so a folder that shares assets through links gets a copy in place of each such link on its next save, which Git shows as a type change.
- python-api.md tests 97 to 102 test the rule in Python, and test 103 the export errors of the same atomic write. ui.md and desktop.md cite file-format.md section 3.11 when B-5 makes those hosts write assets.
