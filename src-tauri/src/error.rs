use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorCode {
    InvalidInput,
    NotFound,
    Conflict,
    Database,
    Supervisor,
    Io,
    Internal,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: AppErrorCode,
    pub message: String,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: AppErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::InvalidInput, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::NotFound, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::Conflict, message)
    }

    pub fn database(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::Database, message)
    }

    pub fn supervisor(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::Supervisor, message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::Io, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(AppErrorCode::Internal, message)
    }

    pub fn from_status(status: u16, message: impl Into<String>) -> Self {
        let message = message.into();

        match status {
            400 => Self::invalid_input(message),
            404 => Self::not_found(message),
            409 => Self::conflict(message),
            401 | 403 | 429 | 500..=599 => Self::supervisor(message),
            _ => Self::internal(message),
        }
    }

    fn classify_message(message: &str) -> AppErrorCode {
        let lower = message.to_ascii_lowercase();

        if lower.contains("query returned no rows") || lower.contains("not found") {
            return AppErrorCode::NotFound;
        }

        if lower.contains("already exists") {
            return AppErrorCode::Conflict;
        }

        if lower.contains("is required")
            || lower.contains("must be")
            || lower.contains("must exist")
            || lower.contains("cannot ")
            || lower.contains("does not belong")
            || lower.contains("invalid ")
        {
            return AppErrorCode::InvalidInput;
        }

        if lower.contains("supervisor")
            || lower.contains("terminal")
            || lower.contains("session/")
            || lower.contains("helper was not found")
        {
            return AppErrorCode::Supervisor;
        }

        if lower.contains("database")
            || lower.contains("sqlite")
            || lower.contains("migration")
            || lower.contains("launch profile")
            || lower.contains("project")
            || lower.contains("work item")
            || lower.contains("document")
            || lower.contains("worktree")
        {
            return AppErrorCode::Database;
        }

        if lower.contains("directory")
            || lower.contains("file")
            || lower.contains("path")
            || lower.contains("read")
            || lower.contains("write")
        {
            return AppErrorCode::Io;
        }

        AppErrorCode::Internal
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for AppError {}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        let code = Self::classify_message(&message);
        Self::new(code, message)
    }
}

impl From<&str> for AppError {
    fn from(message: &str) -> Self {
        Self::from(message.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_string_errors() {
        assert_eq!(
            AppError::from("project name is required").code,
            AppErrorCode::InvalidInput
        );
        assert_eq!(
            AppError::from("a project with that root folder already exists").code,
            AppErrorCode::Conflict
        );
        assert_eq!(
            AppError::from("failed to load created project: Query returned no rows").code,
            AppErrorCode::NotFound
        );
        assert_eq!(
            AppError::from("failed to reach Project Commander supervisor: connection refused").code,
            AppErrorCode::Supervisor
        );
    }

    #[test]
    fn classifies_http_status_errors() {
        assert_eq!(
            AppError::from_status(400, "bad request").code,
            AppErrorCode::InvalidInput
        );
        assert_eq!(
            AppError::from_status(404, "missing").code,
            AppErrorCode::NotFound
        );
        assert_eq!(
            AppError::from_status(409, "duplicate").code,
            AppErrorCode::Conflict
        );
        assert_eq!(
            AppError::from_status(503, "down").code,
            AppErrorCode::Supervisor
        );
    }
}
