# <p align="center">Link Opener</p>

<p align="center"> Link Opener extracts every URL from a file — Word, Excel, PowerPoint, PDF, or plain text/markup — and lets you select which links to open in your default browser. Drag-and-drop or "Open With" support included. </p>

## Supported file formats

| Category | Extensions |
|---|---|
| Word | `.docx`, `.doc` |
| Excel | `.xlsx`, `.xls`, `.xlsm`, `.xlsb`, `.ods` |
| PowerPoint | `.pptx` |
| PDF | `.pdf` |
| Plain text / markup | `.txt`, `.csv`, `.html`, `.htm`, `.md`, `.json`, `.xml`, `.yaml`, `.yml`, `.ini`, `.log`, `.css`, `.js`, `.ts`, `.rtf` |

## Download

Available for Windows

[Download Latest Release](https://github.com/hudsonpear/link-opener/releases)

## Usage

1. Launch the app.
2. Click **Select File...** (or drag & drop a file onto the window).
3. Extracted links appear in the list — select the ones you want.
4. Click **Open Links** to open them in your default browser.

Files can also be opened directly from the OS (double-click / "Open with" on an associated file type).

## Development

```
npm install
npm run tauri dev
```

## Building

```
npm run tauri build
```

Produces an MSI and NSIS installer under `src-tauri/target/release/bundle/`.

## Tech stack

- [Tauri 2](https://tauri.app/) (Rust backend, WebView frontend)
- Vanilla HTML/CSS/JS — no frontend framework
- `calamine` (spreadsheets), `docx-rs` (Word), `pdf-extract` (PDF), `zip` + `quick-xml` (PowerPoint), `cfb` (legacy `.doc`)
