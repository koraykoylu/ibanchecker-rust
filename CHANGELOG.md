# Changelog

## 0.1.2

Documentation for the API's plan requirements, live since 27 September 2026.

- Every method except `country_format` needs an API key. `lookup_bic` used to
  work without one; without a key the API now answers 401 and the client
  returns `Error::Authentication`
- What a key can call follows its plan: a free key covers `validate` only,
  100 requests a month; `validate_bulk` and `lookup_bic` need Basic or above;
  `extract` needs Growth or above
- A call outside the key's plan gets 403 `PLAN_REQUIRED` with `required_plan`
  and `upgrade_url` in the body; the client returns it as `Error::Api`, with
  the status and code in its `ApiError`
- A key whose email address has a verified account at
  https://ibanchecker.cash/dashboard can try the methods its plan lacks:
  `validate_bulk` up to 10 IBANs per call, `lookup_bic`, and `extract` up to
  5,000 characters per call. Over the trial size the API answers 400
  `TOO_MANY_IBANS` or `TEXT_TOO_LONG`
- `validate_bulk` counts one request per IBAN and `extract` one per IBAN
  found, at least one per call; a call that costs more than the requests left
  this month gets 429 `QUOTA_EXCEEDED`
- `country_format` still works without a key, limited to 100 requests an hour
  per IP; that hourly limit no longer covers BIC lookups

The client's behaviour does not change.

## 0.1.1

Documentation for the API's key requirement, live since 27 September 2026.
No change in behaviour.

- `validate`, `validate_bulk` and `extract` need an API key; without one the
  API answers 401 and the client returns `Error::Authentication`. The README
  quick start and the crate docs now build the client with a key
- A free key covers 100 requests a month; over the quota the API answers 429
  `QUOTA_EXCEEDED` with `retry_after` and `upgrade_url` in the body
- `country_format` and `lookup_bic` still work without a key, limited to 100
  requests an hour per IP

## 0.1.0

First release.

- `validate`, `validate_bulk`, `extract`, `country_format` and `lookup_bic`,
  all async
- Typed models for every response, with the raw body kept in `raw`
- An `Error` enum for 400, 401, 404, 429, other statuses and transport
  failures, each carrying the status, the code and the decoded body; a
  malformed IBAN is a result with `valid` false, never an error
- Tri-state API fields are `Option<bool>`, so absent stays distinguishable
  from false, and an explicit null reads the same as an absent key
- `rustls` rather than native TLS, so no system OpenSSL is needed
