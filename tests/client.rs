use ibanchecker::{Client, Error};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

/// Starts a server that replays one response for anything, and a client
/// pointed at it.
async fn server_replying(status: u16, body: serde_json::Value) -> (MockServer, Client) {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;
    let client = Client::builder().base_url(server.uri()).build().unwrap();
    (server, client)
}

async fn server_replying_raw(status: u16, body: &str) -> (MockServer, Client) {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(status).set_body_string(body))
        .mount(&server)
        .await;
    let client = Client::builder().base_url(server.uri()).build().unwrap();
    (server, client)
}

async fn last_request(server: &MockServer) -> Request {
    server
        .received_requests()
        .await
        .expect("the server records requests")
        .pop()
        .expect("the client made a request")
}

#[tokio::test]
async fn validate_returns_a_populated_result() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/validate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "valid": true,
            "iban": "DE89370400440532013000",
            "formatted": "DE89 3704 0044 0532 0130 00",
            "country": "DE",
            "country_name": "Germany",
            "bank_name": "Commerzbank AG Cologne",
            "bic": "COBADEFFXXX",
            "bank_city": "Köln",
            "sepa": true,
            "national_check_valid": true
        })))
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let result = client
        .validate("DE89 3704 0044 0532 0130 00")
        .await
        .unwrap();

    assert!(result.valid);
    assert_eq!(result.country_name, "Germany");
    assert_eq!(result.bic, "COBADEFFXXX");
    assert_eq!(result.bank_city, "Köln");
    assert_eq!(result.sepa, Some(true));
    assert_eq!(result.national_check_valid, Some(true));
    assert_eq!(result.raw["iban"], "DE89370400440532013000");

    let request = last_request(&server).await;
    assert_eq!(request.url.path(), "/validate");
    assert_eq!(
        request.body_json::<serde_json::Value>().unwrap(),
        json!({ "iban": "DE89 3704 0044 0532 0130 00" })
    );
    assert_eq!(
        request.headers.get("content-type").unwrap(),
        "application/json"
    );
}

#[tokio::test]
async fn a_malformed_iban_is_a_result_not_an_error() {
    let (_server, client) = server_replying(
        200,
        json!({
            "valid": false,
            "iban": "XX00",
            "country": "XX",
            "error": "\"XX\" is not a recognized IBAN country code.",
            "error_code": "INVALID_COUNTRY"
        }),
    )
    .await;

    let result = client.validate("XX00").await.unwrap();

    assert!(!result.valid);
    assert_eq!(result.error_code, "INVALID_COUNTRY");
    assert_eq!(result.national_check_valid, None);
    assert_eq!(result.bank_name, "");
}

/// The API sends an explicit null for a field a country's format does not
/// have, rather than omitting the key. Serde treats those as different things,
/// and a suite whose fixtures only omit keys will pass while production fails.
#[tokio::test]
async fn explicit_nulls_are_read_as_absent() {
    let (_server, client) = server_replying(
        200,
        json!({
            "valid": true,
            "iban": "DE89370400440532013000",
            "branch_code": null,
            "bank_name": null,
            "national_check_valid": null,
            "sepa": null
        }),
    )
    .await;

    let result = client.validate("DE89370400440532013000").await.unwrap();

    assert_eq!(result.branch_code, "");
    assert_eq!(result.bank_name, "");
    assert_eq!(result.national_check_valid, None);
    assert_eq!(result.sepa, None);
    assert!(result.raw["branch_code"].is_null(), "raw keeps the null");
}

/// The same for a list and for the nested models inside one.
#[tokio::test]
async fn explicit_nulls_are_read_as_absent_in_nested_models() {
    let (_server, client) = server_replying(
        200,
        json!({
            "country_code": "DE",
            "length": 22,
            "bban_fields": [{ "label": "BLZ", "length": 8, "type": null, "description": null }],
            "sepa": null
        }),
    )
    .await;

    let spec = client.country_format("DE").await.unwrap();

    assert_eq!(spec.sepa, None);
    assert_eq!(spec.bban_fields.len(), 1);
    assert_eq!(spec.bban_fields[0].r#type, "");
    assert_eq!(spec.bban_fields[0].description, "");
}

#[tokio::test]
async fn a_null_list_is_read_as_empty() {
    let (_server, client) = server_replying(200, json!({ "count": 0, "results": null })).await;

    let batch = client.validate_bulk(["DE89"]).await.unwrap();
    assert!(batch.results.is_empty());
}

#[tokio::test]
async fn a_national_check_of_false_is_not_absent() {
    let (_server, client) = server_replying(
        200,
        json!({ "valid": true, "iban": "DE84100100100532013000", "national_check_valid": false }),
    )
    .await;

    let result = client.validate("DE84100100100532013000").await.unwrap();
    assert_eq!(result.national_check_valid, Some(false));
}

#[tokio::test]
async fn the_api_key_becomes_a_bearer_header_and_is_omitted_without_one() {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "valid": true, "iban": "DE89" })),
        )
        .mount(&server)
        .await;

    let with_key = Client::builder()
        .base_url(server.uri())
        .api_key("iban_test_key")
        .build()
        .unwrap();
    with_key.validate("DE89").await.unwrap();
    assert_eq!(
        last_request(&server)
            .await
            .headers
            .get("authorization")
            .unwrap(),
        "Bearer iban_test_key"
    );

    let without_key = Client::builder().base_url(server.uri()).build().unwrap();
    without_key.validate("DE89").await.unwrap();
    assert!(last_request(&server)
        .await
        .headers
        .get("authorization")
        .is_none());

    // An empty key is the same as no key.
    let empty_key = Client::builder()
        .base_url(server.uri())
        .api_key("")
        .build()
        .unwrap();
    empty_key.validate("DE89").await.unwrap();
    assert!(last_request(&server)
        .await
        .headers
        .get("authorization")
        .is_none());
}

#[tokio::test]
async fn the_user_agent_carries_the_client_version() {
    let (server, client) = server_replying(200, json!({ "valid": true, "iban": "DE89" })).await;
    client.validate("DE89").await.unwrap();

    assert_eq!(
        last_request(&server)
            .await
            .headers
            .get("user-agent")
            .unwrap(),
        &format!("ibanchecker-rust/{}", ibanchecker::VERSION)
    );
}

#[tokio::test]
async fn validate_bulk_keeps_input_order_and_counts() {
    let (server, client) = server_replying(
        200,
        json!({
            "count": 2, "valid_count": 1, "invalid_count": 1,
            "results": [
                { "valid": true, "iban": "DE89370400440532013000" },
                { "valid": false, "iban": "XX00" }
            ]
        }),
    )
    .await;

    let batch = client
        .validate_bulk(["DE89370400440532013000", "XX00"])
        .await
        .unwrap();

    assert_eq!(
        (batch.count, batch.valid_count, batch.invalid_count),
        (2, 1, 1)
    );
    assert_eq!(batch.results.len(), 2);
    assert_eq!(batch.results[0].iban, "DE89370400440532013000");
    assert_eq!(batch.results[1].iban, "XX00");
    assert_eq!(
        batch.results[0].raw["iban"], "DE89370400440532013000",
        "nested results should keep their own raw"
    );

    assert_eq!(
        last_request(&server)
            .await
            .body_json::<serde_json::Value>()
            .unwrap(),
        json!({ "ibans": ["DE89370400440532013000", "XX00"] })
    );
}

#[tokio::test]
async fn validate_bulk_sends_an_empty_array_not_null() {
    let (server, client) = server_replying(200, json!({ "count": 0, "results": [] })).await;
    client.validate_bulk(Vec::<String>::new()).await.unwrap();

    assert_eq!(
        last_request(&server)
            .await
            .body_json::<serde_json::Value>()
            .unwrap(),
        json!({ "ibans": [] })
    );
}

#[tokio::test]
async fn extract_reads_ibans_out_of_text() {
    let (server, client) = server_replying(
        200,
        json!({
            "count": 1, "valid_count": 1, "invalid_count": 0,
            "results": [{
                "valid": true,
                "iban": "DE89370400440532013000",
                "bank_name": "Commerzbank AG Cologne"
            }]
        }),
    )
    .await;

    let batch = client
        .extract("Please wire to DE89 3704 0044 0532 0130 00 by Friday.")
        .await
        .unwrap();

    assert_eq!(batch.results.len(), 1);
    assert_eq!(batch.results[0].bank_name, "Commerzbank AG Cologne");
    assert_eq!(last_request(&server).await.url.path(), "/extract");
}

#[tokio::test]
async fn country_format_parses_bban_fields() {
    let (server, client) = server_replying(
        200,
        json!({
            "country_code": "DE", "country_name": "Germany", "length": 22,
            "sepa": true, "swift": true, "example": "DE89370400440532013000",
            "bban_fields": [
                { "label": "BLZ", "length": 8, "type": "numeric", "description": "8-digit Bankleitzahl" },
                { "label": "Account No.", "length": 10, "type": "numeric" }
            ]
        }),
    )
    .await;

    let spec = client.country_format("DE").await.unwrap();

    assert_eq!(spec.length, 22);
    assert_eq!(spec.sepa, Some(true));
    assert_eq!(spec.bban_fields.len(), 2);
    assert_eq!(spec.bban_fields[0].label, "BLZ");
    assert_eq!(spec.bban_fields[0].length, 8);
    assert_eq!(spec.bban_fields[0].r#type, "numeric");
    assert_eq!(spec.bban_fields[1].description, "");

    let request = last_request(&server).await;
    assert_eq!(request.method.as_str(), "GET");
    assert_eq!(
        request.url.path(),
        "/formats/de",
        "the country code should be lowercased"
    );
    assert!(request.body.is_empty(), "a GET should send no body");
}

#[tokio::test]
async fn lookup_bic_uppercases_the_path() {
    let (server, client) = server_replying(
        200,
        json!({
            "bic": "DEUTDEFFXXX", "bic8": "DEUTDEFF",
            "bank_name": "Deutsche Bank AG Frankfurt", "city": "FRANKFURT AM MAIN",
            "sepa": true, "type": "private", "status": "active"
        }),
    )
    .await;

    let bank = client.lookup_bic("deutdeff").await.unwrap();

    assert_eq!(bank.bank_name, "Deutsche Bank AG Frankfurt");
    assert_eq!(bank.bic8, "DEUTDEFF");
    assert_eq!(bank.r#type, "private");
    assert_eq!(bank.status, "active");
    assert_eq!(last_request(&server).await.url.path(), "/swift/DEUTDEFF");
}

#[tokio::test]
async fn path_segments_are_escaped() {
    let (server, client) = server_replying(200, json!({ "bic": "X" })).await;
    client.lookup_bic("de/../admin").await.unwrap();

    assert_eq!(
        last_request(&server)
            .await
            .url
            .as_str()
            .rsplit('/')
            .next()
            .unwrap(),
        "DE%2F..%2FADMIN",
        "the separators should not climb out of the segment"
    );
}

#[tokio::test]
async fn error_statuses_map_to_variants() {
    for status in [400u16, 401, 404, 429, 500, 503] {
        let (_server, client) = server_replying(
            status,
            json!({ "error": "nope", "error_code": "SOME_CODE" }),
        )
        .await;

        let err = client.lookup_bic("ZZZZZZZZ").await.unwrap_err();

        let matched = matches!(
            (status, &err),
            (400, Error::BadRequest(_))
                | (401, Error::Authentication(_))
                | (404, Error::NotFound(_))
                | (429, Error::RateLimit(_))
                | (500, Error::Api(_))
                | (503, Error::Api(_))
        );
        assert!(matched, "HTTP {status} produced {err:?}");

        let api = err.api().expect("an API error");
        assert_eq!(api.status, status);
        assert_eq!(api.code, "SOME_CODE");
        assert_eq!(api.message, "nope");
        assert_eq!(api.body.as_ref().unwrap()["error_code"], "SOME_CODE");
        assert_eq!(err.status(), Some(status));
        assert_eq!(err.code(), Some("SOME_CODE"));
        assert!(err.to_string().contains("nope"));
    }
}

#[tokio::test]
async fn an_error_status_without_a_json_body_still_carries_the_status() {
    let (_server, client) = server_replying_raw(502, "<html>bad gateway</html>").await;

    let err = client.validate("DE89").await.unwrap_err();
    assert!(matches!(err, Error::Api(_)));

    let api = err.api().unwrap();
    assert_eq!(api.status, 502);
    assert_eq!(api.message, "HTTP 502");
    assert_eq!(api.code, "");
    assert!(api.body.is_none());
    assert_eq!(err.code(), None);
}

#[tokio::test]
async fn a_non_json_body_on_a_successful_status_is_an_error() {
    let (_server, client) = server_replying_raw(200, "<html>maintenance</html>").await;

    let err = client.validate("DE89").await.unwrap_err();
    assert!(err.to_string().contains("not JSON"), "{err}");
}

#[tokio::test]
async fn an_empty_body_on_a_successful_status_is_an_error() {
    let (_server, client) = server_replying_raw(200, "").await;

    let err = client.validate("DE89").await.unwrap_err();
    assert!(err.to_string().contains("empty body"), "{err}");
}

#[tokio::test]
async fn a_redirect_is_refused_with_the_target_named() {
    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(
            ResponseTemplate::new(301)
                .insert_header("location", "https://elsewhere.test/api/v1/validate"),
        )
        .mount(&server)
        .await;

    let client = Client::builder().base_url(server.uri()).build().unwrap();
    let err = client.validate("DE89370400440532013000").await.unwrap_err();

    let message = err.to_string();
    assert!(
        message.contains("refusing to follow the redirect"),
        "{message}"
    );
    assert!(
        message.contains("https://elsewhere.test/api/v1/validate"),
        "the target should be named: {message}"
    );
}

#[tokio::test]
async fn the_base_url_drops_a_trailing_slash() {
    let (server, client) = server_replying(200, json!({ "valid": true, "iban": "DE89" })).await;
    let with_slash = Client::builder()
        .base_url(format!("{}/", server.uri()))
        .build()
        .unwrap();

    with_slash.validate("DE89").await.unwrap();
    assert_eq!(
        last_request(&server).await.url.path(),
        "/validate",
        "a doubled slash would show up here"
    );
    drop(client);
}

#[tokio::test]
async fn a_failed_request_is_a_transport_error() {
    // Reserved for documentation by RFC 2606, so it resolves nowhere.
    let client = Client::builder()
        .base_url("https://ibanchecker.invalid/api/v1")
        .build()
        .unwrap();

    let err = client.country_format("de").await.unwrap_err();
    assert!(matches!(err, Error::Transport(_)), "{err:?}");
    assert_eq!(err.api(), None);
    assert_eq!(err.status(), None);
}
