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
    // Every method except country_format needs an API key; a free one covers
    // validate, 100 requests a month: https://ibanchecker.cash/api-docs
    let api_key = std::env::var("IBANCHECKER_API_KEY").expect("set IBANCHECKER_API_KEY");
    let client = Client::with_api_key(api_key);

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

Every method except `country_format` needs an API key, and that includes `lookup_bic`, which used to work without one. Without a key the API answers HTTP 401 and the client returns `Error::Authentication`. Request a free key at [ibanchecker.cash/api-docs](https://ibanchecker.cash/api-docs); it arrives by email in seconds. Paid plans with higher quotas are at [ibanchecker.cash/pricing](https://ibanchecker.cash/pricing).

What a key can call follows its plan:

- A free key covers `validate` only, 100 requests a month.
- `validate_bulk` and `lookup_bic` need the Basic plan or above (Basic, Starter, Growth, Enterprise).
- `extract` needs the Growth plan or above (Growth, Enterprise).

A call outside the key's plan gets HTTP 403 with code `PLAN_REQUIRED`, which the client returns as `Error::Api` (see [Error handling](#error-handling)).

A key whose email address has a verified account at [ibanchecker.cash/dashboard](https://ibanchecker.cash/dashboard) can try the methods its plan lacks: `validate_bulk` with up to 10 IBANs per call (100 on a plan that includes it), `lookup_bic`, and `extract` with up to 5,000 characters per call (50,000 on a plan that includes it). The trial applies to any plan that lacks the method, so a Basic key with a verified account can try `extract`. A trial call over the trial size gets HTTP 400 (`Error::BadRequest`) with code `TOO_MANY_IBANS` for `validate_bulk` or `TEXT_TOO_LONG` for `extract`.

`country_format` works without a key, limited to 100 requests an hour per IP; beyond that the API answers HTTP 429 with code `RATE_LIMIT_EXCEEDED`. This hourly keyless limit applies only to country formats.

```rust
let client = Client::with_api_key("YOUR_API_KEY");

let client = Client::builder()
    .api_key(std::env::var("IBANCHECKER_API_KEY").unwrap_or_default())
    .timeout(std::time::Duration::from_secs(3))
    .build()?;

// Country formats only: no key needed.
let client = Client::new();
```

The key is sent as `Authorization: Bearer <key>`.

### How requests are counted

`validate` and `lookup_bic` count one request per call. `validate_bulk` counts one request per IBAN, and `extract` one per IBAN found, with at least one per call. A call that costs more than the requests left this month gets HTTP 429 with code `QUOTA_EXCEEDED` (`Error::RateLimit`), so a bulk call of 50 IBANs with 20 requests left gets 429.

## Methods

| Method | API key | Description |
| --- | --- | --- |
| `validate(iban)` | required, any plan | Validate a single IBAN. Returns a `ValidationResult`. |
| `validate_bulk(ibans)` | required, Basic or above | Validate up to 100 IBANs (10 on a trial). Returns a `BatchResult`. |
| `extract(text)` | required, Growth or above | Find and validate IBANs in free text (up to 50,000 chars; 5,000 on a trial). Returns a `BatchResult`. |
| `country_format(country)` | not needed | IBAN format spec for an ISO country code. Returns a `FormatSpec`. |
| `lookup_bic(bic)` | required, Basic or above | Resolve an 8 or 11 character BIC. Returns a `BankRecord`. |

A key whose plan lacks `validate_bulk`, `extract` or `lookup_bic` can still try it when the key's email address has a verified account; see [Authentication](#authentication).

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

// Needs a key on the Basic plan or above, or a verified account to try it.
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

A malformed IBAN is **not** an error: `validate` returns a `ValidationResult` with `valid` false. `Error` is returned for transport, authentication, plan, quota and server-side problems only.

```rust
use ibanchecker::Error;

match client.lookup_bic("ZZZZZZZZ").await {
    Ok(bank) => println!("{}", bank.bank_name),
    Err(Error::NotFound(_)) => println!("No bank for that BIC"),
    Err(Error::Authentication(_)) => println!("Check your API key"),
    Err(Error::Api(api)) if api.code == "PLAN_REQUIRED" => println!("Not on this plan: {}", api.message),
    Err(Error::RateLimit(api)) => println!("Out of requests: {}", api.message),
    Err(e) => println!("{e}"),
}
```

| Variant | Returned when |
| --- | --- |
| `Error::BadRequest` | HTTP 400, the request was malformed, or a trial call went over the trial size (`TOO_MANY_IBANS`, `TEXT_TOO_LONG`) |
| `Error::Authentication` | HTTP 401, the API key is missing, invalid or inactive; every method except `country_format` needs one |
| `Error::NotFound` | HTTP 404, no such country code or BIC |
| `Error::RateLimit` | HTTP 429: `QUOTA_EXCEEDED` when a call costs more than the key's requests left this month, `RATE_LIMIT_EXCEEDED` when keyless `country_format` calls pass 100 an hour per IP |
| `Error::Api` | HTTP 403 `PLAN_REQUIRED` when the key's plan does not include the method; any other error status, or a body that could not be read |
| `Error::Transport` | the request never reached the API: DNS, TLS, connection, timeout |

Every variant except `Transport` carries an `ApiError` with the status, the machine-readable code and the decoded body, reachable through `Error::api()`, `Error::status()` and `Error::code()`.

The client has no variant of its own for HTTP 403: a `PLAN_REQUIRED` answer arrives as `Error::Api` with `status` 403 and `code` `"PLAN_REQUIRED"`. Its body carries `required_plan` (`"basic"` or `"growth"`) and `upgrade_url`:

```rust
if let Err(Error::Api(api)) = client.extract("Please wire to DE89 3704 0044 0532 0130 00").await {
    if api.code == "PLAN_REQUIRED" {
        let body = api.body.unwrap_or_default();
        println!("Needs the {} plan", body["required_plan"].as_str().unwrap_or_default());
        println!("Upgrade: {}", body["upgrade_url"].as_str().unwrap_or_default());
    }
}
```

A `QUOTA_EXCEEDED` body also carries `retry_after`, the seconds until the quota resets on the 1st of next month (UTC), and `upgrade_url`:

```rust
if let Err(Error::RateLimit(api)) = client.validate("DE89370400440532013000").await {
    if api.code == "QUOTA_EXCEEDED" {
        let body = api.body.unwrap_or_default();
        println!("Monthly quota used, resets in {} seconds", body["retry_after"]);
        println!("Upgrade: {}", body["upgrade_url"].as_str().unwrap_or_default());
    }
}
```

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
