use crate::{
    auth::{Auth, LoginError},
    config::ServerConfig,
    markdown,
    model::{CommandRequest, CommandResponse, Error, Snapshot},
    store::Store,
};
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Query, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct AppState {
    store: Arc<Mutex<Store>>,
    config: ServerConfig,
    auth: Arc<Auth>,
}

pub fn router(store: Store, port: u16) -> Router {
    router_configured(
        store,
        ServerConfig::local(port),
        Auth::new(None).expect("local auth"),
    )
}

pub fn router_configured(store: Store, config: ServerConfig, auth: Auth) -> Router {
    let state = AppState {
        store: Arc::new(Mutex::new(store)),
        config,
        auth: Arc::new(auth),
    };
    Router::new()
        .route(
            "/",
            get(|| async {
                asset(
                    "text/html; charset=utf-8",
                    include_str!("../web/index.html"),
                )
            }),
        )
        .route(
            "/style.css",
            get(|| async { asset("text/css; charset=utf-8", include_str!("../web/style.css")) }),
        )
        .route(
            "/app.mjs",
            get(|| async {
                asset(
                    "text/javascript; charset=utf-8",
                    include_str!("../web/app.mjs"),
                )
            }),
        )
        .route(
            "/client.mjs",
            get(|| async {
                asset(
                    "text/javascript; charset=utf-8",
                    include_str!("../web/client.mjs"),
                )
            }),
        )
        .route(
            "/connection.mjs",
            get(|| async {
                asset(
                    "text/javascript; charset=utf-8",
                    include_str!("../web/connection.mjs"),
                )
            }),
        )
        .route(
            "/manifest.webmanifest",
            get(|| async {
                asset(
                    "application/manifest+json",
                    include_str!("../web/manifest.webmanifest"),
                )
            }),
        )
        .route(
            "/sw.js",
            get(|| async {
                asset(
                    "text/javascript; charset=utf-8",
                    include_str!("../web/sw.js"),
                )
            }),
        )
        .route(
            "/icon.svg",
            get(|| async { asset("image/svg+xml", include_str!("../web/icon.svg")) }),
        )
        .route(
            "/healthz",
            get(|| async { Json(serde_json::json!({"status":"ok"})) }),
        )
        .route(
            "/icons/192x192.png",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "image/png")],
                    include_bytes!("../web/icons/192x192.png").as_slice(),
                )
            }),
        )
        .route(
            "/icons/512x512.png",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "image/png")],
                    include_bytes!("../web/icons/512x512.png").as_slice(),
                )
            }),
        )
        .route("/api/info", get(info))
        .route(
            "/api/session",
            get(session)
                .post(login)
                .delete(logout)
                .layer(DefaultBodyLimit::max(4096)),
        )
        .route("/api/version", get(version))
        .route("/api/notebook", get(notebook))
        .route("/api/commands", post(command))
        .route("/api/search", get(search))
        .fallback(|| async { (StatusCode::NOT_FOUND, "Not found") })
        // JSON escaping can expand a valid 8 MB notebook up to sixfold.
        .layer(DefaultBodyLimit::max(50_000_000))
        .layer(middleware::from_fn_with_state(state.clone(), boundary))
        .with_state(state)
}

fn asset(content_type: &'static str, text: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type)], text).into_response()
}

fn token(headers: &HeaderMap) -> Option<&str> {
    // An explicitly supplied bearer credential must never fall back to a cookie.
    if let Some(value) = headers.get(header::AUTHORIZATION) {
        return value.to_str().ok()?.strip_prefix("Bearer ");
    }
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| part.trim().strip_prefix("lattice_session="))
}
fn auth_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"code":"authentication_required", "message":message})),
    )
        .into_response()
}
async fn boundary(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let health = path == "/healthz" && request.method() == Method::GET;
    if !health
        && request
            .headers()
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            != Some(&state.config.authority)
    {
        return (StatusCode::FORBIDDEN, "Unrecognized host").into_response();
    }
    let api = path.starts_with("/api/");
    let origin = request.headers().get(header::ORIGIN).cloned();
    if api
        && origin.as_ref().is_some_and(|o| {
            !o.to_str()
                .is_ok_and(|o| state.config.allowed_origins.contains(o))
        })
    {
        return (StatusCode::FORBIDDEN, "Origin is not allowed").into_response();
    }
    let preflight = api && request.method() == Method::OPTIONS;
    if api
        && !preflight
        && request
            .headers()
            .get("x-bedrock-client")
            .or_else(|| request.headers().get("x-lattice-client"))
            .and_then(|v| v.to_str().ok())
            != Some("1")
    {
        return (StatusCode::FORBIDDEN, "Expected Bedrock client header").into_response();
    }
    let protected =
        api && path != "/api/info" && !(path == "/api/session" && request.method() == Method::POST);
    let mut response = if preflight {
        let method = request
            .headers()
            .get(header::ACCESS_CONTROL_REQUEST_METHOD)
            .and_then(|v| v.to_str().ok());
        let headers_ok = request
            .headers()
            .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
            .map(|v| {
                v.to_str().is_ok_and(|v| {
                    v.split(',').all(|h| {
                        matches!(
                            h.trim().to_ascii_lowercase().as_str(),
                            "authorization"
                                | "content-type"
                                | "x-bedrock-client"
                                | "x-lattice-client"
                        )
                    })
                })
            })
            .unwrap_or(true);
        if origin.is_none() || !matches!(method, Some("GET" | "POST" | "DELETE")) || !headers_ok {
            return (StatusCode::FORBIDDEN, "Invalid preflight").into_response();
        }
        StatusCode::NO_CONTENT.into_response()
    } else if protected && !state.auth.authorized(token(request.headers())) {
        auth_error(
            StatusCode::UNAUTHORIZED,
            "Connect with your server access key. Unsaved drafts are kept.",
        )
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    if api {
        headers.insert(header::VARY, HeaderValue::from_static("Origin"));
        if let Some(origin) = origin {
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                HeaderValue::from_static("true"),
            );
            if preflight {
                headers.insert(
                    header::ACCESS_CONTROL_ALLOW_METHODS,
                    HeaderValue::from_static("GET, POST, DELETE"),
                );
                headers.insert(
                    header::ACCESS_CONTROL_ALLOW_HEADERS,
                    HeaderValue::from_static(
                        "Authorization, Content-Type, X-Bedrock-Client, X-Lattice-Client",
                    ),
                );
            }
        }
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert("content-security-policy",HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' https: http://localhost:* http://127.0.0.1:* http://[::1]:*; frame-ancestors 'none'; base-uri 'none'; form-action 'self'"));
    response
}
async fn info(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(
        serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"authRequired":state.auth.required()}),
    )
}
async fn session() -> Json<serde_json::Value> {
    Json(serde_json::json!({"authenticated":true}))
}
#[derive(Deserialize)]
struct Login {
    key: String,
}
async fn login(State(state): State<AppState>, Json(login): Json<Login>) -> Response {
    match state.auth.login(&login.key) {
        Ok(token) => {
            let mut response = Json(serde_json::json!({"token":token})).into_response();
            if let Some(token) = token {
                let cookie = format!(
                    "lattice_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=43200{}",
                    if state.config.secure_cookie {
                        "; Secure"
                    } else {
                        ""
                    }
                );
                if let Ok(value) = HeaderValue::from_str(&cookie) {
                    response.headers_mut().insert(header::SET_COOKIE, value);
                }
            }
            response
        }
        Err(LoginError::Invalid) => auth_error(StatusCode::UNAUTHORIZED, "Invalid access key"),
        Err(LoginError::RateLimited | LoginError::Capacity) => auth_error(
            StatusCode::TOO_MANY_REQUESTS,
            "Session limit reached; try again later",
        ),
        Err(LoginError::Internal) => auth_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Authentication unavailable",
        ),
    }
}
async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    state.auth.logout(token(&headers));
    let mut response = Json(serde_json::json!({"authenticated":false})).into_response();
    let cookie = format!(
        "lattice_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{}",
        if state.config.secure_cookie {
            "; Secure"
        } else {
            ""
        }
    );
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}
async fn version(State(state): State<AppState>) -> Result<Json<crate::model::Version>, ApiError> {
    Ok(Json(with_store(state, Store::version).await?))
}

async fn with_store<T, F>(state: AppState, action: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&mut Store) -> Result<T, Error> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let mut store = state
            .store
            .lock()
            .map_err(|_| Error::Internal("Store lock poisoned".into()))?;
        action(&mut store)
    })
    .await
    .map_err(|e| ApiError(Error::Internal(e.to_string())))?
    .map_err(ApiError)
}

async fn notebook(State(state): State<AppState>) -> Result<Json<Snapshot>, ApiError> {
    Ok(Json(with_store(state, Store::read).await?))
}

async fn command(
    State(state): State<AppState>,
    Json(request): Json<CommandRequest>,
) -> Result<Json<CommandResponse>, ApiError> {
    Ok(Json(
        with_store(state, move |store| store.apply(request)).await?,
    ))
}

#[derive(Deserialize, Default)]
struct SearchQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    tag: String,
}
#[derive(Serialize)]
struct SearchResult {
    revision: i64,
    ids: Vec<String>,
}

async fn search(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResult>, ApiError> {
    if query.q.len() > 4096 || query.tag.len() > 1024 {
        return Err(ApiError(Error::Invalid("Search query too long".into())));
    }
    let notebook = with_store(state, Store::read).await?;
    Ok(Json(SearchResult {
        revision: notebook.revision,
        ids: markdown::search(&notebook, &query.q, &query.tag),
    }))
}

pub struct ApiError(pub Error);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response<Body> {
        let (status, code, message) = match &self.0 {
            Error::Invalid(message) => (StatusCode::BAD_REQUEST, "invalid_input", message.clone()),
            Error::NotFound => (StatusCode::NOT_FOUND, "not_found", "Note not found".into()),
            Error::Conflict => (
                StatusCode::CONFLICT,
                "revision_conflict",
                "Notebook changed; export drafts and reload before writing".into(),
            ),
            Error::TitleExists => (
                StatusCode::CONFLICT,
                "title_exists",
                "A note with that title already exists".into(),
            ),
            error => {
                eprintln!("{error}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "storage_error",
                    "Storage operation failed; your request was not confirmed".into(),
                )
            }
        };
        (
            status,
            Json(serde_json::json!({"code": code, "message": message})),
        )
            .into_response()
    }
}
