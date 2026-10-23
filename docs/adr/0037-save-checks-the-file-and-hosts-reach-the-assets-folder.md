# 0037: Save checks the file before writing, and the web and desktop hosts reach the document's assets folder (two desktop commands)

Status: Proposed. Decisions 1, 7 and 8 change no schema, protocol, capability, license or dependency, so they need no review and gap list item B-6 does not wait for one. Decisions 2 to 6 need human review: decision 2 makes the UI read string values of the `.ptsm` text, an exception to ui.md section 1; decisions 3 to 6 are the desktop capability change with the reach of the stored grants, ADR 0036's options for the desktop, how the hosts treat symbolic links and where a linked document's assets live.

Amends [ADR 0014](0014-desktop-host-and-dependencies.md): its "six commands" become eight, and its "No fs plugin" bullet, under which the webview reaches files only through `read_file` and `write_file` and only files the user picked, gains the asset-named files next to a picked document.

## Context

Gap list items B-5 and B-6 share one review of the desktop's commands:

- **B-6.** Save writes over the document's file whatever happened to it since Partsmith read it, so a `git pull`, a Python `Document.save`, another tab or another program's save is lost without a word.
- **B-5.** The web and desktop hosts send `open_document` without assets (`ui/src/core/api.ts` sends `assets: []`) and write none when they save. So every document with an `import` feature, such as those Python writes, opens with `asset_missing`, Save as into another folder leaves its assets behind, and a core restart drops them.

ADR 0036 fixed the order every host writes in (file-format.md section 3.11). It left three questions about the desktop and the web to this review: what they may do in the folder (its options (a), the entry check, and (b), create only), whether the desktop's refusal of a read-only file applies to assets, and whether the desktop follows `assets` when it is a symbolic link, as Python does. python-api.md open issue 10 adds a fourth: Python replaces a linked document file, while the desktop writes through it.

The desktop's rule (desktop.md section 1) is that the webview reaches exactly the files the user chose in a dialog. The assets folder next to a chosen document is not one of them, so reading or writing it needs new commands and a capability change.

The web's File System Access API gives a page a file without its folder, shows no symbolic links and cannot flush (file-format.md section 3.11). Browsers without it (Firefox, Safari) read single files and save by downloading.

The readers that exist already, the CLI (`read_assets` in `crates/partsmith-cli/src/files.rs`, cli.md 2.3) and Python (`path.parent / "assets"`), look for the assets next to the document's path as given, also when that path is a symbolic link.

## Decision (proposed)

1. **Save checks the file first (B-6).** Each document session keeps the SHA-256 of its file's bytes as it last read or wrote them (`FileReference.digest`, from Web Crypto's `crypto.subtle.digest`); recovery entries keep it too, and recent entries do not need it, since Open recent reads the file again. Before it writes anything, Save reads the file (`readHandle`, which is the existing `read_file` on the desktop). When the bytes differ, it stops with `save_conflict` and offers "Overwrite {file}", "Save as" and "Open the newer file". The last of these keeps the unsaved document as a `replaced` recovery entry (ui.md section 3.19). Where Web Crypto cannot digest (outside a secure context), the digest is null with a warning in the console and Save goes on without the check. No host command changes.
2. **Open reads only the assets the text names.** The UI gives the core the files of `assets/` whose names are string values in the document's JSON (`referencedAssetNames`). That is the one place the UI reads `.ptsm` text: it collects the string values that match `ASSET_NAME` and interprets nothing else, an exception to ui.md section 1 ("never parses `.ptsm` text") and to the spirit of AGENTS.md's "The UI depends only on the protocol", which is why this decision needs review. Python gives the core every asset-named file instead. This way a shared folder of many assets costs only what the document uses. The UI keeps the bytes the core accepted (`DocumentState.assets`; none after an `asset_mismatch` retry), so a core restart gives them to the core again. Recovery entries keep the folder, not the bytes: an asset is named by its hash, so reading it again by name gives the same bytes or none.
3. **The desktop gets two commands, with option (a).** `read_asset` and `write_asset` take the grant of a `.ptsm` document (the grant of any other file, such as an export target, is refused with `UnknownGrant`) and reach only the entries of `assets/` in that document's folder whose names are assets: 64 lowercase hex digits and an extension of `ASSET_EXTENSIONS`, which is `step` in M1 (the engine's only asset kind, engine.md 3.14), so the webview cannot create a `.exe`, `.bat`, `.lnk` or `.desktop` file there; the BLAKE3 check alone would not stop it, since a hostile page can hash what it writes. Their permissions, `allow-read-asset` and `allow-write-asset`, are the only new ones. `write_asset` runs the entry check of file-format.md section 3.11 (option (a) of ADR 0036). It also accepts only bytes whose BLAKE3 hash is the name, so a save can never make a correct asset wrong. No asset grant is stored, nothing is listed or deleted, and no folder other than `assets/` is created (desktop.md sections 3.2 and 3.4).
4. **The desktop follows no link in the assets folder.** When `assets` is a symbolic link, both commands refuse it (`AssetsNotAFolder`). An entry under an asset's name that is a link is never read, and a save replaces it with the file (file-format.md section 3.11). Python follows a linked `assets` (ADR 0036 point 7), so the hosts differ here. "Never" holds against the webview, which cannot create links, but not against another local process racing the host: `assets` is checked and then used by its path, so a link swapped in between is followed, and the entry check opens a file without `O_NONBLOCK`, so a pipe swapped in blocks one thread of the blocking pool. Closing both needs `openat`-style calls and the flag, which std lacks without `unsafe` or a new dependency (desktop.md open issue 10).
5. **The desktop's read-only refusal covers assets.** A read-only file under an asset's name that holds other bytes fails the save with `Denied` ("the file is read-only"), as a read-only document does (desktop.md section 3.4 step 4.2). A read-only file with the right bytes is kept, since nothing needs writing. Python replaces it.
6. **On the desktop, a document's assets folder is next to the granted path itself.** For a granted symbolic link, that is the link's folder, the folder the user saw in the dialog and the one the CLI and Python read; `write_file` still writes the document's bytes to the file the link points to (desktop.md 3.4 step 4.1). So every host finds a linked document's assets in the same place, and the grant reaches no folder the user did not see.
7. **The web asks for the folder when it needs it.** The page holds a `FileSystemDirectoryHandle` the user chose (`showDirectoryPicker` with `startIn` the document's file and mode `readwrite`). It checks that the folder holds the file directly (`resolve`) and keeps it with the document, its recent entry and its recovery entry; opening the same file again with the picker takes the folder of its recent entry. When it holds a folder, it asks for the folder's permission before the file's, so the one prompt a click may show is the folder's, which in Chromium also covers the files in it. It asks for the folder through a notification's "Choose folder" button, so a click lets the page open the dialog, in three cases: an opened document misses an asset, the browser refused the kept folder's permission, or a save has assets for a folder the page does not hold. A canceled dialog shows the notification again. Once a folder is chosen, the document is reopened with its assets. These actions run only for the document session and file they were offered for, and wait during a feature command, as an `apply` action does. Without the API the page reads no folder, so the core holds no assets and a download needs none. The UI then says the imports are missing and where they work.
8. **A wrong file under an asset's name at open.** The core refuses the whole document (`asset_mismatch`). The UI then opens the text again without assets and says so (`assets_mismatch`), so the document can still be opened and edited.

## Options

**The change check (B-6):**

- (i) A content digest, read before each save (recommended, decision 1). It is exact, the same code runs on both hosts, and no host command changes. It reads the whole file once per save; a `.ptsm` of 2,000 features is about 1 MB.
- (ii) Modification time and size, from `File.lastModified` on the web and a new field in the desktop's `read_file` reply. It never reads the contents, but it has three costs. It misses a rewrite within the file system's time resolution (2 s on FAT, 1 s on HFS+) or by a tool that keeps the time. It warns for nothing after a `touch`, a sync client's rewrite or a checkout of the same bytes. And it changes a desktop command, which would bring B-6 under this review.
- (iii) A compare and write in the host (`write_file` with the expected digest, refused when the file differs). It shrinks the gap between the check and the write to the host, but the web has no conditional write, so the hosts would differ, and it changes a desktop command, which would bring B-6 under this review.

**Which assets Open gives the core** (decision 2):

- (i) The asset names among the string values of the text (recommended). One open, one recompute; the UI reads strings of the text and nothing else.
- (ii) Open without assets, take the names from the `asset` params of the import records in the recipe of `document_changed` (protocol data), then reopen with those assets as Choose folder does. It needs no text reading at all, but a document with imports is opened and recomputed twice, and the first recompute fails every import.
- (iii) Every asset-named file of the folder, as Python does (see Alternatives).

**What the desktop may do in `assets/`** (ADR 0036):

- (a) Create, read and replace, with the entry check (recommended, decision 3).
- (b) Create only: `write_asset` writes an asset whose name is not there yet and never reads an existing entry. A wrong file under the name then stays, so its document keeps failing with `asset_mismatch`, and the desktop differs from Python.

**Which stored grants reach the assets folder on the desktop** (decision 3, desktop.md open issue 5). Grants persist across restarts, up to 100, so with any of them the two commands let a compromised webview, with no user action, create `assets/` next to any `.ptsm` granted in the past, write `.step` files of any content there and read the asset-named files of those folders:

- (i) Every stored `.ptsm` grant, as `read_file` and `write_file` take them (recommended, and what the code does). Every path through the UI that reads or writes assets works, a recovery copy opened at startup included. The added reach is a sibling folder of `.step` files only, while `write_file` already lets such a webview rewrite those documents themselves.
- (ii) Only grants that a pick, `read_file` or `write_file` used since the app started. The stored grants then stop reaching assets until the user opens or saves the document again. Open recent, "Open the newer file" and a Save with a known digest are unaffected, since each calls `read_file` first. Two paths change: a recovery copy opened at startup reads no assets, so its imports are missing until the document is opened from its file, and a Save whose digest is not known (a recovery entry from before digests existed) fails with `save_assets`; the UI would read the document's file first in both cases to keep them working. It changes `files.rs` (a set of grants used in this process), desktop.md 3.4 and its tests, and, for those two paths, `src/app/actions/document.ts`.

**A linked `assets` folder on the desktop:**

- (i) Follow it, as Python does. A shared folder of assets then works in both hosts. But a link in a downloaded or cloned folder can point anywhere, so the webview could read and create asset-named files in a folder nobody chose.
- (ii) Never follow it (recommended, decision 4). The grant then reaches only the folder next to the chosen document. A project that links `assets` works in Python but not on the desktop.
- (iii) Follow it for reading only. Open then works with a shared folder, but Save fails there, which is harder to explain than (ii).

**A read-only wrong asset on the desktop:** refuse it (recommended, decision 5, as for documents), or replace it as Python does.

**Where a linked document's assets live** (decision 6):

- Next to the granted link (recommended). Every host reads them there already, and the grant reaches only the folder the user saw.
- Next to the file the link points to, as an earlier draft of this ADR proposed. The document and its assets would stay together where the bytes are, but the CLI and Python would then open a desktop-saved document with `asset_missing`, unless both changed (cli.md 2.3 and its tests, python-api.md 3.1), and the grant would reach a folder the user never saw in the dialog, which a cloned folder's link can choose.

**The linked document file itself** (python-api.md open issue 10; the assets stay next to the link in every case):

- (A) Both hosts write through the link to the file it points to. This is recommended: the desktop already does it, and it keeps a user's link in place. Python would change its document write (python-api.md 3.1) to replace the file the link points to; gap list item PY-1's follow-up tracks that change if (A) is accepted.
- (B) Both hosts replace the link with a regular file. The desktop would change desktop.md section 3.4 step 4.1 and its test 51.
- (C) Keep the difference.

Changing Python is not part of B-5.

**A Save as that waits for its folder on the web** (file-format.md 3.11, ui.md open issue 37). Chromium's save dialog creates the file it names and, as far as we recall its code (to confirm at the gate), empties a file that exists, before the page writes anything:

- Write nothing until the folder is chosen (the current rule). The order of file-format.md 3.11 holds, but a dismissed `save_folder_needed` can leave an existing file empty.
- When the target is empty once the dialog returns (it was created or emptied by the dialog), write the document at once without `mark_saved`, and let "Choose folder" write the assets and the document again. No file is left empty, but for a while the document sits there without its assets, against the order of file-format.md 3.11. Recommended if the gate confirms that the dialog empties an existing file, since the previous contents are then gone either way.

## Alternatives considered

- **Every asset-named file on open, as Python does.** On a shared folder, it reads and copies to the core every asset of every document there, and the UI would keep all of their bytes for a restart.
- **Asset bytes in the recovery entries.** A recovery write would rewrite every asset with the text, every 2 s while the user edits. The folder handle (web) or the grant (desktop) reaches the same bytes by name.
- **A protocol command that adds assets to an open document**, for the web's folder choice. It would keep the undo history, but it is a protocol schema change made for one host's limit. Reopening the settled recipe, as a core restart does (ui.md section 4.4), needs nothing new.
- **One desktop command that reads every asset at once.** It makes fewer calls, but it needs a framing of its own. One call per asset returns raw bytes, as `read_file` does.
- **BLAKE3 in the UI**, to check assets or the document. It would be a new dependency. The engine already checks assets at open, and comparing with the engine's bytes checks them at save.
- **Asking for the folder at once on every web Open.** Most documents import nothing, so this would add a second dialog to almost every Open.
- **Reaching the document through the folder handle** (`folder.getFileHandle(name)`) whenever the page holds a folder, so only the folder's permission is ever asked. It would not rely on Chromium's folder grant covering its files, but `readHandle` and `write` would take the folder, and the page would hold two handles to one file. Asking for the folder's permission first gives the same single prompt where the grant covers the files, and is no worse where it does not.
- **`<input type="file" webkitdirectory>` for Firefox and Safari.** It reads a chosen folder without the File System Access API, so those browsers could view a document's imports. It is not used: it reads the whole folder tree, the browser shows an upload prompt for it, and it cannot write, so Save could not keep the assets. A `.ptsz` bundle (io.md, Later), which carries the assets inside the file, is the better answer for those browsers.

## Consequences

- Save reads the document's file once before writing. A file changed elsewhere is no longer overwritten without a warning, except for a change that lands between that read and the write (ui.md open issue 38).
- The desktop's capability grows by two permissions, and its stored grants reach more than the files the user chose: the `assets/` folder next to each of up to 100 `.ptsm` documents granted in the past, which a compromised webview could fill with `.step` files of its choosing without the user opening the document in this session (desktop.md open issue 5; the options above say how to limit that). `COMMAND_NAMES`, `permissions/autogenerated/`, `capabilities/main.json`, desktop.md sections 1, 1.1, 2.1, 2.2, 2.4, 2.6, 2.7, 2.9, 3.2, 3.4, 3.5 and 3.7, its tests 11 and 34, and `desktop/src-tauri/AGENTS.md` ("six commands", "exactly the files the user chose") change with the code, and tests 54 to 66 are added.
- A web document with imports asks for its folder once per document session that has none. A recent entry and a recovery copy keep the folder. Choosing a folder reopens the document, which ends its undo history (ui.md open issue 37). A document saved directly in the home folder (and, we believe, in Desktop, Documents or Downloads) cannot get its imports on the web if Chromium's directory dialog refuses that folder: move the document and its assets folder into a folder of their own (ui.md open issue 39).
- What the end-to-end tests cannot show, since they run on the origin private file system in Chromium on Linux, is checked by hand at the gate: the real permission prompts after a reload, `resolve` on local folders, the directory dialog's refusals and the save dialog's emptying of an existing file (ui.md open issue 39), and on the desktop the entry check on Windows, which compares length and modification time instead of the inode, and on macOS (desktop.md section 6, manual checks).
- The desktop needs no new gate check for Web Crypto: it already needs a secure context for `crypto.randomUUID()` and Web Locks (desktop.md open issue 11).
- The code of gap list items B-5 and B-6 implements every decision as proposed here, the capability change included (`COMMAND_NAMES`, `permissions/autogenerated/read_asset.toml` and `write_asset.toml`, `capabilities/main.json`), so another choice at the review changes the desktop's `files.rs`, `commands.rs` and capability and the UI's `src/app/actions/document.ts` and `document-assets.ts`.
- file-format.md section 3.11 cites ui.md section 3.19 and desktop.md section 3.4 for these hosts. ui.md tests U-216 to U-232 and E-150 to E-154, and desktop.md tests 54 to 66, test the rules.
