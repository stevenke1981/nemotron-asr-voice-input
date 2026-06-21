/// Build script for nemotron-voice-input.
/// Checks for model files and prints helpful messages.

use std::path::Path;

fn main() {
    let model_dir = Path::new("models");

    if !model_dir.exists() {
        println!("cargo:warning=Models directory not found at: {:?}", model_dir);
        println!("cargo:warning=Please download models from:");
        println!("cargo:warning=  https://huggingface.co/onnx-community/nemotron-3.5-asr-streaming-0.6b-onnx-int4");
        println!("cargo:warning=Place them in the 'models/' directory.");
        println!("cargo:rerun-if-changed=models");
        return;
    }

    let required_files = [
        "encoder.onnx",
        "decoder.onnx",
        "joint.onnx",
        "silero_vad.onnx",
        "tokens.txt",
        "tokenizer.json",
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
        println!("cargo:warning=Some model files are missing. Download from:");
        println!("cargo:warning=  https://huggingface.co/onnx-community/nemotron-3.5-asr-streaming-0.6b-onnx-int4");
    }
}
