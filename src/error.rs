use std::fmt;

pub type Result<T> = std::result::Result<T, SnipError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    Io,
    Usage,
    NoLibrary,
    NotFound,
    Conflict,
    Validation,
}

impl ErrorKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::Io => "io_error",
            Self::Usage => "usage_error",
            Self::NoLibrary => "no_library",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Validation => "validation_error",
        }
    }

    pub fn exit_code(self) -> i32 {
        match self {
            Self::Io => 1,
            Self::Usage => 2,
            Self::NoLibrary | Self::NotFound => 3,
            Self::Conflict => 4,
            Self::Validation => 5,
        }
    }
}

#[derive(Debug)]
pub struct SnipError {
    pub kind: ErrorKind,
    pub message: String,
    pub hint: Option<String>,
}

impl SnipError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            hint: None,
        }
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Io, message)
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Usage, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, message)
    }

    pub fn no_library(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NoLibrary, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Conflict, message)
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Validation, message)
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl fmt::Display for SnipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SnipError {}

impl From<std::io::Error> for SnipError {
    fn from(error: std::io::Error) -> Self {
        Self::io(error.to_string())
    }
}

impl From<toml::de::Error> for SnipError {
    fn from(error: toml::de::Error) -> Self {
        Self::validation(error.to_string())
    }
}

impl From<toml::ser::Error> for SnipError {
    fn from(error: toml::ser::Error) -> Self {
        Self::validation(error.to_string())
    }
}

impl From<serde_json::Error> for SnipError {
    fn from(error: serde_json::Error) -> Self {
        Self::validation(error.to_string())
    }
}
