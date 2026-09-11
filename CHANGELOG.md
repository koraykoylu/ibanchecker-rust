# Changelog

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
