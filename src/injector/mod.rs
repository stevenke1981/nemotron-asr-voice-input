pub mod clipboard;
pub mod sendinput;
pub mod uiautomation;

pub use clipboard::ClipboardInjector;
pub use sendinput::SendInputInjector;
pub use uiautomation::UiautomationInjector;

use thiserror::Error;

/// Text injector unified interface.
pub trait TextInjector: Send {
    /// Inject text into the focused window.
    fn inject_text(&mut self, text: &str) -> Result<(), InjectorError>;

    /// Check if this injector is currently available.
    fn is_available(&self) -> bool;
}

/// Injection strategy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InjectStrategy {
    Uiautomation,
    SendInput,
    Clipboard,
    Auto,
}

impl InjectStrategy {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "uiautomation" => Self::Uiautomation,
            "sendinput" => Self::SendInput,
            "clipboard" => Self::Clipboard,
            _ => Self::Auto,
        }
    }
}

/// Injector errors.
#[derive(Error, Debug)]
pub enum InjectorError {
    #[error("SendInput failed: {0}")]
    SendInputFailed(String),

    #[error("UIAutomation failed: {0}")]
    UiautomationFailed(String),

    #[error("Clipboard operation failed: {0}")]
    ClipboardFailed(String),

    #[error("All injection strategies failed")]
    AllStrategiesFailed,

    #[error("Injector not available")]
    NotAvailable,
}

/// Composite injector that tries strategies in order.
pub struct CompositeInjector {
    strategies: Vec<Box<dyn TextInjector>>,
    current_strategy: InjectStrategy,
}

impl CompositeInjector {
    /// Create a new composite injector with default strategies.
    pub fn new() -> Self {
        Self {
            strategies: vec![
                Box::new(UiautomationInjector::new()),
                Box::new(SendInputInjector::new()),
                Box::new(ClipboardInjector::new()),
            ],
            current_strategy: InjectStrategy::Auto,
        }
    }

    /// Create with a specific strategy.
    pub fn with_strategy(strategy: InjectStrategy) -> Self {
        match strategy {
            InjectStrategy::Uiautomation => Self {
                strategies: vec![Box::new(UiautomationInjector::new())],
                current_strategy: strategy,
            },
            InjectStrategy::SendInput => Self {
                strategies: vec![Box::new(SendInputInjector::new())],
                current_strategy: strategy,
            },
            InjectStrategy::Clipboard => Self {
                strategies: vec![Box::new(ClipboardInjector::new())],
                current_strategy: strategy,
            },
            InjectStrategy::Auto => Self::new(),
        }
    }
}

impl TextInjector for CompositeInjector {
    fn inject_text(&mut self, text: &str) -> Result<(), InjectorError> {
        if text.is_empty() {
            return Ok(());
        }

        for (i, injector) in self.strategies.iter_mut().enumerate() {
            if !injector.is_available() {
                continue;
            }
            match injector.inject_text(text) {
                Ok(()) => {
                    // If not the first strategy, move it to the front
                    if i > 0 {
                        // This strategy worked - keep using it
                        self.current_strategy = match i {
                            0 => InjectStrategy::Uiautomation,
                            1 => InjectStrategy::SendInput,
                            _ => InjectStrategy::Clipboard,
                        };
                    }
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!("Injector strategy {} failed: {}", i, e);
                    continue;
                }
            }
        }

        Err(InjectorError::AllStrategiesFailed)
    }

    fn is_available(&self) -> bool {
        self.strategies.iter().any(|s| s.is_available())
    }
}

impl Default for CompositeInjector {
    fn default() -> Self {
        Self::new()
    }
}
