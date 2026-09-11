use std::fmt;

use serde_json::Value;

/// Everything that can go wrong on the way to a result.
///
/// A malformed IBAN is not an error: [`crate::Client::validate`] returns a
/// [`crate::ValidationResult`] with `valid` false. These are returned for
/// transport, authentication, quota and server-side problems only.
#[derive(Debug)]
pub enum Error {
    /// HTTP 400: the request was malformed.
    BadRequest(ApiError),
    /// HTTP 401: the API key is missing, invalid or inactive.
    Authentication(ApiError),
    /// HTTP 404: no such country code or BIC.
    NotFound(ApiError),
    /// HTTP 429: the hourly rate limit or the monthly quota was exceeded.
    RateLimit(ApiError),
    /// Any other error status, or a response body that could not be read.
    Api(ApiError),
    /// The request never reached the API: DNS, TLS, connection or timeout.
    Transport(reqwest::Error),
}

/// What the API said about a failure.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiError {
    /// The HTTP status.
    pub status: u16,
    /// The machine-readable code from the body, such as `BIC_NOT_FOUND`.
    /// Empty when the API sent none.
    pub code: String,
    /// The human-readable reason.
    pub message: String,
    /// The decoded response body, when the API sent one.
    pub body: Option<Value>,
}

impl Error {
    /// The API's own account of the failure, for every variant except
    /// [`Error::Transport`].
    pub fn api(&self) -> Option<&ApiError> {
        match self {
            Error::BadRequest(e)
            | Error::Authentication(e)
            | Error::NotFound(e)
            | Error::RateLimit(e)
            | Error::Api(e) => Some(e),
            Error::Transport(_) => None,
        }
    }

    /// The HTTP status, when the request got far enough to have one.
    pub fn status(&self) -> Option<u16> {
        self.api().map(|e| e.status)
    }

    /// The machine-readable code, when the API sent one.
    pub fn code(&self) -> Option<&str> {
        self.api()
            .map(|e| e.code.as_str())
            .filter(|c| !c.is_empty())
    }

    pub(crate) fn from_status(status: u16, body: Option<Value>) -> Self {
        let field = |name: &str| {
            body.as_ref()
                .and_then(|b| b.get(name))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };

        let message = match field("error") {
            m if m.is_empty() => format!("HTTP {status}"),
            m => m,
        };

        let api = ApiError {
            status,
            code: field("error_code"),
            message,
            body,
        };

        match status {
            400 => Error::BadRequest(api),
            401 => Error::Authentication(api),
            404 => Error::NotFound(api),
            429 => Error::RateLimit(api),
            _ => Error::Api(api),
        }
    }

    pub(crate) fn unreadable(status: u16, message: &str) -> Self {
        Error::Api(ApiError {
            status,
            code: String::new(),
            message: message.to_string(),
            body: None,
        })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Transport(e) => write!(f, "ibanchecker: request failed: {e}"),
            _ => {
                let api = self
                    .api()
                    .expect("every non-transport variant carries an ApiError");
                write!(f, "ibanchecker: {} (HTTP {})", api.message, api.status)
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Transport(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Transport(e)
    }
}

/// The result of every call in this crate.
pub type Result<T> = std::result::Result<T, Error>;
