use super::{
    SupervisorClient, SupervisorRuntimeInfo, SUPERVISOR_REQUEST_SOURCE, SUPERVISOR_REQUEST_TIMEOUT,
};
use crate::error::{AppError, AppErrorCode, AppResult};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;

enum RequestFailure {
    Retryable(AppError),
    Fatal(AppError),
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
    code: Option<AppErrorCode>,
}

impl SupervisorClient {
    pub(super) fn get_json<TResponse>(&self, route: &str) -> AppResult<TResponse>
    where
        TResponse: DeserializeOwned,
    {
        for attempt in 0..2 {
            let runtime = self.ensure_runtime()?;

            match self.send_get(&runtime, route) {
                Ok(value) => return Ok(value),
                Err(RequestFailure::Fatal(message)) => return Err(message),
                Err(RequestFailure::Retryable(message)) if attempt == 1 => return Err(message),
                Err(RequestFailure::Retryable(_)) => {
                    if self.ping_runtime(&runtime).is_ok() {
                        continue;
                    }

                    self.invalidate_runtime(
                        Some(&runtime),
                        &format!("GET /{route} failed and supervisor health check also failed"),
                    );
                }
            }
        }

        Err(AppError::supervisor("supervisor GET request failed"))
    }

    pub(super) fn request_json<TRequest, TResponse>(
        &self,
        route: &str,
        payload: &TRequest,
    ) -> AppResult<TResponse>
    where
        TRequest: Serialize,
        TResponse: DeserializeOwned,
    {
        self.request_json_with_timeout(route, payload, SUPERVISOR_REQUEST_TIMEOUT)
    }

    pub(super) fn request_json_with_timeout<TRequest, TResponse>(
        &self,
        route: &str,
        payload: &TRequest,
        timeout: Duration,
    ) -> AppResult<TResponse>
    where
        TRequest: Serialize,
        TResponse: DeserializeOwned,
    {
        for attempt in 0..2 {
            let runtime = self.ensure_runtime()?;

            match self.send_json(&runtime, route, payload, timeout) {
                Ok(value) => return Ok(value),
                Err(RequestFailure::Fatal(message)) => return Err(message),
                Err(RequestFailure::Retryable(message)) if attempt == 1 => return Err(message),
                Err(RequestFailure::Retryable(message)) => {
                    if self.ping_runtime(&runtime).is_ok() {
                        continue;
                    }

                    self.invalidate_runtime(
                        Some(&runtime),
                        &format!(
                            "POST /{route} failed and supervisor health check also failed: {}",
                            message.message
                        ),
                    );
                }
            }
        }

        Err(AppError::supervisor("supervisor request failed"))
    }

    fn send_get<TResponse>(
        &self,
        runtime: &SupervisorRuntimeInfo,
        route: &str,
    ) -> Result<TResponse, RequestFailure>
    where
        TResponse: DeserializeOwned,
    {
        let url = format!("http://127.0.0.1:{}/{}", runtime.port, route);
        let response = self
            .inner
            .http_client
            .get(&url)
            .timeout(SUPERVISOR_REQUEST_TIMEOUT)
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", SUPERVISOR_REQUEST_SOURCE)
            .send()
            .map_err(|error| {
                RequestFailure::Retryable(AppError::supervisor(format!(
                    "failed to reach Project Commander supervisor: {error}"
                )))
            })?;

        let status = response.status();

        if !status.is_success() {
            let raw_message = response
                .text()
                .unwrap_or_else(|_| "Project Commander supervisor returned an error".to_string());
            let app_error = serde_json::from_str::<ErrorResponse>(&raw_message)
                .map(|payload| match payload.code {
                    Some(code) => AppError::new(code, payload.error),
                    None => AppError::from_status(status.as_u16(), payload.error),
                })
                .unwrap_or_else(|_| AppError::from_status(status.as_u16(), raw_message));

            return Err(
                if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                    RequestFailure::Retryable(app_error)
                } else {
                    RequestFailure::Fatal(app_error)
                },
            );
        }

        let envelope: serde_json::Value = response.json().map_err(|error| {
            RequestFailure::Retryable(AppError::supervisor(format!(
                "failed to decode supervisor GET response: {error}"
            )))
        })?;

        let data = envelope
            .get("data")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        serde_json::from_value::<TResponse>(data).map_err(|error| {
            RequestFailure::Retryable(AppError::supervisor(format!(
                "failed to decode supervisor GET response data: {error}"
            )))
        })
    }

    fn send_json<TRequest, TResponse>(
        &self,
        runtime: &SupervisorRuntimeInfo,
        route: &str,
        payload: &TRequest,
        timeout: Duration,
    ) -> Result<TResponse, RequestFailure>
    where
        TRequest: Serialize,
        TResponse: DeserializeOwned,
    {
        let url = format!("http://127.0.0.1:{}/{}", runtime.port, route);
        let response = self
            .inner
            .http_client
            .post(&url)
            .timeout(timeout)
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", SUPERVISOR_REQUEST_SOURCE)
            .json(payload)
            .send()
            .map_err(|error| {
                RequestFailure::Retryable(AppError::supervisor(format!(
                    "failed to reach Project Commander supervisor: {error}"
                )))
            })?;

        let status = response.status();

        if !status.is_success() {
            let raw_message = response
                .text()
                .unwrap_or_else(|_| "Project Commander supervisor returned an error".to_string());
            let app_error = serde_json::from_str::<ErrorResponse>(&raw_message)
                .map(|payload| match payload.code {
                    Some(code) => AppError::new(code, payload.error),
                    None => AppError::from_status(status.as_u16(), payload.error),
                })
                .unwrap_or_else(|_| AppError::from_status(status.as_u16(), raw_message));

            return Err(
                if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                    RequestFailure::Retryable(app_error)
                } else {
                    RequestFailure::Fatal(app_error)
                },
            );
        }

        let envelope: serde_json::Value = response.json().map_err(|error| {
            RequestFailure::Retryable(AppError::supervisor(format!(
                "failed to decode supervisor response: {error}"
            )))
        })?;

        let data = envelope
            .get("data")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        serde_json::from_value::<TResponse>(data).map_err(|error| {
            RequestFailure::Retryable(AppError::supervisor(format!(
                "failed to decode supervisor response data: {error}"
            )))
        })
    }
}
