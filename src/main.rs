use lattice::{auth::Auth, config::ServerConfig, http, store::Store};
use std::{io::Write, net::SocketAddr, path::PathBuf};

fn env(name: &str) -> Option<String> {
    std::env::var(name.replace("LATTICE_", "BEDROCK_"))
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var(name).ok().filter(|v| !v.is_empty()))
}
fn generate_key(path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    writeln!(
        file,
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )?;
    file.sync_all()?;
    println!(
        "Created access key file: {}. Keep it private.",
        path.display()
    );
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    if let Ok(mut terminate) =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
        return;
    }
    let _ = tokio::signal::ctrl_c().await;
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bind: SocketAddr = env("LATTICE_BIND")
        .unwrap_or_else(|| "127.0.0.1:4173".into())
        .parse()?;
    let mut database =
        PathBuf::from(env("LATTICE_DB").unwrap_or_else(|| "data/lattice.sqlite3".into()));
    let mut public_url = env("LATTICE_PUBLIC_URL");
    let mut token_file = env("LATTICE_TOKEN_FILE").map(PathBuf::from);
    let mut insecure = env("LATTICE_ALLOW_INSECURE_HTTP").as_deref() == Some("1");
    let mut origins: Vec<String> = env("LATTICE_ALLOWED_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => bind = args.next().ok_or("--bind requires an IP:port")?.parse()?,
            "--port" => bind.set_port(args.next().ok_or("--port requires a number")?.parse()?),
            "--db" => database = PathBuf::from(args.next().ok_or("--db requires a path")?),
            "--public-url" => {
                public_url = Some(args.next().ok_or("--public-url requires an origin")?)
            }
            "--token-file" => {
                token_file = Some(PathBuf::from(
                    args.next().ok_or("--token-file requires a path")?,
                ))
            }
            "--allow-origin" => {
                origins.push(args.next().ok_or("--allow-origin requires an origin")?)
            }
            "--allow-insecure-http" => insecure = true,
            "--generate-key-file" => {
                return generate_key(PathBuf::from(
                    args.next()
                        .ok_or("--generate-key-file requires a new file path")?,
                ));
            }
            "--help" | "-h" => {
                println!(
                    "Bedrock {}\n--bind IP:port (default 127.0.0.1:4173)\n--db PATH\n--public-url https://notes.example.org\n--token-file PATH (required for remote access)\n--allow-origin ORIGIN (repeatable)\n--allow-insecure-http (isolated LAN testing only)\n--generate-key-file NEW_PATH\nEnvironment: BEDROCK_BIND, BEDROCK_DB, BEDROCK_PUBLIC_URL, BEDROCK_TOKEN_FILE, BEDROCK_ALLOWED_ORIGINS (comma-separated), BEDROCK_ALLOW_INSECURE_HTTP=1. Legacy LATTICE_ variables remain accepted.\nTLS terminates at your reverse proxy. Preserve the public Host header. Native origins must be allowed explicitly; see README.",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    let key = token_file.map(std::fs::read_to_string).transpose()?;
    let auth = Auth::new(key.as_deref().map(str::trim))?;
    // Validate before opening the database or binding to the network.
    let requested_url = public_url
        .clone()
        .unwrap_or_else(|| format!("http://{bind}"));
    let config = ServerConfig::new(&requested_url, &origins, insecure)?;
    config.validate_bind(bind, auth.required())?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let config = if public_url.is_none() && bind.port() == 0 {
        ServerConfig::new(
            &format!("http://{}", listener.local_addr()?),
            &origins,
            insecure,
        )?
    } else {
        config
    };
    if let Some(parent) = database.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let store = Store::open(&database)?;
    println!(
        "Bedrock: {}\nListening: {}\nDatabase: {}\nAuthentication: {}",
        config.public_origin,
        listener.local_addr()?,
        database.display(),
        if auth.required() {
            "required"
        } else {
            "local mode"
        }
    );
    axum::serve(listener, http::router_configured(store, config, auth))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}
