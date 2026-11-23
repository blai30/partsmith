# W2-1D-3: A download Save also downloads the imported files

Status: Accepted (owner's rule of 2026-11-05): ui.md open issue 45, which ADR 0064 left to the first caller of a picked file, and plan choice "A download Save of a document holding picked assets", option (i). It binds wave 3's font assets too. No file format, protocol, naming, license, dependency or desktop change. It changes the UI's Save and Save as on a browser without the File System Access API (Firefox and Safari, ui.md 1.1).

## Context

- **What a download Save does in M1** (ui.md 3.19, read at 0f578c1). Without the File System Access API, `chooseSaveTarget` gives a download target and Save as writes the document with `files.write`, which clicks an `<a download>` of an object URL. The page reads no folder there, so the core held no asset, and step 3 of the write says a download target never reaches the asset step.
- **What changes in M2.** [ADR W2-1D-2](W2-1D-2-step-import-in-the-ui-and-the-cli-through-import-step.md) imports a picked STEP file on every browser, through a file input where the API is missing. `save_document` then returns the file, and the page cannot write it next to the downloaded document. The next open of that document reports `asset_missing`.
- **What browsers do with several downloads** (their documentation and settings, not measured here, ui.md open issue 47). A download lands in the browser's download folder, under the name the page gives, or that name with a number when a file of that name is there (Firefox and Chromium add ` (1)` before the extension). No browser lets a page choose a subfolder. Chromium asks once per site before a page starts a second download without a new click ("Download multiple files", its "Automatic downloads" setting). The download target is used only where the API is missing, which in ui.md 1.1's browsers means Firefox and Safari. Firefox saves each download to its folder without a prompt unless the user turned on "Always ask you where to save files". Safari asks once per site before the first download ("Do you want to allow downloads on ...?") unless the user changed the site's Downloads setting.
- **The order of writes.** file-format.md 3.11 and [ADR 0036](0036-hosts-write-assets-first-and-check-them-against-the-engine.md): a host writes the assets before the document, so a document never names a file that is not there.

## Decision

1. **The steps** (ui.md 3.19, writing the document, step 3). For a download target, when `save_document` returned assets, the page downloads each asset first, in the result's order (sorted by name), through `files.write({ kind: 'download', name }, kind, bytes)`, under its asset name (`<64 hex>.step`, `.ttf` or `.otf`), with the file kind `step` for a `.step` asset and `font` for the others, so the media type of `FILE_TYPES` (`model/step`, `font/sfnt`). Then it downloads the document as in M1, then sends `markSaved`. A download that throws stops the save with `save_io`, as a failed document write does, and nothing after it is downloaded.
2. **The message.** After the document's download, the persistent info notification `download_asset` (one asset) or `download_assets` replaces `download_hint` for that save: "{file} and its imported file are in your downloads folder. Move the file named {asset} into a folder named assets next to {file}, or Partsmith cannot find it when you open {file} again." With more than one asset: "{file} and its {count} imported files are in your downloads folder. Move the files whose names end in .step, .ttf or .otf into a folder named assets next to {file}, or Partsmith cannot find them when you open {file} again." It shows after every download Save with assets, not only the first of a session, since the user moves files by hand each time.
3. **The way back.** An opened document whose assets the page cannot read (no folder in Firefox and Safari) shows `assets_unsupported`, which now names the action: each import's `asset_missing` message offers "Choose file..." ([ADR W2-1D-2](W2-1D-2-step-import-in-the-ui-and-the-cli-through-import-step.md) decision 5). Picking the downloaded file adds it by its name, and the import builds.
4. **The test.** E-178, tagged `@engines`, runs in Firefox and WebKit (and Chromium with the API turned off, as the download E-tests do): it imports `S`, saves, and checks that the page downloaded exactly two files, `S` then `Untitled.ptsm`, with the bytes of `S` and the document's text, and that `download_assets` shows. Playwright accepts every download, so the browsers' own prompts are a human check at the gate (m2-plan.md gate 20's preview review in Chrome, Firefox and Safari).

## Alternatives considered

- (i) **Download each asset too** (chosen). Nothing is lost on Firefox and Safari, and gate 17's `@engines` import test can pass there. The industry standard where no folder can be written: slicers and editors that export a model with its textures to a browser download each file. Safe as long as the message says where the files go.
- (ii) **Refuse the save with a warning.** The work is not saved at all on Firefox and Safari. Unsafe.
- (iii) **Offer Import only with the File System Access API or on the desktop.** Firefox and Safari users lose the command that m2-plan.md item 22 gives every host. Gate 17's `@engines` subset includes import.
- (iv) **Warn at import time and keep M1's save.** The document downloads without its file, and the warning comes before the user has anything to move.
- **A zip of the document and its assets.** That is the `.ptsz` bundle of M4 (m2-plan.md section 5.2), a format of its own that every host must open. Not in M2.

## Consequences

- A download Save of a document with imports puts more than one file in the downloads folder, and the user moves the asset files by hand. A renamed duplicate (`<hash> (1).step`) does not match its name: "Choose file..." still adds it, since the core checks bytes, not file names.
- Wave 3's fonts follow the same steps with no change to this ADR.
- M4's `.ptsz` bundles end the hand move. This ADR is then superseded for documents saved as bundles.
- The browsers' prompts are not measured in CI. ui.md open issue 47 records what the gate's human check finds.
