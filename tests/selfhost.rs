use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use lattice::{
    auth::{Auth, LoginError},
    config::ServerConfig,
    http::router_configured,
    model::{Command, CommandRequest, Error},
    store::Store,
};
use serde_json::{Value, json};
use tower::ServiceExt;
const KEY: &str = "synthetic-test-key-never-use-in-deployment-123456";
fn app() -> Router {
    router_configured(
        Store::memory().unwrap(),
        ServerConfig::new(
            "https://notes.example.org",
            &[
                "tauri://localhost".into(),
                "http://tauri.localhost".into(),
                "https://tauri.localhost".into(),
            ],
            false,
        )
        .unwrap(),
        Auth::new(Some(KEY)).unwrap(),
    )
}
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    payload: Value,
    credential: Option<(&str, &str)>,
    origin: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("Host", "notes.example.org")
        .header("X-Lattice-Client", "1")
        .header("Content-Type", "application/json");
    if let Some((name, value)) = credential {
        request = request.header(name, value);
    }
    if let Some(origin) = origin {
        request = request.header("Origin", origin);
    }
    let response = app
        .clone()
        .oneshot(
            request
                .body(if payload.is_null() {
                    Body::empty()
                } else {
                    Body::from(payload.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 10_000_000).await.unwrap();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
#[tokio::test]
async fn authentication_cookie_bearer_logout_and_restart_semantics() {
    let app = app();
    for path in [
        "/api/notebook",
        "/api/search",
        "/api/version",
        "/api/session",
    ] {
        assert_eq!(
            call(&app, "GET", path, Value::Null, None, None).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        call(&app, "GET", "/api/info", Value::Null, None, None)
            .await
            .2["authRequired"],
        true
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/session",
            json!({"key":"wrong"}),
            None,
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, headers, value) = call(
        &app,
        "POST",
        "/api/session",
        json!({"key":KEY}),
        None,
        Some("tauri://localhost"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let cookie = headers["set-cookie"].to_str().unwrap();
    for flag in ["HttpOnly", "Secure", "SameSite=Strict", "Max-Age=43200"] {
        assert!(cookie.contains(flag));
    }
    let bearer = format!("Bearer {}", value["token"].as_str().unwrap());
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/notebook",
            Value::Null,
            Some(("Authorization", &bearer)),
            Some("http://tauri.localhost")
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/version",
            Value::Null,
            Some(("Cookie", cookie)),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "DELETE",
            "/api/session",
            Value::Null,
            Some(("Authorization", &bearer)),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/notebook",
            Value::Null,
            Some(("Cookie", cookie)),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
async fn invalid_bearer_never_falls_back_to_valid_cookie() {
    let app = app();
    let (_, headers, _) = call(&app, "POST", "/api/session", json!({"key":KEY}), None, None).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/notebook")
                .header("Host", "notes.example.org")
                .header("X-Lattice-Client", "1")
                .header("Cookie", headers["set-cookie"].clone())
                .header("Authorization", "Bearer wrong")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
#[tokio::test]
async fn cors_preflights_require_exact_origin_methods_and_headers() {
    let app = app();
    for (origin, method, headers, expected) in [
        (
            "tauri://localhost",
            "POST",
            "content-type,authorization,x-lattice-client",
            StatusCode::NO_CONTENT,
        ),
        (
            "https://tauri.localhost",
            "GET",
            "x-lattice-client",
            StatusCode::NO_CONTENT,
        ),
        (
            "https://evil.example",
            "POST",
            "content-type",
            StatusCode::FORBIDDEN,
        ),
        ("null", "GET", "", StatusCode::FORBIDDEN),
        (
            "tauri://localhost",
            "PUT",
            "x-lattice-client",
            StatusCode::FORBIDDEN,
        ),
        (
            "tauri://localhost",
            "GET",
            "x-unapproved",
            StatusCode::FORBIDDEN,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/commands")
                    .header("Host", "notes.example.org")
                    .header("Origin", origin)
                    .header("Access-Control-Request-Method", method)
                    .header("Access-Control-Request-Headers", headers)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        if expected == StatusCode::NO_CONTENT {
            assert_eq!(response.headers()["access-control-allow-origin"], origin);
            assert_eq!(response.headers()["vary"], "Origin");
            assert_eq!(
                response.headers()["access-control-allow-credentials"],
                "true"
            );
        }
    }
}
#[tokio::test]
async fn host_spoofing_login_body_limit_and_health_are_bounded() {
    let app = app();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/info")
                .header("Host", "attacker.test")
                .header("X-Forwarded-Host", "notes.example.org")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/session",
            json!({"key":"a".repeat(5000)}),
            None,
            None
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("Host", "127.0.0.1:4173")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
#[test]
fn public_configuration_fails_closed() {
    assert!(ServerConfig::new("http://notes.example.org", &[], false).is_err());
    for bad in [
        "https://user:pass@notes.test",
        "https://notes.test/path",
        "https://notes.test/?secret=a",
        "https://notes.test/#x",
        "ftp://notes.test",
    ] {
        assert!(ServerConfig::new(bad, &[], false).is_err());
    }
    let config = ServerConfig::new("https://notes.test", &[], false).unwrap();
    assert!(
        config
            .validate_bind("127.0.0.1:4173".parse().unwrap(), false)
            .is_err()
    );
    assert!(
        config
            .validate_bind("0.0.0.0:4173".parse().unwrap(), true)
            .is_ok()
    );
    assert!(
        ServerConfig::local(4173)
            .validate_bind("0.0.0.0:4173".parse().unwrap(), false)
            .is_err()
    );
    assert_eq!(
        ServerConfig::new("http://[::1]:4173", &[], false)
            .unwrap()
            .authority,
        "[::1]:4173"
    );
}
#[test]
fn access_key_validation_revocation_and_rate_limit() {
    assert!(Auth::new(Some("short")).is_err());
    assert!(Auth::new(Some(&"x".repeat(257))).is_err());
    let auth = Auth::new(Some(KEY)).unwrap();
    let token = auth.login(KEY).unwrap().unwrap();
    assert!(auth.authorized(Some(&token)));
    assert!(!auth.authorized(Some(&"a".repeat(64))));
    auth.logout(Some(&token));
    assert!(!auth.authorized(Some(&token)));
    assert!(!Auth::new(Some(KEY)).unwrap().authorized(Some(&token)));
    for _ in 1..60 {
        assert_eq!(auth.login("wrong"), Err(LoginError::Invalid));
    }
    assert_eq!(auth.login(KEY), Err(LoginError::RateLimited));
}
#[test]
fn database_identity_persists_and_rejects_wrong_notebook_with_same_revision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.sqlite3");
    let mut store = Store::open_with_seed(&path, &[]).unwrap();
    let version = store.version().unwrap();
    drop(store);
    let mut store = Store::open_with_seed(&path, &[]).unwrap();
    assert_eq!(version.notebook_id, store.read().unwrap().notebook_id);
    let other = Store::memory().unwrap().version().unwrap().notebook_id;
    assert!(matches!(
        store.apply(CommandRequest {
            notebook_id: other,
            revision: 0,
            command: Command::Create {
                title: "Wrong".into(),
                body: "".into()
            }
        }),
        Err(Error::Conflict)
    ));
    assert_eq!(store.version().unwrap().revision, 0);
}
#[test]
fn schema_one_upgrade_preserves_notes_and_revision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v1.sqlite3");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE notebook(id INTEGER PRIMARY KEY,revision INTEGER NOT NULL);INSERT INTO notebook VALUES(1,7);CREATE TABLE notes(id TEXT PRIMARY KEY,title TEXT NOT NULL,title_key TEXT UNIQUE NOT NULL,body TEXT NOT NULL,updated INTEGER NOT NULL,position INTEGER UNIQUE NOT NULL);INSERT INTO notes VALUES('a','Existing','existing','keep this',1,0);PRAGMA user_version=1;").unwrap();
    drop(db);
    let mut store = Store::open_with_seed(&path, &[]).unwrap();
    let snapshot = store.read().unwrap();
    assert_eq!(snapshot.revision, 7);
    assert_eq!(snapshot.notes[0].body, "keep this");
    assert!(!snapshot.notebook_id.is_empty());
    drop(store);
    assert_eq!(
        Store::open_with_seed(&path, &[])
            .unwrap()
            .version()
            .unwrap()
            .notebook_id,
        snapshot.notebook_id
    );
}

#[tokio::test]
async fn bedrock_client_header_and_native_preflight_work() {
    let app = app();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/info")
                .header("Host", "notes.example.org")
                .header("X-Bedrock-Client", "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/commands")
                .header("Host", "notes.example.org")
                .header("Origin", "http://tauri.localhost")
                .header("Access-Control-Request-Method", "POST")
                .header(
                    "Access-Control-Request-Headers",
                    "content-type,x-bedrock-client,authorization",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let allowed = response.headers()["access-control-allow-headers"]
        .to_str()
        .unwrap();
    assert!(allowed.contains("X-Bedrock-Client"));
    assert!(allowed.contains("X-Lattice-Client"));
}
