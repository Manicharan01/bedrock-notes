use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use lattice::{http::router, store::Store};
use serde_json::{Value, json};
use tower::ServiceExt;

fn app() -> Router {
    router(Store::memory().unwrap(), 4173)
}
async fn call(app: &Router, method: &str, path: &str, mut body: Value) -> (StatusCode, Value) {
    if method == "POST" && path == "/api/commands" {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/version")
                    .header("Host", "127.0.0.1:4173")
                    .header("X-Lattice-Client", "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let version: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        body["notebook_id"] = version["notebook_id"].clone();
    }
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("Host", "127.0.0.1:4173")
        .header("X-Lattice-Client", "1")
        .header("Content-Type", "application/json")
        .body(if method == "GET" {
            Body::empty()
        } else {
            Body::from(body.to_string())
        })
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64_000_000).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn typed_create_read_search_and_rename_roundtrip() {
    let app = app();
    let (status, created) = call(
        &app,
        "POST",
        "/api/commands",
        json!({"revision":0,"kind":"create","title":"Alpha","body":"# Heading\n\n#rust"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = created["selected_id"].as_str().unwrap();
    let (status, notebook) = call(&app, "GET", "/api/notebook", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(notebook["revision"], 1);
    assert!(
        notebook["metadata"][id]["html"]
            .as_str()
            .unwrap()
            .contains("id=\"heading\"")
    );
    let (_, found) = call(&app, "GET", "/api/search?q=alpha&tag=rust", Value::Null).await;
    assert_eq!(found["ids"][0], id);
    let (status, renamed) = call(
        &app,
        "POST",
        "/api/commands",
        json!({"revision":1,"kind":"rename","id":id,"title":"Beta"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(renamed["notebook"]["notes"][0]["title"], "Beta");
}

#[tokio::test]
async fn stale_revision_and_title_collision_are_distinct_conflicts() {
    let app = app();
    call(
        &app,
        "POST",
        "/api/commands",
        json!({"revision":0,"kind":"create","title":"Alpha"}),
    )
    .await;
    let (status, error) = call(
        &app,
        "POST",
        "/api/commands",
        json!({"revision":0,"kind":"create","title":"Beta"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "revision_conflict");
    let (status, error) = call(
        &app,
        "POST",
        "/api/commands",
        json!({"revision":1,"kind":"create","title":"ALPHA.md"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "title_exists");
}

#[tokio::test]
async fn missing_header_host_and_foreign_origins_are_rejected() {
    let app = app();
    for (host, client, origin) in [
        ("127.0.0.1:4173", false, None),
        ("evil.example:4173", true, None),
        ("127.0.0.1:4173", true, Some("https://evil.example")),
        ("127.0.0.1:4173", true, Some("null")),
    ] {
        let mut request = Request::builder().uri("/api/notebook").header("Host", host);
        if client {
            request = request.header("X-Lattice-Client", "1");
        }
        if let Some(origin) = origin {
            request = request.header("Origin", origin);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
async fn same_origin_client_is_accepted() {
    let request = Request::builder()
        .uri("/api/notebook")
        .header("Host", "127.0.0.1:4173")
        .header("Origin", "http://127.0.0.1:4173")
        .header("X-Lattice-Client", "1")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app().oneshot(request).await.unwrap().status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn malformed_json_unknown_commands_and_wrong_types_do_not_mutate() {
    let app = app();
    for value in [
        json!({"revision":0,"kind":"erase_everything"}),
        json!({"revision":"0","kind":"create","title":"A"}),
        json!({"kind":"create","title":"A"}),
        json!({"revision":0,"kind":"create","title":42}),
    ] {
        let (status, _) = call(&app, "POST", "/api/commands", value).await;
        assert!(status.is_client_error());
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/commands")
                .header("Host", "127.0.0.1:4173")
                .header("X-Lattice-Client", "1")
                .header("Content-Type", "application/json")
                .body(Body::from("{"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.status().is_client_error());
    let (_, s) = call(&app, "GET", "/api/notebook", Value::Null).await;
    assert_eq!(s["revision"], 0);
    assert_eq!(s["notes"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn invalid_import_is_atomic_over_http() {
    let app = app();
    let (status,_)=call(&app,"POST","/api/commands",json!({"revision":0,"kind":"import","notes":[{"title":"Good","body":"hello"},{"title":"../bad","body":"oops"}]})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, s) = call(&app, "GET", "/api/notebook", Value::Null).await;
    assert_eq!(s["revision"], 0);
    assert_eq!(s["notes"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn only_embedded_assets_are_served_and_api_is_uncached() {
    let app = app();
    for path in [
        "/../Cargo.toml",
        "/data/lattice.sqlite3",
        "/seed.json",
        "/core.mjs",
    ] {
        let (status, _) = call(&app, "GET", path, Value::Null).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("Host", "127.0.0.1:4173")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert!(
        response.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'none'")
    );
}

#[tokio::test]
async fn json_escape_expansion_of_a_valid_note_is_supported() {
    let app = app();
    let (status, response) = call(
        &app,
        "POST",
        "/api/commands",
        json!({"revision":0,"kind":"create","title":"Escaped","body":"\0".repeat(lattice::model::MAX_BODY)}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        response["notebook"]["notes"][0]["body"]
            .as_str()
            .unwrap()
            .len(),
        lattice::model::MAX_BODY
    );
}

#[tokio::test]
async fn search_rejects_overlong_queries() {
    let app = app();
    let (status, _) = call(
        &app,
        "GET",
        &format!("/api/search?q={}", "x".repeat(4097)),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
