/// Model auto-download module.
/// Downloads Nemotron ONNX model files from HuggingFace Hub.
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

/// Model file entry: name on HuggingFace and expected size (bytes).
struct ModelFile {
    filename: &'static str,
    description: &'static str,
    /// Expected size in bytes (0 = unknown)
    expected_size: u64,
}

/// List of model files to download from HuggingFace.
/// Repository: onnx-community/nemotron-3.5-asr-streaming-0.6b-onnx-int4
const MODEL_FILES: &[ModelFile] = &[
    ModelFile {
        filename: "encoder.onnx",
        description: "Encoder model (258 MB)",
        expected_size: 258_000_000,
    },
    ModelFile {
        filename: "decoder.onnx",
        description: "Decoder model (82 MB)",
        expected_size: 82_000_000,
    },
    ModelFile {
        filename: "joint.onnx",
        description: "Joint network (2.5 MB)",
        expected_size: 2_500_000,
    },
    ModelFile {
        filename: "silero_vad.onnx",
        description: "Silero VAD model (1.1 MB)",
        expected_size: 1_100_000,
    },
    ModelFile {
        filename: "vocab.txt:tokens.txt",
        description: "Token vocabulary (64 KB)",
        expected_size: 64_000,
    },
    ModelFile {
        filename: "tokenizer.json",
        description: "Tokenizer config (1.8 MB)",
        expected_size: 1_800_000,
    },
];

const HF_BASE_URL: &str =
    "https://huggingface.co/onnx-community/nemotron-3.5-asr-streaming-0.6b-onnx-int4/resolve/main";

/// Check whether all required model files exist in the given directory.
pub fn check_model_files(model_dir: &Path) -> Result<bool> {
    let required = ["encoder.onnx", "decoder.onnx", "joint.onnx", "tokens.txt"];
    let all_exist = required.iter().all(|f| model_dir.join(f).exists());

    if !all_exist {
        let missing: Vec<_> = required
            .iter()
            .filter(|f| !model_dir.join(f).exists())
            .collect();
        info!(
            "Missing model files in {:?}: {:?}",
            model_dir, missing
        );
        return Ok(false);
    }

    info!("All required model files present in {:?}", model_dir);
    Ok(true)
}

/// Download a single file from HuggingFace with progress display.
/// If `filename` contains ":", the part before ":" is the remote name and
/// the part after is the local name (e.g. "vocab.txt:tokens.txt").
fn download_file(
    filename: &str,
    dest_dir: &Path,
    downloaded_bytes: &Arc<AtomicUsize>,
    _total_bytes: u64,
) -> Result<()> {
    let (remote_name, local_name) = if let Some(pos) = filename.find(':') {
        (&filename[..pos], &filename[pos + 1..])
    } else {
        (filename, filename)
    };
    let url = format!("{}/{}", HF_BASE_URL, remote_name);
    let dest_path = dest_dir.join(local_name);

    // Skip if file exists and has reasonable size
    if dest_path.exists() {
        let meta = std::fs::metadata(&dest_path)?;
        if meta.len() > 1000 {
            // Trust existing files > 1 KB
            info!("  [SKIP] {} already exists ({})", filename, meta.len());
            downloaded_bytes.fetch_add(meta.len() as usize, Ordering::SeqCst);
            return Ok(());
        }
    }

    info!("  Downloading {}...", filename);

    let response = ureq::get(&url)
        .call()
        .with_context(|| format!("Failed to start download of {}", filename))?;

    // Get content length
    let content_length: u64 = response
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    // Read body
    let mut body: Vec<u8> = Vec::new();
    let mut reader = response.into_reader();
    std::io::Read::by_ref(&mut reader)
        .read_to_end(&mut body)
        .with_context(|| format!("Failed to read response body for {}", filename))?;

    if body.is_empty() {
        anyhow::bail!("Downloaded empty file: {}", filename);
    }

    // Write to disk
    std::fs::write(&dest_path, &body)
        .with_context(|| format!("Failed to write {}", dest_path.display()))?;

    let actual_size = body.len() as u64;
    downloaded_bytes.fetch_add(body.len(), Ordering::SeqCst);

    if content_length > 0 && actual_size < content_length {
        warn!(
            "  {}: expected {} bytes, got {} bytes",
            filename, content_length, actual_size
        );
    }

    info!(
        "  [OK] {} ({:.2} MB)",
        filename,
        actual_size as f64 / 1_048_576.0
    );

    Ok(())
}

/// Download all model files from HuggingFace to the given directory.
/// Creates the directory if it doesn't exist.
pub fn download_models(model_dir: &Path) -> Result<()> {
    // Create directory
    std::fs::create_dir_all(model_dir)
        .with_context(|| format!("Failed to create model directory: {}", model_dir.display()))?;

    info!("Downloading Nemotron ASR models to {:?}...", model_dir);
    info!("Source: {}", HF_BASE_URL);

    let total_estimate: u64 = MODEL_FILES.iter().map(|f| f.expected_size).sum();
    info!(
        "Total estimated size: {:.2} MB ({} files)",
        total_estimate as f64 / 1_048_576.0,
        MODEL_FILES.len()
    );

    let downloaded_bytes = Arc::new(AtomicUsize::new(0));

    // Download all files
    for file in MODEL_FILES {
        info!("  {} ({})", file.filename, file.description);
        if let Err(e) = download_file(file.filename, model_dir, &downloaded_bytes, total_estimate) {
            // Don't abort on individual file failure - try remaining files
            warn!("Failed to download {}: {}", file.filename, e);
        }
    }

    // Verify downloads
    let total = downloaded_bytes.load(Ordering::SeqCst) as u64;
    info!(
        "Download complete: {:.2} MB downloaded to {:?}",
        total as f64 / 1_048_576.0,
        model_dir
    );

    // Final verification
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
    println!("{:<30} {:>12} {:>12}", "File", "Size", "Status");
    println!("{:-<30} {:-<12} {:-<12}", "", "", "");

    for file in MODEL_FILES {
        let path = model_dir.join(file.filename);
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
        println!("{:<30} {:>12} {:>12}", file.filename, size_str, status);
    }

    println!();
    let all_ok = check_model_files(model_dir).unwrap_or(false);
    if all_ok {
        println!("✓ All required model files present.");
    } else {
        println!("✗ Some model files missing. Run with --download-models to fetch them.");
    }
}
