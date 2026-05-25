use regex::Regex;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tempfile::TempDir;
use walkdir::WalkDir;

fn cmd(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut c = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    c
}

// ── event types ────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    current: usize,
    total: usize,
    filename: String,
    folder: String,
    output_dir: String,
    folder_total: usize,
    status: String,
    code: Option<String>,
    error: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DownloadEvent {
    filename: String,
    downloaded: u64,
    total: u64,
}

// ── resolved binary paths ──────────────────────────────────────────────────

struct BinPaths {
    pdftoppm: PathBuf,
    pdftotext: PathBuf,
    tesseract: PathBuf,
    tessdata_dir: Option<PathBuf>,
}

fn resolve_bins(app: &AppHandle) -> BinPaths {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .or_else(|| app.path().resource_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let bins = base.join("bins");
        eprintln!("[resolve_bins] base={} bins={}", base.display(), bins.display());
        BinPaths {
            pdftoppm: bins.join("pdftoppm.exe"),
            pdftotext: bins.join("pdftotext.exe"),
            tesseract: bins.join("tesseract.exe"),
            tessdata_dir: Some(bins.join("tessdata")),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        BinPaths {
            pdftoppm: PathBuf::from("pdftoppm"),
            pdftotext: PathBuf::from("pdftotext"),
            tesseract: PathBuf::from("tesseract"),
            tessdata_dir: None,
        }
    }
}

fn bins_check(bins: &BinPaths) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let missing: Vec<String> = [
            ("pdftoppm.exe", &bins.pdftoppm),
            ("pdftotext.exe", &bins.pdftotext),
            ("tesseract.exe", &bins.tesseract),
        ]
        .iter()
        .filter(|(_, p)| !p.exists())
        .map(|(name, p)| format!("{name} (procurado em: {})", p.display()))
        .collect();

        let tessdata_missing = bins
            .tessdata_dir
            .as_ref()
            .map(|d| {
                let f = d.join("eng.traineddata");
                if f.exists() { None } else { Some(format!("eng.traineddata (procurado em: {})", f.display())) }
            })
            .unwrap_or(None);

        let all_missing: Vec<String> = missing.into_iter().chain(tessdata_missing).collect();
        if all_missing.is_empty() {
            Ok(())
        } else {
            Err(format!("Arquivos não encontrados:\n{}", all_missing.join("\n")))
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}

fn bins_available(bins: &BinPaths) -> bool {
    #[cfg(target_os = "windows")]
    {
        bins.pdftoppm.exists()
            && bins.pdftotext.exists()
            && bins.tesseract.exists()
            && bins
                .tessdata_dir
                .as_ref()
                .map(|d| d.join("eng.traineddata").exists())
                .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        command_exists(&bins.pdftoppm.to_string_lossy())
            && command_exists(&bins.pdftotext.to_string_lossy())
            && command_exists(&bins.tesseract.to_string_lossy())
            && tesseract_has_languages(bins, &["eng"])
    }
}

// ── helpers ────────────────────────────────────────────────────────────────

fn emit_progress(app: &AppHandle, event: ProgressEvent) {
    let _ = app.emit("progress", event);
}

fn command_exists(name: &str) -> bool {
    let checker = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    cmd(checker)
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn tesseract_has_languages(bins: &BinPaths, required: &[&str]) -> bool {
    let mut cmd = cmd(&bins.tesseract);
    if let Some(td) = &bins.tessdata_dir {
        cmd.arg("--tessdata-dir").arg(td);
    }
    let output = match cmd.arg("--list-langs").output() {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            eprintln!(
                "[deps] tesseract --list-langs failed: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            );
            return false;
        }
        Err(e) => {
            eprintln!("[deps] tesseract --list-langs spawn error: {e}");
            return false;
        }
    };
    let langs = String::from_utf8_lossy(&output.stdout);
    required
        .iter()
        .all(|lang| langs.lines().any(|line| line.trim() == *lang))
}

fn run_with_timeout(command: &mut Command, timeout: Duration) -> Result<Output, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let start = Instant::now();

    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(_) => return child.wait_with_output().map_err(|e| e.to_string()),
            None if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("tempo limite excedido ({timeout:?})"));
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn first_generated_image(temp_dir: &TempDir) -> Result<PathBuf, String> {
    fs::read_dir(temp_dir.path())
        .map_err(|e| format!("read_dir: {e}"))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|path| {
            path.extension()
                .and_then(|x| x.to_str())
                .map(|x| {
                    x.eq_ignore_ascii_case("jpg")
                        || x.eq_ignore_ascii_case("jpeg")
                        || x.eq_ignore_ascii_case("png")
                })
                .unwrap_or(false)
        })
        .ok_or_else(|| "pdftoppm: sem imagem gerada".to_string())
}

fn pdf_first_page_to_image(
    pdf_path: &PathBuf,
    temp_dir: &TempDir,
    bins: &BinPaths,
) -> Result<PathBuf, String> {
    let image_base = temp_dir.path().join("page").to_string_lossy().to_string();
    let pdf_str = pdf_path.to_string_lossy().to_string();

    eprintln!("[process] pdftoppm start");
    let output = run_with_timeout(
        cmd(&bins.pdftoppm).args([
            "-jpeg",
            "-r",
            "120",
            "-f",
            "1",
            "-l",
            "1",
            &pdf_str,
            &image_base,
        ]),
        Duration::from_secs(60),
    )
    .map_err(|e| format!("pdftoppm: {e}"))?;
    eprintln!("[process] pdftoppm exit={}", output.status);

    if !output.status.success() {
        return Err(format!(
            "pdftoppm: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let image_path = first_generated_image(temp_dir)?;
    eprintln!("[process] image: {}", image_path.display());
    Ok(image_path)
}

fn extract_pdf_text(pdf_path: &PathBuf, bins: &BinPaths) -> Result<String, String> {
    eprintln!("[process] pdftotext start");
    let pdf_str = pdf_path.to_string_lossy().to_string();
    let output = run_with_timeout(
        cmd(&bins.pdftotext).args([&pdf_str, "-"]),
        Duration::from_secs(10),
    )
    .map_err(|e| format!("pdftotext: {e}"))?;
    eprintln!("[process] pdftotext exit={}", output.status);

    if !output.stderr.is_empty() {
        eprintln!(
            "[process] pdftotext stderr: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    if !output.status.success() {
        return Err(format!(
            "pdftotext: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    eprintln!("[process] pdftotext done, {} chars", text.len());
    Ok(text)
}

fn ocr_image(image_path: &PathBuf, bins: &BinPaths) -> Result<String, String> {
    eprintln!("[process] tesseract start");
    let image_str = image_path.to_string_lossy().to_string();

    let mut cmd = cmd(&bins.tesseract);
    cmd.env("OMP_THREAD_LIMIT", "1");
    if let Some(td) = &bins.tessdata_dir {
        cmd.arg("--tessdata-dir").arg(td);
    }
    cmd.args([
        &image_str,
        "stdout",
        "-l",
        "eng",
        "--psm",
        "6",
        "--dpi",
        "120",
        "-c",
        "tessedit_char_whitelist=0123456789",
    ]);

    let output = run_with_timeout(&mut cmd, Duration::from_secs(15))
        .map_err(|e| format!("tesseract: {e}"))?;

    eprintln!("[process] tesseract exit={}", output.status);
    if !output.stderr.is_empty() {
        eprintln!(
            "[process] tesseract stderr: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    if !output.status.success() {
        return Err(format!(
            "tesseract: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    eprintln!("[process] tesseract done, {} chars", text.len());
    Ok(text)
}

fn unique_destination(output_dir: &PathBuf, code: &str) -> PathBuf {
    let first = output_dir.join(format!("{code}.pdf"));
    if !first.exists() {
        return first;
    }

    for suffix in 2usize.. {
        let candidate = output_dir.join(format!("{code}-{suffix}.pdf"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("infinite suffix iterator should always find a path")
}

// ── dependency check ───────────────────────────────────────────────────────

#[tauri::command]
fn check_models(app: AppHandle) -> Result<bool, String> {
    let bins = resolve_bins(&app);
    let ok = bins_available(&bins);
    eprintln!("[check_models] ok={ok}");
    Ok(ok)
}

#[tauri::command]
fn download_models(app: AppHandle) {
    std::thread::spawn(move || {
        let bins = resolve_bins(&app);
        if bins_available(&bins) {
            let _ = app.emit(
                "download-progress",
                DownloadEvent {
                    filename: "Ferramentas de OCR verificadas".to_string(),
                    downloaded: 1,
                    total: 1,
                },
            );
            let _ = app.emit("download-done", ());
        } else {
            #[cfg(target_os = "windows")]
            let msg = "Ferramentas de OCR não encontradas no pacote. Reinstale o aplicativo.";
            #[cfg(not(target_os = "windows"))]
            let msg =
                "Instale poppler-utils, tesseract-ocr e o idioma eng do Tesseract.";
            let _ = app.emit("download-error", msg.to_string());
        }
    });
}

// ── diagnose ───────────────────────────────────────────────────────────────

#[tauri::command]
fn diagnose(pdf_path: String, app: AppHandle) -> Result<Vec<String>, String> {
    let mut log: Vec<String> = vec![];
    let bins = resolve_bins(&app);

    macro_rules! step {
        ($($arg:tt)*) => {{
            let msg = format!($($arg)*);
            eprintln!("[diagnose] {msg}");
            log.push(msg);
        }};
    }

    step!("pdftoppm: {}", bins.pdftoppm.display());
    step!("pdftotext: {}", bins.pdftotext.display());
    step!("tesseract: {}", bins.tesseract.display());
    if let Some(td) = &bins.tessdata_dir {
        step!("tessdata: {}", td.display());
    }

    let re = Regex::new(r"\b\d{5,}\b").unwrap();
    step!("running pdftotext...");
    match extract_pdf_text(&PathBuf::from(&pdf_path), &bins) {
        Ok(text) => {
            let preview: String = text.chars().take(200).collect();
            let found = re.find(&text).map(|m| m.as_str().to_string());
            step!(
                "pdftotext ok, len={}, code={found:?}, preview: {preview:?}",
                text.len()
            );
            if found.is_some() {
                return Ok(log);
            }
        }
        Err(e) => step!("pdftotext error: {e}"),
    }

    let temp_dir = TempDir::new().map_err(|e| e.to_string())?;
    let image_base = temp_dir.path().join("page").to_string_lossy().to_string();
    step!("running pdftoppm on {pdf_path}");
    let o = run_with_timeout(
        cmd(&bins.pdftoppm).args([
            "-jpeg",
            "-r",
            "120",
            "-f",
            "1",
            "-l",
            "1",
            &pdf_path,
            &image_base,
        ]),
        Duration::from_secs(60),
    )
    .map_err(|e| {
        step!("pdftoppm spawn error: {e}");
        e
    })?;
    step!("pdftoppm exit: {}", o.status);
    if !o.stdout.is_empty() {
        step!("pdftoppm stdout: {}", String::from_utf8_lossy(&o.stdout));
    }
    if !o.stderr.is_empty() {
        step!("pdftoppm stderr: {}", String::from_utf8_lossy(&o.stderr));
    }
    if !o.status.success() {
        return Ok(log);
    }

    let image_path = match first_generated_image(&temp_dir) {
        Ok(p) => {
            step!("image: {}", p.display());
            p
        }
        Err(e) => {
            step!("ERROR: {e}");
            return Ok(log);
        }
    };

    step!("running tesseract...");
    match ocr_image(&image_path, &bins) {
        Ok(text) => {
            let preview: String = text.chars().take(200).collect();
            let found = re.find(&text).map(|m| m.as_str().to_string());
            step!(
                "tesseract ok, len={}, code={found:?}, preview: {preview:?}",
                text.len()
            );
        }
        Err(e) => step!("tesseract error: {e}"),
    }

    Ok(log)
}

// ── process ────────────────────────────────────────────────────────────────

// Returns the original filename if the PDF failed to process.
fn process_pdf(
    pdf_path: &PathBuf,
    output_dir: &PathBuf,
    re: &Regex,
    app: &AppHandle,
    bins: &BinPaths,
    current: usize,
    total: usize,
    folder_total: usize,
) -> Option<String> {
    let filename = pdf_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let folder = pdf_path
        .parent()
        .and_then(|p| p.file_name())
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let output_dir_str = output_dir.to_string_lossy().to_string();
    eprintln!("[process] {current}/{total} {filename}");

    emit_progress(
        app,
        ProgressEvent {
            current: current - 1,
            total,
            filename: filename.clone(),
            folder: folder.clone(),
            output_dir: output_dir_str.clone(),
            folder_total,
            status: "processing".to_string(),
            code: None,
            error: None,
        },
    );

    let result = (|| -> Result<String, String> {
        match extract_pdf_text(pdf_path, bins) {
            Ok(text) => {
                if let Some(code) = re.find(&text).map(|m| m.as_str().to_string()) {
                    eprintln!("[process] code from pdftotext: {code}");
                    return Ok(code);
                }
                eprintln!("[process] pdftotext found no code, falling back to OCR");
            }
            Err(e) => {
                eprintln!("[process] pdftotext failed, falling back to OCR: {e}");
            }
        }

        let temp_dir = TempDir::new().map_err(|e| format!("tempdir: {e}"))?;
        let image_path = pdf_first_page_to_image(pdf_path, &temp_dir, bins)?;
        let text = ocr_image(&image_path, bins)?;

        re.find(&text)
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| "Nenhum código encontrado".to_string())
    })();

    eprintln!(
        "[process] result: {:?}",
        result.as_ref().map(|c| c.as_str()).unwrap_or("ERR")
    );

    match result {
        Ok(code) => {
            let dest = unique_destination(output_dir, &code);
            eprintln!("[process] copy to: {}", dest.display());
            match fs::copy(pdf_path, &dest) {
                Ok(_) => {
                    emit_progress(
                        app,
                        ProgressEvent {
                            current,
                            total,
                            filename,
                            folder: folder.clone(),
                            output_dir: output_dir_str.clone(),
                            folder_total,
                            status: "done".to_string(),
                            code: Some(code),
                            error: None,
                        },
                    );
                    None
                }
                Err(e) => {
                    emit_progress(
                        app,
                        ProgressEvent {
                            current,
                            total,
                            filename: filename.clone(),
                            folder: folder.clone(),
                            output_dir: output_dir_str.clone(),
                            folder_total,
                            status: "error".to_string(),
                            code: None,
                            error: Some(format!("copy: {e}")),
                        },
                    );
                    Some(filename)
                }
            }
        }
        Err(e) => {
            emit_progress(
                app,
                ProgressEvent {
                    current,
                    total,
                    filename: filename.clone(),
                    folder,
                    output_dir: output_dir_str,
                    folder_total,
                    status: "error".to_string(),
                    code: None,
                    error: Some(e),
                },
            );
            Some(filename)
        }
    }
}

#[tauri::command]
fn process_folders(folders: Vec<String>, app: AppHandle) -> Result<usize, String> {
    let bins = resolve_bins(&app);
    if let Err(msg) = bins_check(&bins) {
        return Err(msg);
    }

    let mut tasks: Vec<(PathBuf, PathBuf)> = vec![];

    for folder_str in &folders {
        let folder = PathBuf::from(folder_str);
        eprintln!("[process_folders] scanning: {}", folder.display());

        if folder.is_file() {
            let is_pdf = folder
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase() == "pdf")
                .unwrap_or(false);
            if is_pdf {
                let parent = folder.parent().unwrap_or(&folder);
                let stem = parent
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let output_dir = parent
                    .parent()
                    .unwrap_or(parent)
                    .join(format!("{stem} (renomeado)"));
                fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
                tasks.push((folder, output_dir));
            }
            continue;
        }

        if !folder.is_dir() {
            eprintln!("[process_folders] not a dir, skipping");
            continue;
        }

        let folder_name = folder
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let parent = folder.parent().unwrap_or(&folder);
        let output_dir = parent.join(format!("{folder_name} (renomeado)"));
        fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
        eprintln!("[process_folders] output_dir={}", output_dir.display());

        for entry in WalkDir::new(&folder).min_depth(1).max_depth(1) {
            if let Ok(entry) = entry {
                let path = entry.path().to_path_buf();
                let is_pdf = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase() == "pdf")
                    .unwrap_or(false);
                if is_pdf {
                    tasks.push((path, output_dir.clone()));
                }
            }
        }
    }

    let total = tasks.len();
    eprintln!("[process_folders] total PDFs: {total}");

    if total == 0 {
        return Ok(0);
    }

    let mut folder_totals: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (_, output_dir) in &tasks {
        *folder_totals
            .entry(output_dir.to_string_lossy().to_string())
            .or_insert(0) += 1;
    }

    std::thread::spawn(move || {
        let re = Regex::new(r"\b\d{5,}\b").unwrap();
        eprintln!("[thread] starting loop");

        let mut errors_by_dir: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for (idx, (pdf_path, output_dir)) in tasks.iter().enumerate() {
            let ft = *folder_totals
                .get(&output_dir.to_string_lossy().to_string())
                .unwrap_or(&1);
            if let Some(failed) =
                process_pdf(pdf_path, output_dir, &re, &app, &bins, idx + 1, total, ft)
            {
                errors_by_dir
                    .entry(output_dir.to_string_lossy().to_string())
                    .or_default()
                    .push(failed);
            }
        }

        for (dir, failed_files) in &errors_by_dir {
            let path = PathBuf::from(dir).join("erros.txt");
            let content = failed_files.join("\n");
            if let Err(e) = fs::write(&path, content) {
                eprintln!("[thread] failed to write erros.txt: {e}");
            } else {
                eprintln!(
                    "[thread] wrote erros.txt with {} entries",
                    failed_files.len()
                );
            }
        }

        eprintln!("[thread] done");
    });

    Ok(total)
}

#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let open_cmd = ("explorer", vec![path.clone()]);
    #[cfg(target_os = "macos")]
    let open_cmd = ("open", vec![path.clone()]);
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let open_cmd = ("xdg-open", vec![path.clone()]);

    cmd(open_cmd.0)
        .args(open_cmd.1)
        .spawn()
        .map_err(|e| format!("open_folder: {e}"))?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            check_models,
            download_models,
            diagnose,
            process_folders,
            open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
