# Bedrock 0.4 — self-hosted notes

One Rust server owns the notebook. The browser/PWA and native Tauri clients for Windows, Linux and Android use the same authenticated HTTP API. No cloud account or AI API is required.

## Updating from Lattice

Native application IDs, notebook UUIDs, SQLite schema, local draft storage, session cookies and backup formats remain compatible. Existing notes are never renamed or rewritten by this design update. The database filename remains `lattice.sqlite3`; existing `LATTICE_` environment variables and `X-Lattice-Client` headers still work. New deployments use `BEDROCK_` variables and support `X-Bedrock-Client`. The bundled client retains the established wire header so it can also connect to version 0.3 servers. Keep your existing Compose project name (`docker compose -p YOUR_EXISTING_PROJECT`) when upgrading so its named data volume is reused.

## Windows: try it locally

Extract the Windows server package, then run `Start-Bedrock.ps1` from PowerShell. Or run:

```powershell
.\bedrock.exe --port 4173 --db .\data\lattice.sqlite3
```

Open `http://127.0.0.1:4173`. Loopback-only local mode permits an unauthenticated notebook. The web UI is embedded in the executable; Node is not needed on the server. The native desktop client is a separate executable and requires Microsoft Edge WebView2 Runtime.

Generate a private access key before enabling remote use:

```powershell
.\bedrock.exe --generate-key-file .\access-key.txt
.\Start-Bedrock.ps1 -Bind '0.0.0.0:4173' -PublicUrl 'https://notes.example.org' -KeyFile '.\access-key.txt'
```

Place a TLS reverse proxy in front of this process. Preserve the incoming public Host header and configure the public URL exactly, including a nonstandard port if used. This server does not terminate TLS. Restrict the HTTP upstream port to your proxy/private network; only the proxy's HTTPS port should be exposed to clients. A public URL must be a root origin, not a subdirectory.

## Docker / Compose

Prerequisites: Docker Engine/Compose, a real domain pointing to the host, and a TLS proxy. The default Compose port binds to host loopback for use with a proxy on that host. A proxy in another container must share a network with `lattice` and use `http://lattice:4173`; do not point a proxy container at its own loopback address.

Linux host quick start from this project directory:

```sh
cp .env.example .env
# Edit .env: use your actual HTTPS origin and domain.
mkdir -m 700 secrets
# Put secrets/ in a private directory; never commit it.
docker compose build
# Create a new key without printing it or overwriting an existing key.
docker run --rm --user 0 -v "$PWD/secrets:/secrets" --entrypoint sh bedrock-local:0.4.1 -c 'bedrock --generate-key-file /secrets/access-key.txt && chown 10001:10001 /secrets/access-key.txt && chmod 0400 /secrets/access-key.txt'
docker compose config --quiet
# Existing HTTPS proxy:
docker compose up -d
# OR the supplied Caddy proxy (requires ports 80 and 443 and public DNS):
docker compose -f compose.yaml -f deploy/compose.https.yaml config --quiet
docker compose -f compose.yaml -f deploy/compose.https.yaml up -d
```

Read the key locally with an appropriately privileged editor when connecting clients. The container runs as UID/GID 10001 and must be able to read the mounted secret. For Windows Docker bind mounts, use NTFS ACLs to restrict the key instead of Linux ownership commands.

Caddy stores certificate state in named volumes. The application stores SQLite in `lattice_data`, runs without root, drops capabilities, uses a read-only root filesystem and exposes `/healthz` for health checks. Existing Docker/Traefik networks or routes are not changed by these examples. Adapt the network attachment to your own installation.

## Connect clients

Allow native app origins on the server. Compose and `Start-Bedrock.ps1` already set these; manual launches should add:

```text
--allow-origin tauri://localhost
--allow-origin http://tauri.localhost
--allow-origin https://tauri.localhost
```

Open a native app, enter the server HTTPS origin, then enter its access key. In a browser, open the public server URL. Browser installation is available through the browser's install-app menu on supported HTTPS sites. The PWA caches only the app shell; it never caches API responses or notebook content.

The server URL is remembered on the device. Access keys and bearer tokens are not written to localStorage. The browser can resume a valid HttpOnly session cookie; native cross-origin clients keep a bearer token in memory and may need to sign in again after a restart. Sessions expire after 12 hours and are revoked by sign-out or server restart. To revoke all clients, rotate the key file and restart the server. Generate a new key into a different file, update the configuration, and retain the old file until the replacement is confirmed.

Settings → Switch server / sign out keeps local draft recovery entries and opens the connection chooser. Drafts and UI state are separated by normalized server URL and durable notebook UUID, so switching notebooks cannot apply another notebook's recovery data.

## Concurrent edits, offline use and backups

Clean clients check for changes every 15 seconds and on window focus. Each mutation must match both notebook UUID and revision. Two writers using the same revision produce one successful transaction and one conflict. A conflict preserves unsaved edits and blocks further writes until you explicitly export/discard them and reload. There is no automatic text merge.

The app needs the server to load a notebook and confirm saves. A loaded editor keeps unsaved drafts on that device during interruptions, but this is not a full offline notebook replica or an offline synchronization engine. Confirmed content remains on the server. Device draft recovery contains plaintext note content in localStorage; protect that device accordingly.

Use Settings to export a JSON backup (including unsaved drafts), export Markdown, import Markdown, or merge a JSON backup. Native clients use the operating system's file picker. Imports merge and rename collisions rather than replacing the notebook. Android content URIs may not contain the original filename; imports then use the first Markdown heading or an `Imported note N` fallback.

For a raw SQLite backup, stop the server and copy the **entire data directory**, including any WAL/SHM files, or use SQLite's online backup command. Do not copy only the live `.sqlite3` file while writes are active. Preserve the notebook UUID when restoring the same notebook. A database upgraded from version 0.2 migrates transactionally from schema 1 to 2; keep a backup first. The older 0.2 server will refuse schema 2.

## Configuration

| Environment | CLI | Purpose |
|---|---|---|
| `BEDROCK_BIND` | `--bind IP:port` | Listener, default `127.0.0.1:4173` |
| `BEDROCK_DB` | `--db PATH` | SQLite file, default `data/lattice.sqlite3` |
| `BEDROCK_PUBLIC_URL` | `--public-url ORIGIN` | HTTPS root origin advertised by your proxy |
| `BEDROCK_TOKEN_FILE` | `--token-file PATH` | Private key file, required outside loopback or with a remote public URL |
| `BEDROCK_ALLOWED_ORIGINS` | repeated `--allow-origin` | Exact client origins; env uses comma separation |
| `BEDROCK_ALLOW_INSECURE_HTTP=1` | `--allow-insecure-http` | Explicit isolated LAN/API testing override; normal clients still require HTTPS for remote servers |

`--port NUMBER` changes the port on the selected bind address. `--generate-key-file NEW_PATH` creates a random key file and exits, refusing to overwrite an existing path. Key files must contain 32–256 printable ASCII characters; generated keys contain 64 hex characters. Keep keys out of URLs, command-line arguments, source control and logs.

This is a single shared notebook/access key, intended for a personal server or trusted small group. It has no individual accounts, role permissions, end-to-end encryption, attachments, plugin execution or CRDT sync. SQLite and TLS protect different parts of the system; server administrators can read stored notes.

## Linux service example

`deploy/lattice.service` and `deploy/server.env.example` describe an optional systemd deployment. The legacy paths are intentional: binary `/opt/lattice/bedrock`, service user/group `lattice`, data `/var/lib/lattice`, and environment `/etc/lattice/server.env`. Install the binary and create that unprivileged user before enabling the unit. Store a generated key at `/etc/lattice/access-key.txt`, readable by that user only, and set the public URL in the environment file. Keep the listener on loopback behind your HTTPS proxy. Validate permissions and paths on your own host before starting the service.

## Upgrade safely

Export a notebook backup and preserve the server data directory before replacing a binary or image. Stop the old process, install the new version, then restart with the same database path, access-key file, public URL, and Compose project name. Restarting invalidates existing in-memory sessions; clients need to sign in again. Confirm the notebook UUID and a few notes before removing the old binary or backup. Schema changes may make a database incompatible with an older server, so retain the pre-upgrade backup for a rollback.

## Further reading

See [Development](development.md) for source/native builds, [API](../API.md) for integrations, [Architecture](architecture.md) for data/security boundaries, and [Troubleshooting](troubleshooting.md) for connection errors. Actual platform checks are recorded in [Verification](../VERIFICATION.md).
