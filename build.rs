/// Build script for nemotron-voice-input.
/// Checks for model files and prints helpful messages.

use std::path::Path;

fn main() {
    let model_dir = Path::new("models");

    if !model_dir.exists() {
        println!("cargo:warning=Models directory not found at: {:?}", model_dir);
        println!("cargo:warning=Models will be auto-downloaded on first run.");
        println!("cargo:rerun-if-changed=models");
        return;
    }

    let required_files = [
        "encoder.int8.onnx",
        "decoder.int8.onnx",
        "joiner.int8.onnx",
        "tokens.txt",
    ];

    let mut all_ok = true;
    for file in &required_files {
        let path = model_dir.join(file);
        if !path.exists() {
            println!("cargo:warning=Missing model file: {}", file);
            all_ok = false;
        }
    }

    if all_ok {
        println!("cargo:info=All model files found in models/ directory.");
    } else {
        println!("cargo:warning=Some model files are missing. Run with --download-models.");
    }
}
