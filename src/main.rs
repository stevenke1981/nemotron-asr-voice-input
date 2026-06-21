mod audio;
mod asr;
mod config;
mod hotkey;
mod injector;
mod ui;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

use audio::AudioCapture;
use config::AppConfig;
use hotkey::register::HotkeyAction;
use hotkey::HotkeyManager;
use injector::{CompositeInjector, InjectStrategy, TextInjector};
use asr::{AsrConfig, AsrEngine, TranscriptResult};

/// Nemotron ASR Voice Input - Real-time speech recognition and text injection.
#[derive(Parser, Debug)]
#[command(name = "nemotron-voice-input", version = "0.1.0", about = "Real-time ASR voice input using Nemotron model")]
struct Cli {
    /// Path to config TOML file
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,

    /// Model directory path (overrides config)
    #[arg(short, long)]
    model_dir: Option<PathBuf>,

    /// Language code (overrides config): en, zh, ja, de, fr, es, ko, auto, etc.
    #[arg(short, long)]
    language: Option<String>,

    /// Execution provider: cpu or cuda
    #[arg(short, long)]
    provider: Option<String>,

    /// Dump audio to WAV file for debugging (implies --file with path)
    #[arg(long)]
    dump_audio: Option<PathBuf>,

    /// Transcribe a WAV file and exit
    #[arg(long)]
    file: Option<PathBuf>,

    /// List available audio input devices and exit
    #[arg(long)]
    list_devices: bool,

    /// Injection strategy: auto, sendinput, uiautomation, clipboard
    #[arg(long)]
    inject: Option<String>,

    /// Number of ASR threads
    #[arg(long)]
    num_threads: Option<u32>,
}

/// Application state shared between threads.
struct AppState {
    is_recording: AtomicBool,
    is_running: AtomicBool,
}

impl AppState {
    fn new() -> Self {
        Self {
            is_recording: AtomicBool::new(false),
            is_running: AtomicBool::new(true),
        }
    }
}

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_target(true)
        .init();

    info!("Nemotron Voice Input v{}", env!("CARGO_PKG_VERSION"));

    // Parse CLI arguments
    let cli = Cli::parse();

    // Handle --list-devices
    if cli.list_devices {
        let devices = AudioCapture::list_devices().context("Failed to list audio devices")?;
        println!("Available audio input devices:");
        for (i, name) in devices.iter().enumerate() {
            println!("  {}. {}", i + 1, name);
        }
        return Ok(());
    }

    // Load configuration
    let mut app_config = AppConfig::load(&cli.config)
        .context("Failed to load configuration")?;

    // Override config with CLI arguments
    if let Some(model_dir) = cli.model_dir {
        app_config.model_dir = model_dir;
    }
    if let Some(language) = cli.language {
        app_config.language.language = language;
    }
    if let Some(provider) = cli.provider {
        app_config.asr.provider = provider;
    }
    if let Some(strategy) = cli.inject {
        app_config.injector.strategy = strategy;
    }
    if let Some(threads) = cli.num_threads {
        app_config.asr.num_threads = threads;
    }

    // Build ASR config
    let asr_config = AsrConfig {
        model_dir: app_config.model_dir.clone(),
        provider: app_config.asr.provider.clone(),
        num_threads: app_config.asr.num_threads,
        chunk_size_ms: app_config.audio.chunk_size_ms,
        use_vad: app_config.asr.use_vad,
        language: app_config.language.language.clone(),
        decoding_method: app_config.asr.decoding_method.clone(),
        max_active_paths: app_config.asr.max_active_paths,
        sample_rate: app_config.audio.sample_rate,
    };

    // Handle --file mode (batch transcription)
    if let Some(file_path) = cli.file {
        return run_batch_transcription(&file_path, &asr_config);
    }

    // Handle --dump-audio mode
    if let Some(dump_path) = cli.dump_audio {
        return run_audio_dump(&dump_path, &app_config);
    }

    // === Interactive mode ===
    info!("Starting interactive mode");

    // Initialize state
    let state = Arc::new(AppState::new());

    // Initialize audio capture
    let mut audio_capture = AudioCapture::new(
        &app_config.audio.device_name,
        app_config.audio.sample_rate,
        app_config.audio.channels,
        app_config.audio.ringbuf_capacity,
    )?;

    // Initialize text injector
    let inject_strategy = InjectStrategy::from_string(&app_config.injector.strategy);
    let mut injector = CompositeInjector::with_strategy(inject_strategy);

    // Initialize hotkey manager
    let mut hotkey_manager = HotkeyManager::new();
    register_hotkey(&mut hotkey_manager, HotkeyAction::ToggleRecording, app_config.hotkey.toggle_modifiers, app_config.hotkey.toggle_vk);
    register_hotkey(&mut hotkey_manager, HotkeyAction::CycleLanguage, app_config.hotkey.lang_modifiers, app_config.hotkey.lang_vk);
    register_hotkey(&mut hotkey_manager, HotkeyAction::Flush, app_config.hotkey.flush_modifiers, app_config.hotkey.flush_vk);
    info!("Hotkeys registered");

    // Channels for communication between threads
    let (transcript_tx, transcript_rx) = crossbeam::channel::bounded::<TranscriptResult>(64);

    // Shared state for audio thread
    let ringbuf = audio_capture.ringbuf().clone();
    let _state_arc = state.clone();
    let asr_config_clone = asr_config.clone();

    // Audio capture processing thread
    let audio_state = state.clone();
    let audio_tx = transcript_tx.clone();
    let audio_ringbuf = ringbuf.clone();
    let _audio_handle = std::thread::Builder::new()
        .name("audio-processor".into())
        .spawn(move || {
            let mut engine = match asr::create_asr_engine(&asr_config_clone) {
                Ok(e) => e,
                Err(e) => {
                    error!("Failed to create ASR engine in audio thread: {}", e);
                    return;
                }
            };

            let chunk_samples = asr_config_clone.chunk_samples();
            let mut temp_buf = vec![0.0f32; chunk_samples];
            let mut last_text = String::new();

            while audio_state.is_running.load(Ordering::SeqCst) {
                if !audio_state.is_recording.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }

                // Try to read audio data from ring buffer
                let available = audio_ringbuf.len();
                if available < chunk_samples {
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }

                // Read a chunk
                let to_read = temp_buf.len().min(available);
                let read = audio_ringbuf.pop_slice(&mut temp_buf[..to_read]);
                if read == 0 {
                    continue;
                }

                // Feed to ASR engine
                if let Err(e) = engine.feed_audio(&temp_buf[..read]) {
                    tracing::debug!("ASR feed error: {}", e);
                    continue;
                }

                // Get transcript
                match engine.get_transcript() {
                    Ok(result) => {
                        if !result.text.is_empty() && result.text != last_text {
                            last_text = result.text.clone();
                            if audio_tx.send(result).is_err() {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!("ASR transcript error: {}", e);
                    }
                }
            }

            info!("Audio processing thread exiting");
        })
        .context("Failed to spawn audio processing thread")?;

    // Main loop
    info!(
        "Ready. Press Ctrl+Alt+R to start/stop recording. Language: {}",
        asr_config.language
    );

    // Initialize ASR engine (main thread)
    let mut asr_engine = asr::create_asr_engine(&asr_config)
        .context("Failed to initialize ASR engine")?;

    // Windows message loop
    let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();

    unsafe {
        while state.is_running.load(Ordering::SeqCst) {
            // Peek messages (non-blocking)
            let has_message = windows::Win32::UI::WindowsAndMessaging::PeekMessageA(
                &mut msg,
                None,
                0,
                0,
                windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
            );

            if has_message.as_bool() {
                if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_QUIT {
                    state.is_running.store(false, Ordering::SeqCst);
                    break;
                }

                // Handle hotkey
                if let Some(action) = hotkey_manager.process_message(&msg) {
                    handle_hotkey_action(
                        action,
                        &state,
                        &mut audio_capture,
                        &mut asr_engine,
                        &app_config,
                    );
                }

                let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                windows::Win32::UI::WindowsAndMessaging::DispatchMessageA(&msg);
            } else {
                // Check for transcript results
                let recv_count = transcript_rx.len();
                if recv_count > 0 {
                    for _ in 0..recv_count {
                        if let Ok(result) = transcript_rx.try_recv() {
                            if !result.text.is_empty() && state.is_recording.load(Ordering::SeqCst) {
                                info!("Transcript: {}", result.text);
                                if result.is_final {
                                    if let Err(e) = injector.inject_text(&result.text) {
                                        error!("Text injection failed: {}", e);
                                    } else {
                                        info!("Injected: {}", result.text);
                                    }
                                }
                            }
                        }
                    }
                }

                // Small sleep to prevent busy-waiting
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    // Cleanup
    info!("Shutting down...");
    state.is_running.store(false, Ordering::SeqCst);
    let _ = audio_capture.stop();

    info!("Nemotron Voice Input stopped.");
    Ok(())
}

/// Register a hotkey with error handling.
fn register_hotkey(manager: &mut HotkeyManager, action: HotkeyAction, modifiers: u32, vk: u32) {
    if let Err(e) = manager.register(action, modifiers, vk) {
        error!("Failed to register hotkey: {}", e);
    }
}

/// Handle a hotkey action.
fn handle_hotkey_action(
    action: HotkeyAction,
    state: &AppState,
    audio_capture: &mut AudioCapture,
    asr_engine: &mut Box<dyn AsrEngine>,
    app_config: &AppConfig,
) {
    match action {
        HotkeyAction::ToggleRecording => {
            if state.is_recording.load(Ordering::SeqCst) {
                stop_recording(state, audio_capture);
            } else {
                start_recording(state, audio_capture, asr_engine);
            }
        }
        HotkeyAction::CycleLanguage => {
            cycle_language(asr_engine, app_config);
        }
        HotkeyAction::Flush => {
            info!("Flush triggered");
            let _ = asr_engine.reset();
        }
    }
}

/// Start recording.
fn start_recording(
    state: &AppState,
    audio_capture: &mut AudioCapture,
    asr_engine: &mut Box<dyn AsrEngine>,
) {
    // Clear ring buffer and reset ASR
    audio_capture.clear_ringbuf();
    let _ = asr_engine.reset();

    if let Err(e) = audio_capture.start() {
        error!("Failed to start recording: {}", e);
        return;
    }
    state.is_recording.store(true, Ordering::SeqCst);
    info!("Recording started - speak now");
}

/// Stop recording.
fn stop_recording(state: &AppState, audio_capture: &mut AudioCapture) {
    state.is_recording.store(false, Ordering::SeqCst);
    if let Err(e) = audio_capture.stop() {
        error!("Failed to stop recording: {}", e);
    }
    info!("Recording stopped");
}

/// Cycle through configured languages.
fn cycle_language(
    asr_engine: &mut Box<dyn AsrEngine>,
    app_config: &AppConfig,
) {
    let langs = &app_config.language.cycle_order;
    if langs.is_empty() {
        return;
    }

    let current = &app_config.language.language;
    let pos = langs.iter().position(|l| l == current).unwrap_or(0);
    let next_pos = (pos + 1) % langs.len();
    let next_lang = &langs[next_pos];

    if let Err(e) = asr_engine.set_language(next_lang) {
        error!("Failed to set language to {}: {}", next_lang, e);
    } else {
        info!("Language switched: {} -> {}", current, next_lang);
    }
}

/// Run batch transcription on a WAV file.
fn run_batch_transcription(
    file_path: &PathBuf,
    asr_config: &AsrConfig,
) -> Result<()> {
    info!("Transcribing file: {:?}", file_path);

    // Initialize ASR engine
    let mut engine = asr::create_asr_engine(asr_config)?;

    // Read WAV file
    let wav_data = std::fs::read(file_path)
        .context("Failed to read WAV file")?;

    // Parse WAV header (simplified - assumes standard 16-bit PCM)
    let sample_rate = asr_config.sample_rate;
    let samples: Vec<f32> = if wav_data.len() > 44 {
        // Skip WAV header and convert to f32
        let data_start = 44; // Simplistic: assumes standard PCM WAV header
        let data = &wav_data[data_start..];
        data.chunks(2)
            .filter_map(|c| {
                if c.len() >= 2 {
                    Some(i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
                } else {
                    None
                }
            })
            .collect()
    } else {
        return Err(anyhow::anyhow!("Invalid WAV file (too small)"));
    };

    info!("Loaded {} samples at {} Hz", samples.len(), sample_rate);

    // Feed audio in chunks and get results
    let chunk_size = asr_config.chunk_samples();
    let mut full_text = String::new();

    for chunk in samples.chunks(chunk_size) {
        engine.feed_audio(chunk)?;

        match engine.get_transcript()? {
            result if result.is_final => {
                if !result.text.is_empty() {
                    if !full_text.is_empty() {
                        full_text.push(' ');
                    }
                    full_text.push_str(&result.text);
                    println!("[Final] {}", result.text);
                }
            }
            result if !result.text.is_empty() => {
                print!("\r[Partial] {}", result.text);
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
            _ => {}
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    // Final result
    engine.reset()?;
    match engine.get_transcript()? {
        result if !result.text.is_empty() => {
            if !full_text.is_empty() {
                full_text.push(' ');
            }
            full_text.push_str(&result.text);
            println!("\n[Final] {}", result.text);
        }
        _ => {}
    }

    println!("\n=== Full Transcript ===");
    println!("{}", full_text);

    Ok(())
}

/// Run audio dump mode for debugging.
fn run_audio_dump(dump_path: &PathBuf, app_config: &AppConfig) -> Result<()> {
    info!("Audio dump mode - saving to {:?}", dump_path);
    info!("Press Ctrl+C to stop...");

    // Initialize audio capture
    let mut audio_capture = AudioCapture::new(
        &app_config.audio.device_name,
        app_config.audio.sample_rate,
        app_config.audio.channels,
        app_config.audio.ringbuf_capacity,
    )?;

    let ringbuf = audio_capture.ringbuf().clone();
    audio_capture.start()?;

    // Collect audio for 5 seconds
    let duration = Duration::from_secs(5);
    let start = std::time::Instant::now();
    let mut all_audio: Vec<f32> = Vec::new();

    while start.elapsed() < duration {
        let available = ringbuf.len();
        if available > 0 {
            let mut buf = vec![0.0f32; available];
            let read = ringbuf.pop_slice(&mut buf);
            all_audio.extend_from_slice(&buf[..read]);
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    audio_capture.stop()?;

    // Write WAV file
    let sample_rate = app_config.audio.sample_rate;
    write_wav(dump_path, &all_audio, sample_rate)?;

    info!(
        "Saved {} samples ({:.2}s) to {:?}",
        all_audio.len(),
        all_audio.len() as f64 / sample_rate as f64,
        dump_path
    );

    Ok(())
}

/// Write a WAV file from f32 PCM data.
/// Creates a standard 16-bit PCM WAV file.
fn write_wav(path: &PathBuf, samples: &[f32], sample_rate: u32) -> Result<()> {
    use std::io::Write;

    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = samples.len() as u32 * 2; // 16-bit = 2 bytes per sample
    let file_size = 36 + data_size;

    let mut file = std::fs::File::create(path)?;

    // RIFF header
    file.write_all(b"RIFF")?;
    file.write_all(&file_size.to_le_bytes())?;
    file.write_all(b"WAVE")?;

    // fmt chunk
    file.write_all(b"fmt ")?;
    file.write_all(&(16u32).to_le_bytes())?; // chunk size
    file.write_all(&(1u16).to_le_bytes())?; // PCM format
    file.write_all(&num_channels.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&bits_per_sample.to_le_bytes())?;

    // data chunk
    file.write_all(b"data")?;
    file.write_all(&data_size.to_le_bytes())?;

    // Convert f32 samples to i16
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        file.write_all(&i16_sample.to_le_bytes())?;
    }

    Ok(())
}
