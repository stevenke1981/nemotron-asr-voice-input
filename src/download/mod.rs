/// Model auto-download module.
/// Downloads Nemotron ONNX model packages from sherpa-onnx GitHub releases.
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

// ── Model package ─────────────────────────────────────────────────────

/// Sherpa-onnx Nemotron multilingual model package name.
/// The model supports 40+ languages with automatic language detection.
/// Chunk size: 560ms (balances latency vs accuracy).
const MODEL_PACKAGE: &str =
    "sherpa-onnx-nemotron-3.5-asr-streaming-0.6b-560ms-int8-2026-06-11";

/// Download URL for the model tarball (tar.bz2).
const MODEL_URL: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemotron-3.5-asr-streaming-0.6b-560ms-int8-2026-06-11.tar.bz2";

/// Silero VAD model (downloaded separately).
const VAD_URL: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx";

// ── Public interface ──────────────────────────────────────────────────

/// Check whether all required model files exist in the given directory.
pub fn check_model_files(model_dir: &Path) -> Result<bool> {
    let required = [
        "encoder.int8.onnx",
        "decoder.int8.onnx",
        "joiner.int8.onnx",
        "tokens.txt",
    ];
    let optional = ["silero_vad.onnx"];

    let all_exist = required.iter().all(|f| model_dir.join(f).exists());

    if !all_exist {
        let missing: Vec<_> = required
            .iter()
            .filter(|f| !model_dir.join(f).exists())
            .collect();
        info!("Missing model files in {:?}: {:?}", model_dir, missing);
        return Ok(false);
    }

    // Check optional files
    for f in &optional {
        if !model_dir.join(f).exists() {
            info!("Optional model file missing: {} (VAD will be disabled)", f);
        }
    }

    info!("All required model files present in {:?}", model_dir);
    Ok(true)
}

/// Download all model files to the given directory.
/// Creates the directory if it doesn't exist.
/// If `on_progress` is provided, it is called with (phase_name, current_bytes, total_bytes_estimate)
/// so the GUI can display download progress.
pub fn download_models(model_dir: &Path, on_progress: Option<&dyn Fn(&str, u64, u64)>) -> Result<()> {
    std::fs::create_dir_all(model_dir)
        .with_context(|| format!("Failed to create model directory: {}", model_dir.display()))?;

    if let Some(cb) = on_progress {
        cb("checking", 0, 0);
    }
    info!("Downloading Nemotron ASR models to {:?}...", model_dir);
    info!("Source: {}", MODEL_URL);

    let downloaded_bytes = Arc::new(AtomicUsize::new(0));

    // Step 1: Download the model tarball
    if let Some(cb) = on_progress {
        cb("downloading_tarball", 0, 0);
    }
    let tarball_path = download_tarball(model_dir, &downloaded_bytes)?;

    // Step 2: Extract the tarball
    if let Some(cb) = on_progress {
        cb("extracting", 0, 0);
    }
    let total = downloaded_bytes.load(Ordering::SeqCst);
    info!(
        "Extracting model package ({:.2} MB)...",
        total as f64 / 1_048_576.0
    );
    extract_tarball(&tarball_path, model_dir)?;

    // Clean up tarball
    if tarball_path.exists() {
        std::fs::remove_file(&tarball_path)
            .unwrap_or_else(|e| warn!("Failed to remove temp tarball: {}", e));
    }

    // Step 3: Download VAD model
    if let Some(cb) = on_progress {
        cb("downloading_vad", 0, 0);
    }
    download_vad(model_dir, &downloaded_bytes)?;

    // Step 4: Final verification
    if let Some(cb) = on_progress {
        cb("verifying", 100, 100);
    }
    let total = downloaded_bytes.load(Ordering::SeqCst) as u64;
    info!("Download complete ({:.2} MB to {:?})", total as f64 / 1_048_576.0, model_dir);

    match check_model_files(model_dir) {
        Ok(true) => info!("All model files verified successfully."),
        Ok(false) => warn!(
            "Some model files are still missing. Check {:?}",
            model_dir
        ),
        Err(e) => warn!("Model verification error: {}", e),
    }

    Ok(())
}

/// Print the status of model files in the given directory.
pub fn print_model_status(model_dir: &Path) {
    println!("Model directory: {:?}", model_dir);
    println!();
    println!("{:<45} {:>12} {:>12}", "File", "Size", "Status");
    println!("{:-<45} {:-<12} {:-<12}", "", "", "");

    let all_files = [
        "encoder.int8.onnx",
        "decoder.int8.onnx",
        "joiner.int8.onnx",
        "tokens.txt",
        "silero_vad.onnx",
    ];

    for fname in &all_files {
        let path = model_dir.join(fname);
        let (size_str, status) = if path.exists() {
            match std::fs::metadata(&path) {
                Ok(meta) => {
                    let size = meta.len();
                    let size_str = if size > 1_048_576 {
                        format!("{:.1} MB", size as f64 / 1_048_576.0)
                    } else if size > 1024 {
                        format!("{:.1} KB", size as f64 / 1024.0)
                    } else {
                        format!("{} B", size)
                    };
                    let status = if size > 1000 { "OK" } else { "EMPTY" };
                    (size_str, status)
                }
                Err(_) => ("???".into(), "ERROR"),
            }
        } else {
            ("---".into(), "MISSING")
        };
        println!("{:<45} {:>12} {:>12}", fname, size_str, status);
    }

    println!();
    let all_ok = check_model_files(model_dir).unwrap_or(false);
    if all_ok {
        println!("✓ All required model files present.");
    } else {
        println!("✗ Some model files missing. Run with --download-models to fetch them.");
    }
}

// ── Internal helpers ──────────────────────────────────────────────────

/// Download the model tarball to a temp file in the model directory.
fn download_tarball(
    model_dir: &Path,
    downloaded_bytes: &Arc<AtomicUsize>,
) -> Result<PathBuf> {
    let dest_path = model_dir.join(format!("{}.tar.bz2", MODEL_PACKAGE));

    // Skip if already downloaded with reasonable size
    if dest_path.exists() {
        let meta = std::fs::metadata(&dest_path)?;
        if meta.len() > 1_000_000 {
            info!("  [SKIP] Tarball already exists ({:.1} MB)", meta.len() as f64 / 1_048_576.0);
            downloaded_bytes.fetch_add(meta.len() as usize, Ordering::SeqCst);
            return Ok(dest_path);
        }
    }

    info!("  Downloading model package (tar.bz2)...");

    let response = ureq::get(MODEL_URL)
        .call()
        .with_context(|| "Failed to start download of model package")?;

    let mut body: Vec<u8> = Vec::new();
    let mut reader = response.into_reader();
    std::io::Read::by_ref(&mut reader)
        .read_to_end(&mut body)
        .context("Failed to read model package response body")?;

    if body.is_empty() {
        anyhow::bail!("Downloaded empty model package");
    }

    std::fs::write(&dest_path, &body)
        .with_context(|| format!("Failed to write {}", dest_path.display()))?;

    let actual_size = body.len();
    downloaded_bytes.fetch_add(actual_size, Ordering::SeqCst);

    info!(
        "  [OK] Downloaded {:.2} MB",
        actual_size as f64 / 1_048_576.0
    );

    Ok(dest_path)
}

/// Extract a tar.bz2 archive into the destination directory.
/// Handles tar archives that have a single top-level directory
/// by moving files from that directory up to the destination.
fn extract_tarball(tarball: &Path, dest: &Path) -> Result<()> {
    use bzip2::read::BzDecoder;
    use tar::Archive;

    // Extract to a temp staging directory
    let staging = dest.join(".tmp_extract");
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    std::fs::create_dir_all(&staging)?;

    let file = std::fs::File::open(tarball)
        .with_context(|| format!("Failed to open tarball: {}", tarball.display()))?;
    let decoder = BzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(&staging)
        .context("Failed to extract tarball")?;

    // Find extracted contents — they may be inside a top-level directory
    let entries: Vec<_> = std::fs::read_dir(&staging)
        .context("Failed to read staging directory")?
        .filter_map(|e| e.ok())
        .collect();

    let source_dir: PathBuf = if entries.len() == 1 && entries[0].file_type().map(|t| t.is_dir()).unwrap_or(false) {
        // Single top-level directory — use its contents
        entries[0].path()
    } else {
        // No top-level directory — use staging directly
        staging.clone()
    };

    // Move files to destination
    for entry in std::fs::read_dir(&source_dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let dest_path = dest.join(&file_name);

        if entry.file_type()?.is_file() {
            // Remove existing file if present
            if dest_path.exists() {
                std::fs::remove_file(&dest_path)?;
            }
            std::fs::rename(&entry.path(), &dest_path)
                .with_context(|| format!("Failed to move {} to {}", file_name.to_string_lossy(), dest.display()))?;
            info!("  Extracted: {}", file_name.to_string_lossy());
        }
    }

    // Clean up staging
    std::fs::remove_dir_all(&staging)?;

    info!("  Extraction complete.");
    Ok(())
}

/// Download the Silero VAD model separately.
fn download_vad(model_dir: &Path, downloaded_bytes: &Arc<AtomicUsize>) -> Result<()> {
    let dest_path = model_dir.join("silero_vad.onnx");

    if dest_path.exists() {
        let meta = std::fs::metadata(&dest_path)?;
        if meta.len() > 1000 {
            info!("  [SKIP] silero_vad.onnx already exists ({:.1} KB)", meta.len() as f64 / 1024.0);
            downloaded_bytes.fetch_add(meta.len() as usize, Ordering::SeqCst);
            return Ok(());
        }
    }

    info!("  Downloading silero_vad.onnx...");

    let response = ureq::get(VAD_URL)
        .call()
        .context("Failed to start download of silero_vad.onnx")?;

    let mut body: Vec<u8> = Vec::new();
    let mut reader = response.into_reader();
    std::io::Read::by_ref(&mut reader)
        .read_to_end(&mut body)
        .context("Failed to read silero_vad.onnx response body")?;

    if body.is_empty() {
        anyhow::bail!("Downloaded empty silero_vad.onnx");
    }

    std::fs::write(&dest_path, &body)
        .with_context(|| format!("Failed to write {}", dest_path.display()))?;

    let actual_size = body.len();
    downloaded_bytes.fetch_add(actual_size, Ordering::SeqCst);

    info!(
        "  [OK] silero_vad.onnx ({:.2} MB)",
        actual_size as f64 / 1_048_576.0
    );

    Ok(())
}
