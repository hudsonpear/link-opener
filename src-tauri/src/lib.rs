use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;

use calamine::{open_workbook_auto, Reader};
use quick_xml::events::Event;
use serde::Serialize;
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

struct PendingFile(Mutex<Option<String>>);

#[derive(Serialize)]
struct FileReadResult {
    success: bool,
    text: String,
    #[serde(rename = "type")]
    file_type: String,
    size: u64,
}

fn find_file_arg(args: &[String]) -> Option<String> {
    args.iter()
        .skip(1)
        .find(|a| Path::new(a).extension().is_some())
        .cloned()
}

fn read_docx_text(bytes: &[u8]) -> Result<String, String> {
    let docx = docx_rs::read_docx(bytes).map_err(|e| e.to_string())?;
    let mut out = String::new();

    fn push_run(run: &docx_rs::Run, out: &mut String) {
        for child in &run.children {
            if let docx_rs::RunChild::Text(t) = child {
                out.push_str(&t.text);
            }
        }
    }

    fn push_paragraph(p: &docx_rs::Paragraph, out: &mut String) {
        for child in &p.children {
            match child {
                docx_rs::ParagraphChild::Run(r) => push_run(r, out),
                docx_rs::ParagraphChild::Hyperlink(h) => {
                    for hc in &h.children {
                        if let docx_rs::ParagraphChild::Run(r) = hc {
                            push_run(r, out);
                        }
                    }
                }
                _ => {}
            }
        }
        out.push('\n');
    }

    for child in &docx.document.children {
        match child {
            docx_rs::DocumentChild::Paragraph(p) => push_paragraph(p, &mut out),
            docx_rs::DocumentChild::Table(t) => {
                for row in &t.rows {
                    let docx_rs::TableChild::TableRow(row) = row;
                    for cell in &row.cells {
                        let docx_rs::TableRowChild::TableCell(cell) = cell;
                        for cc in &cell.children {
                            if let docx_rs::TableCellContent::Paragraph(p) = cc {
                                push_paragraph(p, &mut out);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(out)
}

fn read_xlsx_text(path: &str) -> Result<String, String> {
    let mut workbook = open_workbook_auto(path).map_err(|e| e.to_string())?;
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or("Workbook has no sheets")?;
    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|e| e.to_string())?;

    let lines: Vec<String> = range
        .rows()
        .map(|row| {
            row.iter()
                .map(|cell| cell.to_string())
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect();

    Ok(lines.join("\n"))
}

fn read_pptx_text(path: &str) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let slide_re = regex::Regex::new(r"^ppt/slides/slide(\d+)\.xml$").unwrap();

    let mut slides: Vec<(u32, String)> = Vec::new();
    for i in 0..archive.len() {
        let name = archive
            .by_index(i)
            .map_err(|e| e.to_string())?
            .name()
            .to_string();
        if let Some(caps) = slide_re.captures(&name) {
            slides.push((caps[1].parse().unwrap_or(0), name));
        }
    }
    slides.sort_by_key(|(n, _)| *n);

    let mut out = String::new();
    for (n, name) in &slides {
        let mut entry = archive.by_name(name).map_err(|e| e.to_string())?;
        let mut xml = String::new();
        entry
            .read_to_string(&mut xml)
            .map_err(|e| e.to_string())?;
        if *n > slides[0].0 {
            out.push('\n');
        }
        out.push_str(&extract_pptx_slide_text(&xml)?);
    }
    Ok(out)
}

fn extract_pptx_slide_text(xml: &str) -> Result<String, String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut out = String::new();
    let mut in_text = false;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf).map_err(|e| e.to_string())? {
            Event::Start(e) if e.name().as_ref().ends_with(b"a:t") => in_text = true,
            Event::End(e) if e.name().as_ref().ends_with(b"a:t") => in_text = false,
            Event::End(e) if e.name().as_ref().ends_with(b"a:p") => out.push('\n'),
            Event::Text(t) if in_text => {
                if let Ok(txt) = t.decode() {
                    out.push_str(&txt);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

// Legacy binary .doc has no easy pure-Rust parser (needs FIB/piece-table decoding);
// scrape printable UTF-16LE runs from the WordDocument stream as a best-effort fallback.
fn read_doc_text(bytes: &[u8]) -> Result<String, String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut comp = cfb::CompoundFile::open(cursor).map_err(|e| e.to_string())?;
    let mut stream = comp
        .open_stream("WordDocument")
        .map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    stream
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;

    let mut out = String::new();
    let mut run: Vec<u16> = Vec::new();
    let mut i = 0;
    while i + 1 < data.len() {
        let code = u16::from_le_bytes([data[i], data[i + 1]]);
        let printable = (0x20..=0x7E).contains(&code) || (0xA0..=0x2FFF).contains(&code);
        if printable {
            run.push(code);
        } else {
            if run.len() >= 4 {
                if let Ok(s) = String::from_utf16(&run) {
                    out.push_str(s.trim());
                    out.push('\n');
                }
            }
            run.clear();
        }
        i += 2;
    }
    if run.len() >= 4 {
        if let Ok(s) = String::from_utf16(&run) {
            out.push_str(s.trim());
            out.push('\n');
        }
    }
    Ok(out)
}

#[tauri::command]
async fn open_file_dialog(app: tauri::AppHandle) -> Option<String> {
    let file = app
        .dialog()
        .file()
        .add_filter(
            "Documents",
            &[
                "txt", "docx", "doc", "xlsx", "xls", "xlsm", "xlsb", "ods", "pptx", "csv", "pdf",
                "html", "htm", "md", "json", "xml", "yaml", "yml", "ini", "log", "css", "js",
                "ts", "rtf",
            ],
        )
        .add_filter("All Files", &["*"])
        .blocking_pick_file();

    file.map(|f| f.to_string())
}

#[tauri::command]
async fn read_file(path: String) -> Result<FileReadResult, String> {
    let ext = Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let file_type = ext.to_uppercase();

    let size = fs::metadata(&path)
        .map_err(|e| format!("Failed to read file: {e}"))?
        .len();

    let text = match ext.as_str() {
        "docx" => {
            let bytes = fs::read(&path).map_err(|e| format!("Failed to read file: {e}"))?;
            read_docx_text(&bytes).map_err(|e| format!("Failed to read file: {e}"))?
        }
        "doc" => {
            let bytes = fs::read(&path).map_err(|e| format!("Failed to read file: {e}"))?;
            read_doc_text(&bytes).map_err(|e| format!("Failed to read file: {e}"))?
        }
        "pptx" => read_pptx_text(&path).map_err(|e| format!("Failed to read file: {e}"))?,
        "xlsx" | "xls" | "xlsm" | "xlsb" | "ods" => {
            read_xlsx_text(&path).map_err(|e| format!("Failed to read file: {e}"))?
        }
        "pdf" => pdf_extract::extract_text(&path)
            .map_err(|e| format!("Failed to read file: {e}"))?,
        _ => fs::read_to_string(&path)
            .map_err(|_| format!("Unsupported file type: .{ext}"))?,
    };

    Ok(FileReadResult {
        success: true,
        text,
        file_type,
        size,
    })
}

#[tauri::command]
async fn open_link(app: tauri::AppHandle, url: String) -> Result<(), String> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_startup_file(state: State<PendingFile>) -> Option<String> {
    state.0.lock().unwrap().take()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let startup_file = find_file_arg(&args);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(file) = find_file_arg(&argv) {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_focus();
                    let _ = window.emit("open-file-from-os", file);
                }
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(PendingFile(Mutex::new(startup_file)))
        .invoke_handler(tauri::generate_handler![
            open_file_dialog,
            read_file,
            open_link,
            get_startup_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
