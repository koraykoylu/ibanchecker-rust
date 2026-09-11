//! Official Rust client for the [ibanchecker.cash](https://ibanchecker.cash)
//! IBAN validation API.
//!
//! Validate IBANs across 92 countries, validate up to 100 IBANs per request,
//! extract IBANs from free text, look up country format specifications and
//! resolve SWIFT/BIC codes.
//!
//! An API key is optional. Without one, requests are limited to 100 per hour
//! per IP. Get a free key at <https://ibanchecker.cash/api-docs>.
//!
//! ```no_run
//! # async fn run() -> Result<(), ibanchecker::Error> {
//! let client = ibanchecker::Client::new();
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
