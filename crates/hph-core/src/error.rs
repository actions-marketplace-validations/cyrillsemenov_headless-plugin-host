use thiserror::Error;

#[derive(Debug, Error)]
pub enum HostError {
    #[error("Plugin load error: {0}")]
    LoadError(String),
    #[error("Plugin initialization error: {0}")]
    InitError(String),
    #[error("Plugin activation error: {0}")]
    ActivationError(String),
    #[error("Plugin processing error: {0}")]
    ProcessError(String),
    #[error("State error: {0}")]
    StateError(String),
    #[error("GUI error: {0}")]
    GuiError(String),
    #[error("Unsupported feature or extension: {0}")]
    Unsupported(String),
}

pub type HostResult<T> = Result<T, HostError>;
