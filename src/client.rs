use std::time::Duration;

use reqwest::{header, Method};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::{BankRecord, BatchResult, Error, FormatSpec, Result, ValidationResult};

/// The version that goes out in the `User-Agent` header.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The production API.
pub const DEFAULT_BASE_URL: &str = "https://ibanchecker.cash/api/v1";

/// Client for the ibanchecker.cash API.
///
/// Cloning is cheap: the inner HTTP client shares its connection pool.
#[derive(Debug, Clone)]
pub struct Client {
    api_key: Option<String>,
    base_url: String,
    http: reqwest::Client,
}

/// Builds a [`Client`].
#[derive(Debug, Default)]
pub struct ClientBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    timeout: Option<Duration>,
    http: Option<reqwest::Client>,
}

impl ClientBuilder {
    /// Sets the API key. Without one, requests are limited to 100 per hour per
    /// IP.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        self.api_key = if key.is_empty() { None } else { Some(key) };
        self
    }

    /// Points the client at a different host. Any trailing slash is trimmed.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into().trim_end_matches('/').to_string());
        self
    }

    /// Sets the request timeout. Ignored when [`ClientBuilder::http_client`]
    /// supplies a client of its own.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Routes the calls through an application's own HTTP client. It replaces
    /// the timeout and the redirect policy below, so set both on the client
    /// passed in.
    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    /// Builds the client.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Transport`] when the underlying HTTP client cannot be
    /// constructed, which in practice means a TLS backend that failed to load.
    pub fn build(self) -> Result<Client> {
        let http = match self.http {
            Some(http) => http,
            None => reqwest::Client::builder()
                .timeout(self.timeout.unwrap_or(Duration::from_secs(10)))
                // Refusing beats following. reqwest, like net/http, turns a
                // POST into a GET on a 301, as the RFC asks, so an http base
                // URL would reach the https endpoint as a GET and come back
                // 405 Method Not Allowed with nothing to explain it. And a
                // redirect to another host is how an API key travels somewhere
                // it was never meant to go.
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        };

        Ok(Client {
            api_key: self.api_key,
            base_url: self
                .base_url
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            http,
        })
    }
}

impl Client {
    /// A client with no API key, pointed at production.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be built. Use
    /// [`Client::builder`] when that should be handled instead.
    pub fn new() -> Self {
        Self::builder().build().expect("default HTTP client")
    }

    /// A client with an API key, pointed at production.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be built.
    pub fn with_api_key(key: impl Into<String>) -> Self {
        Self::builder()
            .api_key(key)
            .build()
            .expect("default HTTP client")
    }

    /// Starts building a client.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Validates a single IBAN.
    ///
    /// A malformed IBAN is not an error: the result comes back with `valid`
    /// false and an `error` plus `error_code` explaining why.
    pub async fn validate(&self, iban: &str) -> Result<ValidationResult> {
        self.request(Method::POST, "/validate", Some(json!({ "iban": iban })))
            .await
    }

    /// Validates up to 100 IBANs in one request. Results come back in the same
    /// order as the input.
    pub async fn validate_bulk<I, S>(&self, ibans: I) -> Result<BatchResult>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let list: Vec<String> = ibans.into_iter().map(|s| s.as_ref().to_string()).collect();
        self.request(
            Method::POST,
            "/validate/bulk",
            Some(json!({ "ibans": list })),
        )
        .await
    }

    /// Scans free text (emails, invoices) for IBAN-shaped strings and
    /// validates each candidate. Up to 50,000 characters per request.
    pub async fn extract(&self, text: &str) -> Result<BatchResult> {
        self.request(Method::POST, "/extract", Some(json!({ "text": text })))
            .await
    }

    /// The IBAN format specification for an ISO 3166-1 alpha-2 country code,
    /// for example `"DE"`.
    pub async fn country_format(&self, country: &str) -> Result<FormatSpec> {
        let path = format!("/formats/{}", escape(&country.to_lowercase()));
        self.request(Method::GET, &path, None).await
    }

    /// Resolves an 8 or 11 character ISO 9362 BIC to a bank record.
    pub async fn lookup_bic(&self, bic: &str) -> Result<BankRecord> {
        let path = format!("/swift/{}", escape(&bic.to_uppercase()));
        self.request(Method::GET, &path, None).await
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        payload: Option<Value>,
    ) -> Result<T> {
        let mut req = self
            .http
            .request(method, format!("{}{}", self.base_url, path))
            .header(header::ACCEPT, "application/json")
            .header(header::USER_AGENT, format!("ibanchecker-rust/{VERSION}"));

        if let Some(key) = &self.api_key {
            req = req.header(header::AUTHORIZATION, format!("Bearer {key}"));
        }
        if let Some(body) = payload {
            req = req.json(&body);
        }

        let response = req.send().await?;
        let status = response.status();

        // Read before the body is consumed: under Policy::none the redirect
        // response is handed back as-is, and its Location is the only thing
        // that can name where the base URL actually points.
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);

        let text = response.text().await?;

        if status.is_redirection() {
            return Err(Error::unreadable(
                status.as_u16(),
                &format!(
                    "refusing to follow the redirect to {}: set the base URL to that address \
                     (the API is https, and an http base URL redirects)",
                    location.as_deref().unwrap_or("another address")
                ),
            ));
        }

        // Decoded separately from T, so an error status still carries whatever
        // the API said even when it does not match the expected shape.
        let decoded: Option<Value> = if text.trim().is_empty() {
            None
        } else {
            serde_json::from_str(&text).ok()
        };

        if status.is_client_error() || status.is_server_error() {
            return Err(Error::from_status(status.as_u16(), decoded));
        }

        match decoded {
            None if text.trim().is_empty() => Err(Error::unreadable(
                status.as_u16(),
                "the API returned an empty body",
            )),
            None => Err(Error::unreadable(
                status.as_u16(),
                "the API returned a body that is not JSON",
            )),
            Some(value) => serde_json::from_value(value).map_err(|e| {
                Error::unreadable(
                    status.as_u16(),
                    &format!("could not decode the response: {e}"),
                )
            }),
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

/// Percent-encodes everything outside the unreserved set of RFC 3986, so a
/// value cannot climb out of its path segment.
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
