//! Official Rust client for the [ibanchecker.cash](https://ibanchecker.cash)
//! IBAN validation API.
//!
//! Validate IBANs across 92 countries, validate up to 100 IBANs per request,
//! extract IBANs from free text, look up country format specifications and
//! resolve SWIFT/BIC codes.
//!
//! [`Client::validate`], [`Client::validate_bulk`] and [`Client::extract`]
//! need an API key; without one they return [`Error::Authentication`]. A free
//! key covers 100 requests a month and arrives by email in seconds: request it
//! at <https://ibanchecker.cash/api-docs>. Paid plans are at
//! <https://ibanchecker.cash/pricing>. [`Client::country_format`] and
//! [`Client::lookup_bic`] work without a key, limited to 100 requests an hour
//! per IP.
//!
//! ```no_run
//! # async fn run() -> Result<(), ibanchecker::Error> {
//! let api_key = std::env::var("IBANCHECKER_API_KEY").expect("set IBANCHECKER_API_KEY");
//! let client = ibanchecker::Client::with_api_key(api_key);
//!
//! let result = client.validate("DE89 3704 0044 0532 0130 00").await?;
//! if result.valid {
//!     println!("{} {}", result.bank_name, result.bic);
//! } else {
//!     println!("{} ({})", result.error, result.error_code);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! A malformed IBAN is not an error: [`Client::validate`] returns a
//! [`ValidationResult`] with `valid` false and an `error` plus `error_code`
//! explaining why. [`Error`] is returned for transport, authentication, quota
//! and server-side problems only.
//!
//! # Tri-state fields
//!
//! `national_check_valid`, `sepa` and `swift` are `Option<bool>`. The API
//! distinguishes false from absent, so `None` means "not known for this IBAN"
//! rather than "no".
//!
//! # Raw responses
//!
//! Every model keeps the untouched response body in `raw`, so a field added to
//! the API later is reachable without waiting for a client release.

#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations)]

mod client;
mod error;
mod models;

pub use client::{Client, ClientBuilder, DEFAULT_BASE_URL, VERSION};
pub use error::{ApiError, Error, Result};
pub use models::{BankRecord, BatchResult, BbanField, FormatSpec, ValidationResult};
