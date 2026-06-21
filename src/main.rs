mod audio;
mod asr;
mod config;
mod convert;
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
use hotkey::register::format_hotkey;
use hotkey::HotkeyManager;
use injector::{CompositeInjector, InjectStrategy, TextInjector};
use asr::{AsrConfig, TranscriptResult};
use ui::strings::{Strings, UiLang};
use ui::tray::{TrayAction, TrayManager};
use ui::gui::app::spawn_gui;
use ui::gui::state::{GuiAction, GuiSnapshot, TranscriptEntry};


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
    register_hotkey(
        &mut hotkey_manager,
        HotkeyAction::PushToTalk,
        app_config.hotkey.ptt_modifiers,
        app_config.hotkey.ptt_vk,
    );
    // Store PTT VK for key-up detection
    *state.ptt_vk.lock().unwrap() = app_config.hotkey.ptt_vk;

    // Initialize the conversion mode from config (also set runtime static)
    let initial_mode = convert::ConversionMode::from_config(&app_config.conversion.mode);
    *state.conversion_mode.lock().unwrap() = initial_mode;
    config::settings::RUNTIME_CONVERSION_MODE.store(initial_mode.index() as u8, std::sync::atomic::Ordering::SeqCst);

    let toggle_reg = hotkey_manager.actual_key(HotkeyAction::ToggleRecording)
        .map(|(m, v)| format_hotkey(m, v))
        .unwrap_or_else(|| format_hotkey(app_config.hotkey.toggle_modifiers, app_config.hotkey.toggle_vk));
    let lang_reg = hotkey_manager.actual_key(HotkeyAction::CycleLanguage)
        .map(|(m, v)| format_hotkey(m, v))
        .unwrap_or_else(|| format_hotkey(app_config.hotkey.lang_modifiers, app_config.hotkey.lang_vk));
    let flush_reg = hotkey_manager.actual_key(HotkeyAction::Flush)
        .map(|(m, v)| format_hotkey(m, v))
        .unwrap_or_else(|| format_hotkey(app_config.hotkey.flush_modifiers, app_config.hotkey.flush_vk));
    let ptt_reg = hotkey_manager.actual_key(HotkeyAction::PushToTalk)
        .map(|(m, v)| format_hotkey(m, v))
        .unwrap_or_else(|| format_hotkey(app_config.hotkey.ptt_modifiers, app_config.hotkey.ptt_vk));
    info!("Hotkeys registered: ToggleRecording({}), CycleLanguage({}), Flush({}), PushToTalk({})",
        toggle_reg, lang_reg, flush_reg, ptt_reg);

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

    // ── GUI initialization ──────────────────────────────────
    let gui_snapshot = Arc::new(std::sync::Mutex::new(GuiSnapshot {
        is_recording: false,
        current_language: app_config.language.language.clone(),
        conversion_mode: app_config.conversion.mode.clone(),
        latest_final_text: String::new(),
        latest_partial_text: String::new(),
        history: Vec::new(),
        show_settings_requested: false,
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
    let _gui_handle = spawn_gui(
        gui_snapshot.clone(),
        gui_snapshot_rx,
        gui_action_tx.clone(),
        show_overlay.clone(),
        initial_pos,
        initial_size,
        Some(app_config.ui.theme.clone()),
    );

    // ── Audio capture processing thread ──────────────────────────────
    let gui_snapshot_for_audio = gui_snapshot.clone();
    let gui_snapshot_tx_for_audio = gui_snapshot_tx.clone();
    let audio_state = state.clone();
    let audio_tx = transcript_tx.clone();
    let audio_ringbuf = audio_capture.ringbuf().clone();
    let asr_config_clone = asr_config.clone();
    let _audio_language = current_language.clone();
    let capture_sample_rate = audio_capture.capture_rate();

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
            let mut resample_buf = vec![0.0f32; chunk_target];
            let mut last_text = String::new();
            let mut last_vad = config::settings::RUNTIME_VAD_ENABLED.load(Ordering::SeqCst);
            let mut was_recording = false;

            info!(
                "Audio processing: capture {} Hz → target {} Hz, chunk {} → {} samples",
                capture_sample_rate, target_rate, chunk_capture, chunk_target
            );

            while audio_state.is_running.load(Ordering::SeqCst) {
                let is_recording = audio_state.is_recording.load(Ordering::SeqCst);

                // Detect recording start → reset dedup state so each session is clean
                if is_recording && !was_recording {
                    last_text.clear();
                }
                was_recording = is_recording;

                if !is_recording {
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }

                let available = audio_ringbuf.len();
                if available < chunk_capture {
                    std::thread::sleep(Duration::from_millis(20));
                    continue;
                }

                let to_read = capture_buf.len().min(available);
                let read = audio_ringbuf.pop_slice(&mut capture_buf[..to_read]);
                if read == 0 {
                    continue;
                }

                // Resample if needed (simple linear interpolation)
                let feed_data: &[f32] = if capture_sample_rate == target_rate {
                    &capture_buf[..read]
                } else {
                    let input = &capture_buf[..read];
                    let input_len = input.len();
                    let output_len = resample_buf.len().min(
                        (input_len as u64 * target_rate as u64 / capture_sample_rate as u64) as usize,
                    );
                    if output_len == 0 { continue; }
                    let ratio = input_len as f64 / output_len as f64;
                    for i in 0..output_len {
                        let src = i as f64 * ratio;
                        let src_i = src.floor() as usize;
                        let frac = src - src_i as f64;
                        let a = input[src_i.min(input_len - 1)] as f64;
                        let b = input[(src_i + 1).min(input_len - 1)] as f64;
                        resample_buf[i] = (a + frac * (b - a)) as f32;
                        resample_buf[i] = resample_buf[i].clamp(-1.0, 1.0);
                    }
                    &resample_buf[..output_len]
                };

                // Check for runtime VAD toggle
                let current_vad = config::settings::RUNTIME_VAD_ENABLED.load(Ordering::SeqCst);
                if current_vad != last_vad {
                    last_vad = current_vad;
                    let _ = engine.set_vad(current_vad);
                }

                if let Err(e) = engine.feed_audio(feed_data) {
                    tracing::debug!("ASR feed error: {}", e);
                    continue;
                }

                match engine.get_transcript() {
                    Ok(result) => {
                        if !result.text.is_empty() && (result.is_final || result.text != last_text) {
                            // Only update last_text for NEW partial text (not for is_final
                            // duplicates, so the next utterance can still send its first partial)
                            if result.text != last_text {
                                last_text = result.text.clone();
                            }
                            // Update shared last_transcript for injection on stop
                            *audio_state.last_transcript.lock().unwrap() = result.text.clone();
                            // Send snapshot to GUI (before moving result)
                            let result_clone = result.clone();
                            if audio_tx.send(result).is_err() {
                                break;
                            }
                            let mut snap = gui_snapshot_for_audio.lock().unwrap();
                            let trimmed = result_clone.text.trim().to_string();
                            if result_clone.is_final {
                                snap.latest_final_text = trimmed.clone();
                                snap.latest_partial_text.clear();
                                snap.history.push(TranscriptEntry {
                                    text: trimmed,
                                    timestamp: simple_timestamp(),
                                    language: asr_config_clone.language.clone(),
                                });
                            } else {
                                snap.latest_partial_text = trimmed;
                            }
                            snap.is_recording = audio_state.is_recording.load(Ordering::SeqCst);
                            let _ = gui_snapshot_tx_for_audio.send(snap.clone());
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
    // Initialize runtime VAD flag from config
    config::settings::RUNTIME_VAD_ENABLED.store(app_config.asr.use_vad, Ordering::SeqCst);

    // Initialize Chinese text converters
    if let Err(e) = convert::init_converters() {
        info!("Chinese text converter not available: {} (continuing without conversion)", e);
    } else {
        info!("Chinese text converter initialized");
    }

    info!(
        "Ready. Press {} to start/stop recording, {} for push-to-talk. Language: {}",
        toggle_reg, ptt_reg, app_config.language.language
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
                // ── PTT key-up monitoring ────────────────────────────
                if state.is_ptt_mode.load(Ordering::SeqCst) {
                    let vk = *state.ptt_vk.lock().unwrap();
                    let key_down = windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk as i32) < 0;
                    if !key_down {
                        // Key released — stop recording + inject
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
                                    // Apply conversion mode (read runtime static for live updates)
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
                            let _ = gui_action_tx.send(GuiAction::ShowOverlay(show));
                        }
                        TrayAction::Exit => {
                            info!("Tray: Exit requested");
                            state.is_running.store(false, Ordering::SeqCst);
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
                            if let Err(e) = cfg.save(&cli.config) {
                                error!("Failed to save config: {}", e);
                            } else {
                                // Update runtime settings
                                *current_language.lock().unwrap() = cfg.language.language.clone();
                                config::settings::RUNTIME_VAD_ENABLED.store(cfg.asr.use_vad, Ordering::SeqCst);
                                config::settings::RUNTIME_CONVERSION_MODE.store(
                                    convert::ConversionMode::from_config(&cfg.conversion.mode).index() as u8,
                                    Ordering::SeqCst,
                                );
                                *state.conversion_mode.lock().unwrap() = convert::ConversionMode::from_config(&cfg.conversion.mode);
                                // Update GUI snapshot
                                let _snap = gui_snapshot.lock().unwrap();
                                // snapshot fields are read-only for now
                                info!("Config saved and runtime settings updated");
                            }
                        }
                        GuiAction::ShowOverlay(show) => {
                            info!("GUI: Overlay toggled: {}", show);
                        }
                        GuiAction::DeleteHistoryEntry(idx) => {
                            let snap = gui_snapshot.lock().unwrap();
                            if idx < snap.history.len() {
                                // Can't remove from locked snapshot easily;
                                // this will be handled properly in Phase 2
                            }
                        }
                        GuiAction::ClearHistory => {
                            gui_snapshot.lock().unwrap().history.clear();
                        }
                        GuiAction::Exit => {
                            info!("GUI: Exit requested");
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
    state.is_recording.store(false, Ordering::SeqCst);
    if let Err(e) = audio_capture.stop() {
        error!("Failed to stop recording: {}", e);
        tray.show_notification("Error", &format!("Failed to stop recording: {}", e));
    }
    tray.set_recording_state(false);

    // Inject the last non-empty transcript (for results that weren't final)
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
