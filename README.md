# ibanchecker

Official Rust client for the [ibanchecker.cash](https://ibanchecker.cash) IBAN validation API.

Validate IBANs across 92 countries, validate up to 100 IBANs per request, extract IBANs from free text, look up country format specifications, and resolve SWIFT/BIC codes. No IBAN data is stored or logged; all validation runs in memory at the edge.

[![crates.io](https://img.shields.io/crates/v/ibanchecker.svg)](https://crates.io/crates/ibanchecker)
[![docs.rs](https://docs.rs/ibanchecker/badge.svg)](https://docs.rs/ibanchecker)

## Install

```bash
cargo add ibanchecker
```

Requires Rust 1.88 or newer. The client is async and built on `reqwest` with `rustls`, so it needs no system OpenSSL.

## Quick start

```rust
use ibanchecker::Client;

#[tokio::main]
async fn main() -> Result<(), ibanchecker::Error> {
    let client = Client::new(); // no API key needed for light use (100 requests/hour per IP)

    let result = client.validate("DE89 3704 0044 0532 0130 00").await?;

    if result.valid {
        println!("{}", result.country_name); // Germany
        println!("{}", result.bank_name);    // Commerzbank AG Cologne
        println!("{}", result.bic);          // COBADEFFXXX
    } else {
        println!("{} ({})", result.error, result.error_code);
    }

    Ok(())
}
```

## Authentication

An API key is optional. Without one, requests are limited to 100 per hour per IP. With a key, requests count against your plan quota. Get a free key at [ibanchecker.cash/api-docs](https://ibanchecker.cash/api-docs).

```rust
let client = Client::with_api_key("iban_your_api_key");

let client = Client::builder()
    .api_key(std::env::var("IBANCHECKER_API_KEY").unwrap_or_default())
    .timeout(std::time::Duration::from_secs(3))
    .build()?;
```

## Methods

| Method | Description |
| --- | --- |
| `validate(iban)` | Validate a single IBAN. Returns a `ValidationResult`. |
| `validate_bulk(ibans)` | Validate up to 100 IBANs. Returns a `BatchResult`. |
| `extract(text)` | Find and validate IBANs in free text (up to 50,000 chars). Returns a `BatchResult`. |
| `country_format(country)` | IBAN format spec for an ISO country code. Returns a `FormatSpec`. |
| `lookup_bic(bic)` | Resolve an 8 or 11 character BIC. Returns a `BankRecord`. |

### Bulk validation

```rust
let batch = client
    .validate_bulk(["DE89370400440532013000", "GB29NWBK60161331926819", "XX00"])
    .await?;

println!("{} of {} valid", batch.valid_count, batch.count);

for result in &batch.results { // results come back in input order
    println!("{} {}", result.iban, result.valid);
}
```

### Extract from text

```rust
let batch = client
    .extract("Please wire to DE89 3704 0044 0532 0130 00 by Friday.")
    .await?;

for result in &batch.results {
    println!("{} {}", result.iban, result.bank_name);
}
```

### Country format and BIC lookup

```rust
let spec = client.country_format("DE").await?;
println!("{} {}", spec.length, spec.example); // 22 DE89370400440532013000

for field in &spec.bban_fields {
    println!("{} ({})", field.label, field.length); // BLZ (8), Account No. (10)
}

let bank = client.lookup_bic("DEUTDEFF").await?;
println!("{} {}", bank.bank_name, bank.city); // Deutsche Bank AG Frankfurt  FRANKFURT AM MAIN
```

### Tri-state fields are `Option<bool>`

`national_check_valid`, `sepa` and `swift` are `Option<bool>`. The API distinguishes false from absent, so `None` means "not known for this IBAN" rather than "no".

```rust
let result = client.validate("DE84100100100532013000").await?;

if result.valid && result.national_check_valid == Some(false) {
    println!("Valid IBAN, but the account number looks mistyped.");
}
```

`national_check_valid` reports a domestic account check digit run on top of the ISO 13616 checksum, such as Germany's per-bank Prüfziffer or the UK sort-code and account modulus check. It is advisory: an IBAN with `valid` true is a valid IBAN whatever this says.

## Error handling

A malformed IBAN is **not** an error: `validate` returns a `ValidationResult` with `valid` false. `Error` is returned for transport, authentication, quota and server-side problems only.

```rust
use ibanchecker::Error;

match client.lookup_bic("ZZZZZZZZ").await {
    Ok(bank) => println!("{}", bank.bank_name),
    Err(Error::NotFound(_)) => println!("No bank for that BIC"),
    Err(Error::RateLimit(api)) => println!("Slow down: {}", api.message),
    Err(Error::Authentication(_)) => println!("Check your API key"),
    Err(e) => println!("{e}"),
}
```

| Variant | Returned when |
| --- | --- |
| `Error::BadRequest` | HTTP 400, the request was malformed |
| `Error::Authentication` | HTTP 401, the API key is missing, invalid or inactive |
| `Error::NotFound` | HTTP 404, no such country code or BIC |
| `Error::RateLimit` | HTTP 429, hourly limit or monthly quota exceeded |
| `Error::Api` | any other error status, or a body that could not be read |
| `Error::Transport` | the request never reached the API: DNS, TLS, connection, timeout |

Every variant except `Transport` carries an `ApiError` with the status, the machine-readable code and the decoded body, reachable through `Error::api()`, `Error::status()` and `Error::code()`.

## Your own HTTP client

```rust
let client = Client::builder()
    .http_client(my_reqwest_client)
    .build()?;
```

That replaces the timeout and the redirect policy below, so set both on the client you pass in.

### Redirects are refused, not followed

The default client stops at a redirect and returns an error naming the target. That is deliberate, for two reasons. `reqwest` turns a POST into a GET on a 301, as the RFC asks, so an `http://` base URL would reach the https endpoint as a GET and come back `405 Method Not Allowed` with nothing to explain it. And a redirect to another host is how an API key travels somewhere it was never meant to go.

Use an `https` base URL and the situation does not arise.

## Raw responses

Every model keeps the untouched response body in `raw`, so a field added to the API later is reachable without waiting for a client release.

```rust
let result = client.validate("DE89370400440532013000").await?;
println!("{}", result.raw["transfer_type"]); // "SEPA+SWIFT"
```

## Tests

```bash
cargo test
```

The suite runs against `wiremock`, so it needs no network.

## Links

- Website: https://ibanchecker.cash
- API documentation: https://ibanchecker.cash/api-docs
- OpenAPI spec: https://ibanchecker.cash/openapi.json
- Free online tools: https://ibanchecker.cash/tools

Clients for other languages: [Python](https://pypi.org/project/ibanchecker/), [PHP](https://packagist.org/packages/ibanchecker/client), [JavaScript](https://www.npmjs.com/package/@ibanchecker/client), [Ruby](https://rubygems.org/gems/ibanchecker), [Go](https://pkg.go.dev/github.com/koraykoylu/ibanchecker-go), and an [MCP server](https://www.npmjs.com/package/@ibanchecker/mcp).

## License

MIT
