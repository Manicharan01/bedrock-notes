use std::{collections::BTreeSet, net::SocketAddr};
use url::Url;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub public_origin: String,
    pub authority: String,
    pub secure_cookie: bool,
    pub allowed_origins: BTreeSet<String>,
}

pub fn normalize_origin(value: &str) -> Result<String, String> {
    let url = Url::parse(value).map_err(|_| "Invalid public URL/origin")?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "URLs must be HTTP(S) origins without credentials, a path, query or fragment".into(),
        );
    }
    Ok(url.origin().ascii_serialization())
}

impl ServerConfig {
    pub fn local(port: u16) -> Self {
        Self::new(&format!("http://127.0.0.1:{port}"), &[], true).expect("valid loopback origin")
    }
    pub fn new(
        public_url: &str,
        extra_origins: &[String],
        allow_insecure: bool,
    ) -> Result<Self, String> {
        let public_origin = normalize_origin(public_url)?;
        let url = Url::parse(&public_origin).map_err(|e| e.to_string())?;
        let loopback = matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        );
        if url.scheme() != "https" && !loopback && !allow_insecure {
            return Err("Remote self-hosting requires an HTTPS public URL (terminate TLS at the reverse proxy)".into());
        }
        let mut allowed_origins = BTreeSet::from([public_origin.clone()]);
        for origin in extra_origins {
            let normalized = if origin.trim_end_matches('/') == "tauri://localhost" {
                "tauri://localhost".into()
            } else {
                normalize_origin(origin)?
            };
            if !matches!(
                normalized.as_str(),
                "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost"
            ) {
                let parsed = Url::parse(&normalized).map_err(|e| e.to_string())?;
                let is_loopback = matches!(
                    parsed.host_str(),
                    Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
                );
                if parsed.scheme() != "https" && !is_loopback {
                    return Err("Extra browser origins must use HTTPS".into());
                }
            }
            allowed_origins.insert(normalized);
        }
        let authority = match url.port() {
            Some(port) => format!("{}:{port}", url.host_str().ok_or("Missing host")?),
            None => url.host_str().ok_or("Missing host")?.to_string(),
        };
        Ok(Self {
            secure_cookie: url.scheme() == "https",
            public_origin,
            authority,
            allowed_origins,
        })
    }
    pub fn validate_bind(&self, bind: SocketAddr, has_key: bool) -> Result<(), String> {
        let url = Url::parse(&self.public_origin).map_err(|e| e.to_string())?;
        let local_public = matches!(
            url.host_str(),
            Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
        );
        if (!bind.ip().is_loopback() || !local_public) && !has_key {
            return Err("Remote self-hosting requires BEDROCK_TOKEN_FILE or --token-file".into());
        }
        Ok(())
    }
}
