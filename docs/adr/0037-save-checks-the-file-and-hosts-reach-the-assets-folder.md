# 0037: Save checks the file before writing, and the web and desktop hosts reach the document's assets folder (two desktop commands)

Status: Proposed, needs human review (the desktop capability change, ADR 0036's options for the desktop, and how the hosts treat symbolic links)

## Context

Gap list items B-5 and B-6 share one review of the desktop's commands:

- **B-6.** Save writes over the document's file whatever happened to it since Partsmith read it, so a `git pull`, a Python `Document.save`, another tab or another program's save is lost without a word.
- **B-5.** The web and desktop hosts send `open_document` without assets (`ui/src/core/api.ts` sends `assets: []`) and write none when they save. So every document with an `import` feature, such as those Python writes, opens with `asset_missing`, Save as into another folder leaves its assets behind, and a core restart drops them.

ADR 0036 fixed the order every host writes in (file-format.md section 3.11). It left three questions about the desktop and the web to this review: what they may do in the folder (its options (a), the entry check, and (b), create only), whether the desktop's refusal of a read-only file applies to assets, and whether the desktop follows `assets` when it is a symbolic link, as Python does. python-api.md open issue 10 adds a fourth: Python replaces a linked document file, while the desktop writes through it.

The desktop's rule (desktop.md section 1) is that the webview reaches exactly the files the user chose in a dialog. The assets folder next to a chosen document is not one of them, so reading or writing it needs new commands and a capability change.

The web's File System Access API gives a page a file without its folder, shows no symbolic links and cannot flush (file-format.md section 3.11). Browsers without it (Firefox, Safari) read single files and save by downloading.

## Decision (proposed)

1. **Save checks the file first (B-6).** Each document session keeps the SHA-256 of its file's bytes as it last read or wrote them (`FileReference.digest`, from Web Crypto's `crypto.subtle.digest`; recent and recovery entries keep it too). Before it writes anything, Save reads the file (`readHandle`, which is the existing `read_file` on the desktop). When the bytes differ, it stops with `save_conflict` and offers "Overwrite {file}", "Save as" and "Open the newer file". The last of these keeps the unsaved document as a `replaced` recovery entry (ui.md section 3.19). No host command changes.
2. **Open reads only the assets the text names.** The UI gives the core the files of `assets/` whose names are string values in the document's JSON (`referencedAssetNames`). Python gives it every asset-named file instead. This way a shared folder of many assets costs only what the document uses. The UI keeps those bytes (`DocumentState.assets`), so a core restart gives them to the core again. Recovery entries keep the folder, not the bytes: an asset is named by its hash, so reading it again by name gives the same bytes or none.
3. **The desktop gets two commands, with option (a).** `read_asset` and `write_asset` take the grant of a document and reach only the asset-named entries of `assets/` in that document's folder. Their permissions, `allow-read-asset` and `allow-write-asset`, are the only new ones. `write_asset` runs the entry check of file-format.md section 3.11 (option (a) of ADR 0036). It also accepts only bytes whose BLAKE3 hash is the name, so a save can never make a correct asset wrong. No asset grant is stored, nothing is listed or deleted, and no folder other than `assets/` is created (desktop.md sections 3.2 and 3.4).
4. **The desktop follows no link in the assets folder.** When `assets` is a symbolic link, both commands refuse it (`AssetsNotAFolder`). An entry under an asset's name that is a link is never read, and a save replaces it with the file (file-format.md section 3.11). Python follows a linked `assets` (ADR 0036 point 7), so the hosts differ here.
5. **The desktop's read-only refusal covers assets.** A read-only file under an asset's name that holds other bytes fails the save with `Denied` ("the file is read-only"), as a read-only document does (desktop.md section 3.4 step 4.2). A read-only file with the right bytes is kept, since nothing needs writing. Python replaces it.
6. **On the desktop, the document's folder is that of the file `write_file` writes.** For a granted symbolic link, that is the folder of the file it points to, so the document and its assets stay together where its bytes are.
7. **The web asks for the folder when it needs it.** The page holds a `FileSystemDirectoryHandle` the user chose (`showDirectoryPicker` with `startIn` the document's file and mode `readwrite`). It checks that the folder holds the file directly (`resolve`) and keeps it with the document, its recent entry and its recovery entry. It asks through a notification's "Choose folder" button, so a click lets the page open the dialog, in two cases: an opened document misses an asset, or a save has assets for a folder the page does not hold. Once a folder is chosen, the document is reopened with its assets. Without the API the page reads no folder, so the core holds no assets and a download needs none. The UI then says the imports are missing and where they work.
8. **A wrong file under an asset's name at open.** The core refuses the whole document (`asset_mismatch`). The UI then opens the text again without assets and says so (`assets_mismatch`), so the document can still be opened and edited.

## Options

**The change check (B-6):**

- (i) A content digest, read before each save (recommended, decision 1). It is exact, the same code runs on both hosts, and no host command changes. It reads the whole file once per save; a `.ptsm` of 2,000 features is about 1 MB.
- (ii) Modification time and size, from `File.lastModified` on the web and a new field in the desktop's `read_file` reply. It never reads the contents, but it has three costs. It misses a rewrite within the file system's time resolution (2 s on FAT, 1 s on HFS+) or by a tool that keeps the time. It warns for nothing after a `touch`, a sync client's rewrite or a checkout of the same bytes. And it changes a desktop command.
- (iii) A compare and write in the host (`write_file` with the expected digest, refused when the file differs). It shrinks the gap between the check and the write to the host, but the web has no conditional write, so the hosts would differ, and it changes a desktop command.

**What the desktop may do in `assets/`** (ADR 0036):

- (a) Create, read and replace, with the entry check (recommended, decision 3).
- (b) Create only: `write_asset` writes an asset whose name is not there yet and never reads an existing entry. A wrong file under the name then stays, so its document keeps failing with `asset_mismatch`, and the desktop differs from Python.

**A linked `assets` folder on the desktop:**

- (i) Follow it, as Python does. A shared folder of assets then works in both hosts. But a link in a downloaded or cloned folder can point anywhere, so the webview could read and create asset-named files in a folder nobody chose.
- (ii) Never follow it (recommended, decision 4). The grant then reaches only the folder next to the chosen document. A project that links `assets` works in Python but not on the desktop.
- (iii) Follow it for reading only. Open then works with a shared folder, but Save fails there, which is harder to explain than (ii).

**A read-only wrong asset on the desktop:** refuse it (recommended, decision 5, as for documents), or replace it as Python does.

**The linked document file** (python-api.md open issue 10):

- (A) Both hosts write through the link to the file it points to and keep the assets next to that file. This is recommended: the desktop already does it (decision 6), and Python would move its document write and its `assets/` folder to the target's folder.
- (B) Both hosts replace the link with a regular file. The desktop would change desktop.md section 3.4 step 4.1 and its test 51.
- (C) Keep the difference.

Changing Python is not part of B-5.

## Alternatives considered

- **Every asset-named file on open, as Python does.** On a shared folder, it reads and copies to the core every asset of every document there, and the UI would keep all of their bytes for a restart.
- **Asset bytes in the recovery entries.** A recovery write would rewrite every asset with the text, every 2 s while the user edits. The folder handle (web) or the grant (desktop) reaches the same bytes by name.
- **A protocol command that adds assets to an open document**, for the web's folder choice. It would keep the undo history, but it is a protocol schema change made for one host's limit. Reopening the settled recipe, as a core restart does (ui.md section 4.4), needs nothing new.
- **One desktop command that reads every asset at once.** It makes fewer calls, but it needs a framing of its own. One call per asset returns raw bytes, as `read_file` does.
- **BLAKE3 in the UI**, to check assets or the document. It would be a new dependency. The engine already checks assets at open, and comparing with the engine's bytes checks them at save.
- **Asking for the folder at once on every web Open.** Most documents import nothing, so this would add a second dialog to almost every Open.

## Consequences

- Save reads the document's file once before writing. A file changed elsewhere is no longer overwritten without a warning, except for a change that lands between that read and the write (ui.md open issue 38).
- The desktop's capability grows by two permissions. `COMMAND_NAMES`, `permissions/autogenerated/`, `capabilities/main.json`, desktop.md sections 1.1, 2.2, 2.4, 2.6, 2.7, 2.9, 3.2, 3.4 and 3.7, and its tests 11 and 34 change, and tests 54 to 66 are added.
- A web document with imports asks for its folder once per document session that has none. A recent entry and a recovery copy keep the folder. Choosing a folder reopens the document, which ends its undo history (ui.md open issue 37).
- The desktop gate check confirms that `crypto.subtle` exists in WebView2, WKWebView and WebKitGTK (desktop.md open issue 11).
- file-format.md section 3.11 cites ui.md section 3.19 and desktop.md section 3.4 for these hosts. ui.md tests U-216 to U-232 and E-150 to E-154, and desktop.md tests 54 to 66, test the rules.
