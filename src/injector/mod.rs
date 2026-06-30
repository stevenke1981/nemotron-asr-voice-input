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
    #[allow(dead_code)]
    UiautomationFailed(String),

    #[error("Clipboard operation failed: {0}")]
    #[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── InjectStrategy ──────────────────────────────────────────────

    #[test]
    fn test_inject_strategy_from_string_valid() {
        assert_eq!(InjectStrategy::from_string("sendinput"), InjectStrategy::SendInput);
        assert_eq!(InjectStrategy::from_string("SendInput"), InjectStrategy::SendInput);
        assert_eq!(InjectStrategy::from_string("SENDINPUT"), InjectStrategy::SendInput);
        assert_eq!(InjectStrategy::from_string("uiautomation"), InjectStrategy::Uiautomation);
        assert_eq!(InjectStrategy::from_string("clipboard"), InjectStrategy::Clipboard);
        assert_eq!(InjectStrategy::from_string("auto"), InjectStrategy::Auto);
    }

    #[test]
    fn test_inject_strategy_from_string_invalid_falls_to_auto() {
        assert_eq!(InjectStrategy::from_string("unknown"), InjectStrategy::Auto);
        assert_eq!(InjectStrategy::from_string(""), InjectStrategy::Auto);
    }

    // ── CompositeInjector ───────────────────────────────────────────

    #[test]
    fn test_composite_injector_empty_text_returns_ok() {
        let mut injector = CompositeInjector::new();
        assert!(injector.inject_text("").is_ok());
    }

    #[test]
    fn test_composite_injector_all_fail_returns_error() {
        struct AlwaysFailingInjector;
        impl TextInjector for AlwaysFailingInjector {
            fn inject_text(&mut self, _: &str) -> Result<(), InjectorError> {
                Err(InjectorError::SendInputFailed("mock failure".into()))
            }
            fn is_available(&self) -> bool { true }
        }

        let mut injector = CompositeInjector {
            strategies: vec![Box::new(AlwaysFailingInjector)],
            current_strategy: InjectStrategy::Auto,
        };
        let result = injector.inject_text("hello");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), InjectorError::AllStrategiesFailed));
    }

    #[test]
    fn test_composite_injector_first_available_succeeds() {
        struct FirstOkInjector;
        impl TextInjector for FirstOkInjector {
            fn inject_text(&mut self, _: &str) -> Result<(), InjectorError> { Ok(()) }
            fn is_available(&self) -> bool { true }
        }

        struct NeverReachedInjector;
        impl TextInjector for NeverReachedInjector {
            fn inject_text(&mut self, _: &str) -> Result<(), InjectorError> {
                panic!("should not be called");
            }
            fn is_available(&self) -> bool { true }
        }

        let mut injector = CompositeInjector {
            strategies: vec![
                Box::new(FirstOkInjector),
                Box::new(NeverReachedInjector),
            ],
            current_strategy: InjectStrategy::Auto,
        };
        assert!(injector.inject_text("hello").is_ok());
    }

    #[test]
    fn test_composite_injector_skips_unavailable() {
        struct UnavailableInjector;
        impl TextInjector for UnavailableInjector {
            fn inject_text(&mut self, _: &str) -> Result<(), InjectorError> {
                panic!("unavailable injector should not be called");
            }
            fn is_available(&self) -> bool { false }
        }

        struct FallbackInjector;
        impl TextInjector for FallbackInjector {
            fn inject_text(&mut self, _: &str) -> Result<(), InjectorError> { Ok(()) }
            fn is_available(&self) -> bool { true }
        }

        let mut injector = CompositeInjector {
            strategies: vec![
                Box::new(UnavailableInjector),
                Box::new(FallbackInjector),
            ],
            current_strategy: InjectStrategy::Auto,
        };
        assert!(injector.inject_text("test").is_ok());
    }

    #[test]
    fn test_composite_available_returns_true_when_any_available() {
        struct Unavailable;
        impl TextInjector for Unavailable {
            fn inject_text(&mut self, _: &str) -> Result<(), InjectorError> { Ok(()) }
            fn is_available(&self) -> bool { false }
        }

        struct Available;
        impl TextInjector for Available {
            fn inject_text(&mut self, _: &str) -> Result<(), InjectorError> { Ok(()) }
            fn is_available(&self) -> bool { true }
        }

        let injector = CompositeInjector {
            strategies: vec![Box::new(Unavailable), Box::new(Available)],
            current_strategy: InjectStrategy::Auto,
        };
        assert!(injector.is_available());
    }

    #[test]
    fn test_composite_available_returns_false_when_none_available() {
        struct NA;
        impl TextInjector for NA {
            fn inject_text(&mut self, _: &str) -> Result<(), InjectorError> { Ok(()) }
            fn is_available(&self) -> bool { false }
        }

        let injector = CompositeInjector {
            strategies: vec![Box::new(NA)],
            current_strategy: InjectStrategy::Auto,
        };
        assert!(!injector.is_available());
    }
}
