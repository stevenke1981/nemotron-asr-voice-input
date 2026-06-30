mod audio;
mod asr;
mod config;
mod convert;
mod download;
mod hotkey;
mod injector;
mod ui;
mod wav;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use audio::AudioCapture;
use audio::resampler::StreamingResampler;
use config::AppConfig;
use download::print_model_status;
use hotkey::register::HotkeyAction;
use hotkey::register::format_hotkey;
use hotkey::HotkeyManager;
use injector::{CompositeInjector, InjectStrategy, TextInjector};
use asr::{AsrConfig, TranscriptResult};
use ui::strings::{Strings, UiLang};
use ui::tray::{TrayAction, TrayManager};
use ui::gui::app::run_gui;
use ui::gui::state::{GuiAction, GuiSnapshot, ModelStatus, TranscriptEntry};


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

    /// Dump audio to WAV file for debugging
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

    /// Download model files from HuggingFace and exit
    #[arg(long)]
    download_models: bool,

    /// Print model file status and exit
    #[arg(long)]
    model_status: bool,

    /// Disable system tray icon
    #[arg(long)]
    no_tray: bool,
}

/// Application state shared between threads.
struct AppState {
    is_recording: AtomicBool,
    is_running: AtomicBool,
    /// Latest non-empty transcript text (for injection on stop).
    last_transcript: std::sync::Mutex<String>,
    /// Push-to-talk mode active (held key).
    is_ptt_mode: AtomicBool,
    /// Virtual key code for the PTT key (to detect key-up).
    ptt_vk: std::sync::Mutex<u32>,
    /// Conversion mode shared across threads.
    conversion_mode: std::sync::Mutex<convert::ConversionMode>,
    /// Signal the audio processing thread to reset the ASR engine between utterances.
    engine_reset: AtomicBool,
    /// Set to true when full-audio (non-streaming) processing completes.
    /// stop_recording() polls this instead of using a fixed sleep.
    full_audio_done: AtomicBool,
}

impl AppState {
    fn new() -> Self {
        Self {
            is_recording: AtomicBool::new(false),
            is_running: AtomicBool::new(true),
            last_transcript: std::sync::Mutex::new(String::new()),
            is_ptt_mode: AtomicBool::new(false),
            ptt_vk: std::sync::Mutex::new(0),
            conversion_mode: std::sync::Mutex::new(convert::ConversionMode::None),
            engine_reset: AtomicBool::new(false),
            full_audio_done: AtomicBool::new(false),
        }
    }
}

/// Generate HH:MM:SS timestamp from system time.
fn simple_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs();
    let hours = (total_secs / 3600) % 24;
    let mins = (total_secs / 60) % 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
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

    // ── Single-instance enforcement ────────────────────────────────
    const MUTEX_NAME: &str = "Global\\NemotronVoiceInput-{5E8B2E6A-8C1A-4C3D-9F0E-7D2A1B3C4D5E}";
    let mutex_wide: Vec<u16> = MUTEX_NAME.encode_utf16().chain(std::iter::once(0)).collect();
    let _instance_mutex = unsafe {
        let handle = windows::Win32::System::Threading::CreateMutexW(
            None, // default security attributes
            false, // initially not owned
            windows::core::PCWSTR(mutex_wide.as_ptr()),
        );
        if handle.is_err() || windows::Win32::Foundation::GetLastError() == windows::Win32::Foundation::ERROR_ALREADY_EXISTS
        {
            error!("Another instance is already running — exiting.");
            eprintln!("Nemotron Voice Input is already running.");
            std::process::exit(0);
        }
        handle
    };

    // Parse CLI arguments
    let cli = Cli::parse();

    // Handle --model-status
    if cli.model_status {
        let model_dir = cli.model_dir.clone().unwrap_or_else(|| PathBuf::from("models"));
        print_model_status(&model_dir);
        return Ok(());
    }

    // Handle --download-models
    if cli.download_models {
        let model_dir = cli.model_dir.unwrap_or_else(|| PathBuf::from("models"));
        info!("Downloading models to {:?}...", model_dir);
        download::download_models(&model_dir, None)?;
        return Ok(());
    }

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
    if let Some(ref model_dir) = cli.model_dir {
        app_config.model_dir = model_dir.clone();
    }
    if let Some(ref language) = cli.language {
        app_config.language.language = language.clone();
    }
    if let Some(ref provider) = cli.provider {
        app_config.asr.provider = provider.clone();
    }
    if let Some(ref strategy) = cli.inject {
        app_config.injector.strategy = strategy.clone();
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
        vad_threshold: app_config.asr.vad_threshold,
        language: app_config.language.language.clone(),
        decoding_method: app_config.asr.decoding_method.clone(),
        max_active_paths: app_config.asr.max_active_paths,
        sample_rate: app_config.audio.sample_rate,
    };

    // Handle --file mode (batch transcription) — needs models
    if let Some(file_path) = cli.file {
        if !download::check_model_files(&app_config.model_dir).unwrap_or(false) {
            anyhow::bail!("Model files missing. Run with --download-models first, or use interactive mode.");
        }
        return run_batch_transcription(&file_path, &asr_config);
    }

    // Handle --dump-audio mode
    if let Some(dump_path) = cli.dump_audio {
        return run_audio_dump(&dump_path, &app_config);
    }

    // === Interactive mode ===
    info!("Starting interactive mode");

    // ── Hide console window ──────────────────────────────────────────
    // Detach from the console so only the egui window is visible.
    unsafe {
        let _ = windows::Win32::System::Console::FreeConsole();
    }

    // ── Model status for GUI startup progress ────────────────────────
    let model_status = std::sync::Arc::new(std::sync::Mutex::new(ModelStatus::Checking));
    {
        let bg_model_status = model_status.clone();
        let bg_model_dir = app_config.model_dir.clone();
        let _ = std::thread::Builder::new()
            .name("model-check".into())
            .spawn(move || {
                let ok = download::check_model_files(&bg_model_dir).unwrap_or(false);
                if ok {
                    tracing::info!("All model files present — skipping download.");
                    *bg_model_status.lock().unwrap() = ModelStatus::Ready;
                    return;
                }
                tracing::info!("Model files missing — downloading with progress...");
                *bg_model_status.lock().unwrap() = ModelStatus::Downloading(0, 0);
                let notify = |phase: &str, _current: u64, _total: u64| {
                    let mut s = bg_model_status.lock().unwrap();
                    match phase {
                        "downloading_tarball" => *s = ModelStatus::Downloading(_current, _total),
                        "extracting" => *s = ModelStatus::Extracting,
                        "downloading_vad" => *s = ModelStatus::Downloading(_current, _total),
                        _ => *s = ModelStatus::Downloading(_current, _total),
                    }
                };
                match download::download_models(&bg_model_dir, Some(&notify)) {
                    Ok(()) => {
                        tracing::info!("Model download complete.");
                        *bg_model_status.lock().unwrap() = ModelStatus::Ready;
                    }
                    Err(e) => {
                        tracing::error!("Model download failed: {}", e);
                        *bg_model_status.lock().unwrap() = ModelStatus::Failed(e.to_string());
                    }
                }
            })
            .expect("Failed to spawn model-check thread");
    }

    // Initialize state
    let state = Arc::new(AppState::new());
    let language_list = Arc::new(app_config.language.cycle_order.clone());
    let current_language = Arc::new(std::sync::Mutex::new(app_config.language.language.clone()));

    // Initialize audio capture
    let audio_capture = AudioCapture::new(
        &app_config.audio.device_name,
        app_config.audio.sample_rate,
        app_config.audio.channels,
        app_config.audio.ringbuf_capacity,
    )?;

    // Initialize text injector
    let inject_strategy = InjectStrategy::from_string(&app_config.injector.strategy);
    let injector = CompositeInjector::with_strategy(inject_strategy);

    // Store PTT VK for key-up detection (must happen before bg thread starts)
    *state.ptt_vk.lock().unwrap() = app_config.hotkey.ptt_vk;

    // Initialize the conversion mode from config (also set runtime static)
    let initial_mode = convert::ConversionMode::from_config(&app_config.conversion.mode);
    *state.conversion_mode.lock().unwrap() = initial_mode;
    config::settings::RUNTIME_CONVERSION_MODE.store(initial_mode.index() as u8, std::sync::atomic::Ordering::SeqCst);

    // Channels
    let (transcript_tx, transcript_rx) = crossbeam::channel::bounded::<TranscriptResult>(64);
    let (tray_tx, tray_rx) = crossbeam::channel::unbounded::<TrayAction>();

    // ── UI strings ───────────────────────────────────────────────────
    let ui_lang = UiLang::from_code(&app_config.ui.language);
    let ui_strings = Strings::new(ui_lang);
    ui::tray::set_ui_lang(&app_config.ui.language);
    info!("UI language: {:?}", ui_lang);

    // ── GUI initialization ──────────────────────────────────
    let gui_snapshot = Arc::new(std::sync::Mutex::new(GuiSnapshot {
        is_recording: false,
        current_language: app_config.language.language.clone(),
        conversion_mode: app_config.conversion.mode.clone(),
        latest_final_text: String::new(),
        latest_partial_text: String::new(),
        history: Vec::new(),
        show_settings_requested: false,
        exit_requested: false,
    }));

    let (gui_snapshot_tx, gui_snapshot_rx) = crossbeam::channel::bounded::<GuiSnapshot>(256);
    let (gui_action_tx, gui_action_rx) = crossbeam::channel::unbounded::<GuiAction>();
    let show_overlay = Arc::new(AtomicBool::new(false));

    let initial_pos = app_config.ui.window_x
        .zip(app_config.ui.window_y)
        .map(|(x, y)| egui::Pos2::new(x, y));
    let initial_size = app_config.ui.window_width
        .zip(app_config.ui.window_height)
        .map(|(w, h)| egui::Vec2::new(w, h));
    let gui_theme = app_config.ui.theme.clone();

    // ── Audio capture processing thread ──────────────────────────────
    let gui_snapshot_for_audio = gui_snapshot.clone();
    let gui_snapshot_tx_for_audio = gui_snapshot_tx.clone();
    let audio_state = state.clone();
    let audio_tx = transcript_tx.clone();
    let audio_ringbuf = audio_capture.ringbuf().clone();
    let asr_config_clone = asr_config.clone();
    let _audio_language = current_language.clone();
    let capture_sample_rate = audio_capture.capture_rate();

    // Pass model_status to audio thread for startup wait
    let model_status_for_audio = model_status.clone();

    let _audio_handle = std::thread::Builder::new()
        .name("audio-processor".into())
        .spawn(move || {
            // Set thread priority
            set_current_thread_priority(2);

            // Wait for models to be ready (GUI shows download progress during this time)
            loop {
                let status = model_status_for_audio.lock().unwrap().clone();
                match status {
                    ModelStatus::Ready => {
                        tracing::info!("Models ready — starting ASR engine.");
                        break;
                    }
                    ModelStatus::Failed(msg) => {
                        tracing::error!("Model download failed: {} — ASR engine not available.", msg);
                        tracing::info!("Audio processing will continue without ASR (no transcripts).");
                        // Continue without ASR engine — all recordings will produce no output
                        return;
                    }
                    _ => {
                        // Still checking/downloading — keep waiting
                    }
                }
                if !audio_state.is_running.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }

            let mut engine = match asr::create_asr_engine(&asr_config_clone) {
                Ok(e) => e,
                Err(e) => {
                    error!("Failed to create ASR engine: {}", e);
                    error!("  Models may be missing or corrupt. Run `nemotron-voice-input --download-models` to download.");
                    error!("  Or run `nemotron-voice-input --model-status` to check model status.");
                    info!("ASR engine unavailable — recording will not produce transcripts");
                    return;
                }
            };

            let target_rate: u32 = asr_config_clone.sample_rate; // 16000
            let chunk_target = asr_config_clone.chunk_samples(); // samples at target_rate

            // How many capture-rate samples we need per chunk
            let chunk_capture = if capture_sample_rate != target_rate {
                // Round up to avoid starving the ASR
                ((chunk_target as u64 * capture_sample_rate as u64 + target_rate as u64 - 1)
                    / target_rate as u64) as usize
            } else {
                chunk_target
            };

            let mut capture_buf = vec![0.0f32; chunk_capture];
            let mut resampler = match StreamingResampler::new(capture_sample_rate, target_rate) {
                Ok(resampler) => resampler,
                Err(e) => {
                    error!("Failed to create audio resampler: {}", e);
                    return;
                }
            };
            let mut last_text = String::new();
            let mut last_vad = config::settings::RUNTIME_VAD_ENABLED.load(Ordering::SeqCst);
            let mut last_vad_threshold = f32::from_bits(
                config::settings::RUNTIME_VAD_THRESHOLD.load(Ordering::SeqCst));
            let mut was_recording = false;
            // Full-audio buffer: accumulates ALL resampled audio during recording.
            // On stop, fed to a fresh ASR stream for complete-context decode.
            let mut full_audio: Vec<f32> = Vec::new();

            info!(
                "Audio processing: capture {} Hz → target {} Hz, chunk {} → {} samples",
                capture_sample_rate, target_rate, chunk_capture, chunk_target
            );

            while audio_state.is_running.load(Ordering::SeqCst) {
                let is_recording = audio_state.is_recording.load(Ordering::SeqCst);

                // ── Recording start ──────────────────────────────────────
                if is_recording && !was_recording {
                    last_text.clear();
                    full_audio.clear();
                    resampler.reset();
                    audio_state.full_audio_done.store(false, Ordering::SeqCst);
                    if audio_state.engine_reset.swap(false, Ordering::SeqCst) {
                        if let Err(e) = engine.reset() {
                            tracing::error!("ASR engine reset error: {}", e);
                        } else {
                            tracing::debug!("ASR engine reset for new utterance");
                        }
                    }
                }

                // ── Recording stop: process full accumulated audio ───────
                if !is_recording && was_recording {
                    // Drain remaining ring buffer into full_audio
                    loop {
                        let remaining = audio_ringbuf.len();
                        if remaining == 0 { break; }
                        let r = capture_buf.len().min(remaining);
                        let read = audio_ringbuf.pop_slice(&mut capture_buf[..r]);
                        if read == 0 { break; }
                        if let Err(e) = resampler.process_into(&capture_buf[..read], &mut full_audio) {
                            tracing::error!("Audio resampling failed while draining: {}", e);
                            break;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(50));
                    loop {
                        let remaining = audio_ringbuf.len();
                        if remaining == 0 { break; }
                        let r = capture_buf.len().min(remaining);
                        let read = audio_ringbuf.pop_slice(&mut capture_buf[..r]);
                        if read == 0 { break; }
                        if let Err(e) = resampler.process_into(&capture_buf[..read], &mut full_audio) {
                            tracing::error!("Audio resampling failed during final drain: {}", e);
                            break;
                        }
                    }

                    if let Err(e) = resampler.flush_into(&mut full_audio) {
                        tracing::error!("Audio resampler flush failed: {}", e);
                    }

                    if !full_audio.is_empty() {
                        // Save utterance audio to voices/ (before silence padding)
                        if let Err(e) = wav::save_utterance_audio(&full_audio, target_rate) {
                            tracing::error!("Failed to save utterance audio: {}", e);
                        }

                        match asr::decode_complete_utterance(engine.as_mut(), &full_audio) {
                            Ok(result) if !result.text.is_empty() && result.text != last_text => {
                                let final_text = result.text;
                                tracing::info!("Full-audio transcript: {}", final_text);
                                last_text = final_text.clone();
                                *audio_state.last_transcript.lock().unwrap() = final_text.clone();
                                let _ = audio_tx.send(TranscriptResult {
                                    text: final_text.clone(), is_final: true, segment_id: 0, confidence: 0.0,
                                });
                                let mut snap = gui_snapshot_for_audio.lock().unwrap();
                                snap.latest_final_text = final_text.clone();
                                snap.latest_partial_text.clear();
                                snap.history.push(TranscriptEntry {
                                    text: final_text, timestamp: simple_timestamp(),
                                    language: asr_config_clone.language.clone(),
                                });
                                snap.is_recording = false;
                                if gui_snapshot_tx_for_audio.send(snap.clone()).is_err() {
                                    tracing::debug!("GUI snapshot channel closed during full-audio stop");
                                }
                            }
                            Ok(_) => {}
                            Err(e) => tracing::error!("Full-audio decode failed: {}", e),
                        }
                    }
                    audio_state.full_audio_done.store(true, Ordering::SeqCst);
                }

                was_recording = is_recording;

                if !is_recording {
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }

                // ── During recording: accumulate + streaming decode ──────
                let available = audio_ringbuf.len();
                if available < chunk_capture {
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }

                let to_read = capture_buf.len().min(available);
                let read = audio_ringbuf.pop_slice(&mut capture_buf[..to_read]);
                if read == 0 { continue; }

                // Resample and accumulate into full_audio
                let input_slice = &capture_buf[..read];
                let pre_len = full_audio.len();
                if let Err(e) = resampler.process_into(input_slice, &mut full_audio) {
                    tracing::error!("Audio resampling failed: {}", e);
                    continue;
                }
                let feed_data = &full_audio[pre_len..];

                // Runtime VAD toggle + threshold
                let current_vad = config::settings::RUNTIME_VAD_ENABLED.load(Ordering::SeqCst);
                if current_vad != last_vad {
                    last_vad = current_vad;
                    if let Err(e) = engine.set_vad(current_vad) {
                        tracing::warn!("Failed to set VAD state to {}: {}", current_vad, e);
                    }
                }
                let current_threshold = f32::from_bits(
                    config::settings::RUNTIME_VAD_THRESHOLD.load(Ordering::SeqCst));
                if (current_threshold - last_vad_threshold).abs() > 0.001 {
                    last_vad_threshold = current_threshold;
                    if let Err(e) = engine.set_vad_threshold(current_threshold) {
                        tracing::warn!("Failed to set VAD threshold to {}: {}", current_threshold, e);
                    }
                }

                if let Err(e) = engine.feed_audio(feed_data) {
                    tracing::debug!("ASR feed error: {}", e);
                    continue;
                }

                // Streaming decode (real-time partial results)
                match engine.get_transcript() {
                    Ok(result) => {
                        if !result.text.is_empty() && (result.is_final || result.text != last_text) {
                            if result.text != last_text { last_text = result.text.clone(); }
                            *audio_state.last_transcript.lock().unwrap() = result.text.clone();
                            let result_clone = result.clone();
                            if audio_tx.send(result).is_err() { break; }
                            let mut snap = gui_snapshot_for_audio.lock().unwrap();
                            let trimmed = result_clone.text.trim().to_string();
                            if result_clone.is_final {
                                snap.latest_final_text = trimmed.clone();
                                snap.latest_partial_text.clear();
                                snap.history.push(TranscriptEntry {
                                    text: trimmed, timestamp: simple_timestamp(),
                                    language: asr_config_clone.language.clone(),
                                });
                            } else {
                                snap.latest_partial_text = trimmed;
                            }
                            snap.is_recording = audio_state.is_recording.load(Ordering::SeqCst);
                            if gui_snapshot_tx_for_audio.send(snap.clone()).is_err() {
                                tracing::debug!("GUI snapshot channel closed (audio thread exiting)");
                            }
                        }
                    }
                    Err(e) => tracing::debug!("ASR transcript error: {}", e),
                }
            }

            info!("Audio processing thread exiting");
        })
        .context("Failed to spawn audio processing thread")?;

    // Main loop
    // Initialize runtime VAD flag from config
    config::settings::RUNTIME_VAD_ENABLED.store(app_config.asr.use_vad, Ordering::SeqCst);

    // Initialize Chinese text converters
    if let Err(e) = convert::init_converters() {
        info!("Chinese text converter not available: {} (continuing without conversion)", e);
    } else {
        info!("Chinese text converter initialized");
    }

    // Shared running flag for GUI <-> background thread coordination.
    // This shadows `state.is_running` (used by audio/watchdog threads);
    // both are set to false on exit.
    let running_flag = Arc::new(AtomicBool::new(true));

    // ── Spawn Win32 message loop on background thread ─────────────────
    let bg_flag = running_flag.clone();
    let bg_state = state.clone();
    let bg_gui_snapshot = gui_snapshot.clone();
    let bg_gui_snapshot_tx = gui_snapshot_tx.clone();
    let bg_show_overlay = show_overlay.clone();
    let bg_language_list = language_list.clone();
    let bg_current_language = current_language.clone();

    let bg_config_path = cli.config.clone();
    let bg_tray_tx = tray_tx.clone();
    let _bg_handle = std::thread::Builder::new()
        .name("win32-main-loop".into())
        .spawn(move || {
            win32_background_loop(
                bg_flag,
                bg_state,
                audio_capture,
                injector,
                transcript_rx,
                tray_rx,
                gui_action_rx,
                bg_gui_snapshot,
                bg_gui_snapshot_tx,
                bg_show_overlay,
                bg_language_list,
                bg_current_language,
                cli.no_tray,
                bg_tray_tx,
                ui_strings,
                bg_config_path,
                // hotkey config
                app_config.hotkey.toggle_modifiers,
                app_config.hotkey.toggle_vk,
                app_config.hotkey.lang_modifiers,
                app_config.hotkey.lang_vk,
                app_config.hotkey.flush_modifiers,
                app_config.hotkey.flush_vk,
                app_config.hotkey.ptt_modifiers,
                app_config.hotkey.ptt_vk,
            );
        })
        .context("Failed to spawn Win32 background thread")?;

    // ── Run GUI on main thread (blocking until window closes) ─────
    // The GUI shows model download progress during startup.
    run_gui(
        gui_snapshot,
        gui_snapshot_rx,
        gui_action_tx,
        show_overlay,
        running_flag.clone(),
        initial_pos,
        initial_size,
        Some(gui_theme),
        ui_lang,
        Some(&app_config),
        model_status,
    );

    // ── GUI window closed → signal shutdown, wait for bg thread ───
    info!("GUI window closed — shutting down background threads...");
    state.is_running.store(false, Ordering::SeqCst);
    running_flag.store(false, Ordering::SeqCst);
    // bg_handle will rejoin shortly (running_flag already false, or bg thread already exited)

    info!("Nemotron Voice Input stopped.");
    Ok(())
}

/// Win32 message pump running on a background thread.
///
/// Handles hotkeys, tray messages, transcript injection, GUI actions,
/// and PTT key-up monitoring. Exits when `running_flag` becomes false.
fn win32_background_loop(
    running_flag: Arc<AtomicBool>,
    state: Arc<AppState>,
    mut audio_capture: AudioCapture,
    mut injector: CompositeInjector,
    transcript_rx: crossbeam::channel::Receiver<TranscriptResult>,
    tray_rx: crossbeam::channel::Receiver<TrayAction>,
    gui_action_rx: crossbeam::channel::Receiver<GuiAction>,
    gui_snapshot: Arc<std::sync::Mutex<GuiSnapshot>>,
    gui_snapshot_tx: crossbeam::channel::Sender<GuiSnapshot>,
    show_overlay: Arc<AtomicBool>,
    language_list: Arc<Vec<String>>,
    current_language: Arc<std::sync::Mutex<String>>,
    no_tray: bool,
    tray_tx: crossbeam::channel::Sender<TrayAction>,
    ui_strings: Strings,
    config_path: PathBuf,
    // Hotkey configuration
    toggle_modifiers: u32,
    toggle_vk: u32,
    lang_modifiers: u32,
    lang_vk: u32,
    flush_modifiers: u32,
    flush_vk: u32,
    ptt_modifiers: u32,
    ptt_vk: u32,
) {
    // ── Initialize Win32 resources on this thread ─────────────────────
    let mut hotkey_manager = HotkeyManager::new();
    register_hotkey(&mut hotkey_manager, HotkeyAction::ToggleRecording, toggle_modifiers, toggle_vk);
    register_hotkey(&mut hotkey_manager, HotkeyAction::CycleLanguage, lang_modifiers, lang_vk);
    register_hotkey(&mut hotkey_manager, HotkeyAction::Flush, flush_modifiers, flush_vk);
    register_hotkey(&mut hotkey_manager, HotkeyAction::PushToTalk, ptt_modifiers, ptt_vk);
    let toggle_reg = hotkey_manager.actual_key(HotkeyAction::ToggleRecording)
        .map(|(m, v)| format_hotkey(m, v))
        .unwrap_or_else(|| format_hotkey(toggle_modifiers, toggle_vk));
    let ptt_reg = hotkey_manager.actual_key(HotkeyAction::PushToTalk)
        .map(|(m, v)| format_hotkey(m, v))
        .unwrap_or_else(|| format_hotkey(ptt_modifiers, ptt_vk));
    info!("Hotkeys registered: ToggleRecording({}), PushToTalk({})", toggle_reg, ptt_reg);

    let tray_manager: TrayManager;
    if !no_tray {
        let mut tm = TrayManager::new();
        if let Err(e) = tm.initialize(tray_tx) {
            warn!("Failed to initialize system tray: {} (continuing without tray)", e);
            tray_manager = TrayManager::new();
        } else {
            tm.show_notification(ui_strings.app_name(), ui_strings.notification_ready());
            tray_manager = tm;
        }
    } else {
        tray_manager = TrayManager::new();
    }

    info!(
        "Ready. Press {} to start/stop recording, {} for push-to-talk.",
        toggle_reg, ptt_reg
    );

    let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();

    unsafe {
        while running_flag.load(Ordering::SeqCst) {
            let has_message = windows::Win32::UI::WindowsAndMessaging::PeekMessageA(
                &mut msg,
                None,
                0,
                0,
                windows::Win32::UI::WindowsAndMessaging::PM_REMOVE,
            );

            if has_message.as_bool() {
                if msg.message == windows::Win32::UI::WindowsAndMessaging::WM_QUIT {
                    running_flag.store(false, Ordering::SeqCst);
                    break;
                }

                // Handle hotkey
                if let Some(action) = hotkey_manager.process_message(&msg) {
                    handle_hotkey_action(
                        action,
                        &state,
                        &mut audio_capture,
                        &mut injector,
                        &language_list,
                        &current_language,
                        &tray_manager,
                    );
                }

                let _ = windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                windows::Win32::UI::WindowsAndMessaging::DispatchMessageA(&msg);
            } else {
                // ── PTT key-up monitoring ────────────────────────────
                if state.is_ptt_mode.load(Ordering::SeqCst) {
                    let vk = *state.ptt_vk.lock().unwrap();
                    let key_down = windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk as i32) < 0;
                    if !key_down {
                        state.is_ptt_mode.store(false, Ordering::SeqCst);
                        stop_recording(&state, &mut audio_capture, &mut injector, &tray_manager);
                    }
                }

                // Check for transcript results
                let recv_count = transcript_rx.len();
                if recv_count > 0 {
                    for _ in 0..recv_count {
                        if let Ok(result) = transcript_rx.try_recv() {
                            if !result.text.is_empty()
                                && state.is_recording.load(Ordering::SeqCst)
                            {
                                info!("Transcript: {}", result.text);
                                if result.is_final {
                                    let mode = config::settings::runtime_conversion_mode();
                                    let text = convert::convert_text(&result.text, mode);
                                    if let Err(e) = injector.inject_text(&text) {
                                        error!("Text injection failed: {}", e);
                                    } else {
                                        info!("Injected: {}", text);
                                    }
                                }
                            }
                        }
                    }
                }

                // Check for tray actions
                while let Ok(action) = tray_rx.try_recv() {
                    match action {
                        TrayAction::ToggleRecording => {
                            if state.is_recording.load(Ordering::SeqCst) {
                                stop_recording(&state, &mut audio_capture, &mut injector, &tray_manager);
                            } else {
                                start_recording(&state, &mut audio_capture, &tray_manager);
                            }
                        }
                        TrayAction::CycleLanguage => {
                            cycle_language(&language_list, &current_language, &tray_manager);
                        }
                        TrayAction::Flush => {
                            info!("Tray: Flush");
                            audio_capture.clear_ringbuf();
                            tray_manager.show_notification(
                                ui_strings.app_name(),
                                ui_strings.notification_flushed(),
                            );
                        }
                        TrayAction::OpenSettings => {
                            info!("Tray: Open Settings requested (egui panel)");
                            if let Ok(mut snap) = gui_snapshot.lock() {
                                snap.show_settings_requested = true;
                            }
                            let _ = gui_snapshot_tx.send(gui_snapshot.lock().unwrap().clone());
                        }
                        TrayAction::ShowMainWindow => {
                            info!("Tray: Show Main Window requested");
                        }
                        TrayAction::ToggleOverlay => {
                            let show = !show_overlay.load(Ordering::SeqCst);
                            show_overlay.store(show, Ordering::SeqCst);
                            info!("Tray: Toggling overlay: {}", show);
                            // Forward to GUI (action_tx), but this bg thread only has gui_action_rx.
                            // The overlay state is passed via show_overlay AtomicBool, so
                            // GuiApp::update will pick it up from GuiSharedState.
                        }
                        TrayAction::Exit => {
                            info!("Tray: Exit requested");
                            // Signal all threads
                            state.is_running.store(false, Ordering::SeqCst);
                            running_flag.store(false, Ordering::SeqCst);
                            // Tell the GUI to close via snapshot
                            if let Ok(mut snap) = gui_snapshot.lock() {
                                snap.exit_requested = true;
                            }
                            let _ = gui_snapshot_tx.send(gui_snapshot.lock().unwrap().clone());
                            break;
                        }
                    }
                }

                // Check for GUI actions
                while let Ok(gui_action) = gui_action_rx.try_recv() {
                    match gui_action {
                        GuiAction::ToggleRecording => {
                            if state.is_recording.load(Ordering::SeqCst) {
                                stop_recording(&state, &mut audio_capture, &mut injector, &tray_manager);
                            } else {
                                start_recording(&state, &mut audio_capture, &tray_manager);
                            }
                        }
                        GuiAction::CycleLanguage => {
                            cycle_language(&language_list, &current_language, &tray_manager);
                        }
                        GuiAction::Flush => {
                            info!("GUI: Flush triggered");
                            audio_capture.clear_ringbuf();
                        }
                        GuiAction::SetLanguage(lang) => {
                            *current_language.lock().unwrap() = lang;
                        }
                        GuiAction::SaveConfig(cfg) => {
                            info!("GUI: Saving config");
                            if let Err(e) = cfg.save(&config_path) {
                                error!("Failed to save config: {}", e);
                            } else {
                                // Update runtime settings
                                *current_language.lock().unwrap() = cfg.language.language.clone();
                                config::settings::RUNTIME_VAD_ENABLED.store(cfg.asr.use_vad, Ordering::SeqCst);
                                config::settings::RUNTIME_VAD_THRESHOLD.store(
                                    cfg.asr.vad_threshold.to_bits(), Ordering::SeqCst);
                                config::settings::RUNTIME_CONVERSION_MODE.store(
                                    convert::ConversionMode::from_config(&cfg.conversion.mode).index() as u8,
                                    Ordering::SeqCst,
                                );
                                *state.conversion_mode.lock().unwrap() = convert::ConversionMode::from_config(&cfg.conversion.mode);
                                info!("Config saved and runtime settings updated");
                            }
                        }
                        GuiAction::ShowOverlay(_show) => {
                            info!("GUI: Overlay toggled");
                        }
                        GuiAction::DeleteHistoryEntry(_idx) => {
                            let _snap = gui_snapshot.lock().unwrap();
                            // history deletion handled on the GUI side
                        }
                        GuiAction::ClearHistory => {
                            gui_snapshot.lock().unwrap().history.clear();
                        }
                        GuiAction::Exit => {
                            info!("GUI: Exit requested");
                            state.is_running.store(false, Ordering::SeqCst);
                            running_flag.store(false, Ordering::SeqCst);
                            if let Ok(mut snap) = gui_snapshot.lock() {
                                snap.exit_requested = true;
                            }
                            let _ = gui_snapshot_tx.send(gui_snapshot.lock().unwrap().clone());
                            break;
                        }
                    }
                }

                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    // Cleanup
    let _ = audio_capture.stop();
    info!("Win32 background loop exited");
}

/// Register a hotkey with error handling.
fn register_hotkey(manager: &mut HotkeyManager, action: HotkeyAction, modifiers: u32, vk: u32) {
    if let Err(e) = manager.register(action, modifiers, vk) {
        error!("Failed to register hotkey: {}", e);
    }
}

/// Handle a hotkey action (called from both message loop and tray).
fn handle_hotkey_action(
    action: HotkeyAction,
    state: &AppState,
    audio_capture: &mut AudioCapture,
    injector: &mut CompositeInjector,
    language_list: &[String],
    current_language: &Arc<std::sync::Mutex<String>>,
    tray: &TrayManager,
) {
    info!("Hotkey action: {:?}", action);
    match action {
        HotkeyAction::ToggleRecording => {
            if state.is_recording.load(Ordering::SeqCst) {
                stop_recording(state, audio_capture, injector, tray);
            } else {
                start_recording(state, audio_capture, tray);
            }
        }
        HotkeyAction::CycleLanguage => {
            cycle_language(language_list, current_language, tray);
        }
        HotkeyAction::PushToTalk => {
            if !state.is_recording.load(Ordering::SeqCst) {
                // Start recording in PTT mode (key-down)
                state.is_ptt_mode.store(true, Ordering::SeqCst);
                start_recording(state, audio_capture, tray);
            }
        }
        HotkeyAction::Flush => {
            info!("Flush triggered");
            audio_capture.clear_ringbuf();
            tray.show_notification("Flush", "Buffer cleared");
        }
    }
}

/// Start recording.
fn start_recording(
    state: &AppState,
    audio_capture: &mut AudioCapture,
    tray: &TrayManager,
) {
    audio_capture.clear_ringbuf();
    *state.last_transcript.lock().unwrap() = String::new();
    // Signal the audio processing thread to reset the ASR decoder state
    state.engine_reset.store(true, Ordering::SeqCst);

    if let Err(e) = audio_capture.start() {
        error!("Failed to start recording: {}", e);
        tray.show_notification("Error", &format!("Failed to start recording: {}", e));
        return;
    }
    state.is_recording.store(true, Ordering::SeqCst);
    tray.set_recording_state(true);

    let s = ui::tray::tray_strings();
    tray.show_notification(s.app_name(), s.notification_recording_started());
    info!("Recording started - speak now");
}

/// Stop recording and inject the last transcript.
fn stop_recording(
    state: &AppState,
    audio_capture: &mut AudioCapture,
    injector: &mut CompositeInjector,
    tray: &TrayManager,
) {
    // CRITICAL: Stop audio capture FIRST before signaling the audio thread.
    // If we set is_recording=false first, the audio thread starts draining
    // the ring buffer while the cpal callback may still be pushing data,
    // causing the last ~50-100ms of audio to be truncated.
    if let Err(e) = audio_capture.stop() {
        error!("Failed to stop recording: {}", e);
        tray.show_notification("Error", &format!("Failed to stop recording: {}", e));
    }

    // Small sleep to let any in-flight callback data reach the ring buffer
    // before the audio thread drains it.
    std::thread::sleep(Duration::from_millis(10));

    state.is_recording.store(false, Ordering::SeqCst);
    tray.set_recording_state(false);

    // Wait for the audio processing thread to finish full-audio decode.
    // Poll full_audio_done flag with 100ms intervals (up to 10 seconds).
    for _ in 0..100 {
        if state.full_audio_done.load(Ordering::SeqCst) { break; }
        std::thread::sleep(Duration::from_millis(100));
    }
    // Small extra wait for async injection to propagate
    std::thread::sleep(Duration::from_millis(50));

    // Inject the last non-empty transcript
    let text = state.last_transcript.lock().unwrap().clone();
    if !text.is_empty() {
        let mode = config::settings::runtime_conversion_mode();
        let converted = convert::convert_text(&text, mode);
        if let Err(e) = injector.inject_text(&converted) {
            error!("Text injection failed (on stop): {}", e);
        } else {
            info!("Injected (on stop): {}", converted);
        }
    }

    let s = ui::tray::tray_strings();
    tray.show_notification(s.app_name(), s.notification_recording_stopped());
    info!("Recording stopped");
}

/// Cycle through configured languages.
fn cycle_language(
    language_list: &[String],
    current_language: &Arc<std::sync::Mutex<String>>,
    tray: &TrayManager,
) {
    if language_list.is_empty() {
        return;
    }

    let current = current_language.lock().unwrap().clone();
    let pos = language_list.iter().position(|l| l == &current).unwrap_or(0);
    let next_pos = (pos + 1) % language_list.len();
    let next_lang = &language_list[next_pos];

    *current_language.lock().unwrap() = next_lang.clone();
    info!("Language cycled: {} -> {}", current, next_lang);

    let msg = ui::tray::tray_strings().notification_language_switched_to(next_lang);
    tray.show_notification("Language", &msg);
}

/// Set thread priority via Windows API.
fn set_current_thread_priority(priority: i32) {
    use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY};
    unsafe {
        let thread = GetCurrentThread();
        let tp = THREAD_PRIORITY(priority);
        match SetThreadPriority(thread, tp) {
            Ok(()) => info!("Audio thread priority set to {}", priority),
            Err(e) => warn!("Failed to set thread priority to {}: {}", priority, e),
        }
    }
}

/// Run batch transcription on a WAV file.
fn run_batch_transcription(file_path: &PathBuf, asr_config: &AsrConfig) -> Result<()> {
    info!("Transcribing file: {:?}", file_path);

    let mut engine = asr::create_asr_engine(asr_config)?;
    let wav_data = std::fs::read(file_path).context("Failed to read WAV file")?;
    let (sample_rate, samples) = wav::parse_pcm16_mono_wav(&wav_data)?;
    anyhow::ensure!(
        sample_rate == asr_config.sample_rate,
        "WAV sample rate is {} Hz, but ASR expects {} Hz",
        sample_rate,
        asr_config.sample_rate
    );

    info!("Loaded {} samples at {} Hz", samples.len(), sample_rate);
    let result = asr::decode_complete_utterance(engine.as_mut(), &samples)?;

    println!("\n=== Full Transcript ===");
    println!("{}", result.text);

    Ok(())
}

/// Run audio dump mode for debugging.
fn run_audio_dump(dump_path: &PathBuf, app_config: &AppConfig) -> Result<()> {
    info!("Audio dump mode - saving to {:?}", dump_path);
    info!("Press Ctrl+C to stop...");

    let mut audio_capture = AudioCapture::new(
        &app_config.audio.device_name,
        app_config.audio.sample_rate,
        app_config.audio.channels,
        app_config.audio.ringbuf_capacity,
    )?;

    let ringbuf = audio_capture.ringbuf().clone();
    audio_capture.start()?;

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

    let sample_rate = app_config.audio.sample_rate;
    wav::write_wav(dump_path, &all_audio, sample_rate)?;

    info!(
        "Saved {} samples ({:.2}s) to {:?}",
        all_audio.len(),
        all_audio.len() as f64 / sample_rate as f64,
        dump_path
    );

    Ok(())
}

