mod audio;
mod asr;
mod config;
mod download;
mod hotkey;
mod injector;
mod ui;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use audio::AudioCapture;
use config::AppConfig;
use download::print_model_status;
use hotkey::register::HotkeyAction;
use hotkey::HotkeyManager;
use injector::{CompositeInjector, InjectStrategy, TextInjector};
use asr::{AsrConfig, TranscriptResult};
use ui::strings::{Strings, UiLang};
use ui::tray::{TrayAction, TrayManager};
use ui::config_window;

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
        download::download_models(&model_dir)?;
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

    // Auto-download models if missing
    if !download::check_model_files(&app_config.model_dir).unwrap_or(false) {
        info!("Model files missing. Downloading...");
        download::download_models(&app_config.model_dir)?;
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
    let language_list = Arc::new(app_config.language.cycle_order.clone());
    let current_language = Arc::new(std::sync::Mutex::new(app_config.language.language.clone()));

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
    register_hotkey(
        &mut hotkey_manager,
        HotkeyAction::ToggleRecording,
        app_config.hotkey.toggle_modifiers,
        app_config.hotkey.toggle_vk,
    );
    register_hotkey(
        &mut hotkey_manager,
        HotkeyAction::CycleLanguage,
        app_config.hotkey.lang_modifiers,
        app_config.hotkey.lang_vk,
    );
    register_hotkey(
        &mut hotkey_manager,
        HotkeyAction::Flush,
        app_config.hotkey.flush_modifiers,
        app_config.hotkey.flush_vk,
    );
    info!("Hotkeys registered");

    // Channels
    let (transcript_tx, transcript_rx) = crossbeam::channel::bounded::<TranscriptResult>(64);
    let (tray_tx, tray_rx) = crossbeam::channel::unbounded::<TrayAction>();

    // ── UI strings ───────────────────────────────────────────────────
    let ui_lang = UiLang::from_code(&app_config.ui.language);
    let ui_strings = Strings::new(ui_lang);
    ui::tray::set_ui_lang(&app_config.ui.language);
    info!("UI language: {:?}", ui_lang);

    // ── Tray initialization ──────────────────────────────────────────
    let mut tray_manager = TrayManager::new();
    if !cli.no_tray {
        if let Err(e) = tray_manager.initialize(tray_tx.clone()) {
            warn!("Failed to initialize system tray: {} (continuing without tray)", e);
        } else {
            tray_manager.show_notification(
                ui_strings.app_name(),
                ui_strings.notification_ready(),
            );
        }
    }

    // Store tray sender for Settings window
    let _tray_tx_for_settings = tray_tx.clone();

    // ── Audio capture processing thread ──────────────────────────────
    let audio_state = state.clone();
    let audio_tx = transcript_tx.clone();
    let audio_ringbuf = audio_capture.ringbuf().clone();
    let asr_config_clone = asr_config.clone();
    let _audio_language = current_language.clone();

    let _audio_handle = std::thread::Builder::new()
        .name("audio-processor".into())
        .spawn(move || {
            // Set thread priority
            set_current_thread_priority(2);

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

            let chunk_samples = asr_config_clone.chunk_samples();
            let mut temp_buf = vec![0.0f32; chunk_samples];
            let mut last_text = String::new();

            while audio_state.is_running.load(Ordering::SeqCst) {
                if !audio_state.is_recording.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }

                let available = audio_ringbuf.len();
                if available < chunk_samples {
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }

                let to_read = temp_buf.len().min(available);
                let read = audio_ringbuf.pop_slice(&mut temp_buf[..to_read]);
                if read == 0 {
                    continue;
                }

                if let Err(e) = engine.feed_audio(&temp_buf[..read]) {
                    tracing::debug!("ASR feed error: {}", e);
                    continue;
                }

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

    // ── Watchdog thread ──────────────────────────────────────────────
    let watchdog_state = state.clone();
    let _watchdog_handle = std::thread::Builder::new()
        .name("watchdog".into())
        .spawn(move || {
            while watchdog_state.is_running.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_secs(30));
                if !watchdog_state.is_running.load(Ordering::SeqCst) {
                    break;
                }
                tracing::debug!("Watchdog tick - application running");
            }
            info!("Watchdog thread exiting");
        })
        .context("Failed to spawn watchdog thread")?;

    // Main loop
    info!(
        "Ready. Press Ctrl+Alt+R to start/stop recording. Language: {}",
        asr_config.language
    );

    let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();

    unsafe {
        while state.is_running.load(Ordering::SeqCst) {
            let has_message = windows::Win32::UI::WindowsAndMessaging::PeekMessageA(
                &mut msg,
                None, // get messages for all windows owned by this thread
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
                        &mut injector,
                        &language_list,
                        &current_language,
                        &tray_manager,
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
                            if !result.text.is_empty()
                                && state.is_recording.load(Ordering::SeqCst)
                            {
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

                // Check for tray actions
                while let Ok(action) = tray_rx.try_recv() {
                    match action {
                        TrayAction::ToggleRecording => {
                            if state.is_recording.load(Ordering::SeqCst) {
                                stop_recording(&state, &mut audio_capture, &tray_manager);
                            } else {
                                start_recording(&state, &mut audio_capture, &tray_manager);
                            }
                        }
                        TrayAction::CycleLanguage => {
                            cycle_language(
                                &language_list,
                                &current_language,
                                &tray_manager,
                            );
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
                            info!("Tray: Open Settings requested");
                            config_window::show_config_window(
                                tray_manager.hwnd(),
                                &app_config,
                            );
                        }
                        TrayAction::Exit => {
                            info!("Tray: Exit requested");
                            state.is_running.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                }

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

/// Handle a hotkey action (called from both message loop and tray).
fn handle_hotkey_action(
    action: HotkeyAction,
    state: &AppState,
    audio_capture: &mut AudioCapture,
    _injector: &mut CompositeInjector,
    language_list: &[String],
    current_language: &Arc<std::sync::Mutex<String>>,
    tray: &TrayManager,
) {
    match action {
        HotkeyAction::ToggleRecording => {
            if state.is_recording.load(Ordering::SeqCst) {
                stop_recording(state, audio_capture, tray);
            } else {
                start_recording(state, audio_capture, tray);
            }
        }
        HotkeyAction::CycleLanguage => {
            cycle_language(language_list, current_language, tray);
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

    if let Err(e) = audio_capture.start() {
        error!("Failed to start recording: {}", e);
        return;
    }
    state.is_recording.store(true, Ordering::SeqCst);
    tray.set_recording_state(true);

    let s = ui::tray::tray_strings();
    tray.show_notification(s.app_name(), s.notification_recording_started());
    info!("Recording started - speak now");
}

/// Stop recording.
fn stop_recording(
    state: &AppState,
    audio_capture: &mut AudioCapture,
    tray: &TrayManager,
) {
    state.is_recording.store(false, Ordering::SeqCst);
    if let Err(e) = audio_capture.stop() {
        error!("Failed to stop recording: {}", e);
    }
    tray.set_recording_state(false);

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

    let sample_rate = asr_config.sample_rate;
    let samples: Vec<f32> = if wav_data.len() > 44 {
        let data_start = 44;
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
fn write_wav(path: &PathBuf, samples: &[f32], sample_rate: u32) -> Result<()> {
    use std::io::Write;

    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = samples.len() as u32 * 2;
    let file_size = 36 + data_size;

    let mut file = std::fs::File::create(path)?;

    file.write_all(b"RIFF")?;
    file.write_all(&file_size.to_le_bytes())?;
    file.write_all(b"WAVE")?;

    file.write_all(b"fmt ")?;
    file.write_all(&(16u32).to_le_bytes())?;
    file.write_all(&(1u16).to_le_bytes())?;
    file.write_all(&num_channels.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&bits_per_sample.to_le_bytes())?;

    file.write_all(b"data")?;
    file.write_all(&data_size.to_le_bytes())?;

    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        file.write_all(&i16_sample.to_le_bytes())?;
    }

    Ok(())
}
