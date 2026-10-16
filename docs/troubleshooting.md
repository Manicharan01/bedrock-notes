# Troubleshooting

Keep a tab containing unsaved work open. Download a JSON backup before clearing site data, reloading after a conflict, or uninstalling a client.

## The server cannot be reached

1. On the host, check `http://127.0.0.1:4173/healthz`. A healthy process returns `{"status":"ok"}`. Inspect startup output or `docker compose logs --tail=100 lattice` if it does not.
2. Check `BEDROCK_PUBLIC_URL`. It must be the exact public origin, such as `https://notes.example.org`, with no subdirectory.
3. Confirm the proxy preserves the public Host and reaches the actual server/container. A proxy container's `127.0.0.1` points to itself. On a shared network use `http://lattice:4173`.
4. Check DNS and HTTPS from the client. Clients reject remote plaintext HTTP. On Android, `localhost` is the phone; use the server's HTTPS hostname.

`/healthz` proves liveness, not that the public hostname, session, or notebook is working.

## HTTP 401 or session expired

Enter the current access key. Sessions expire after 12 hours and after a server restart. Native tokens stay in memory, so closing the app can require another sign-in. If you rotated the key, use the new one. Never put it in a URL or public issue.

## HTTP 403 / unrecognized host / native client blocked

The client's URL must match the server's public URL. Preserve its Host at the proxy. For native clients allow these origins explicitly:

```text
tauri://localhost
http://tauri.localhost
https://tauri.localhost
```

Compose and `Start-Bedrock.ps1` already set them. Keep exact origin checks and authentication enabled. Programmatic clients must send `X-Bedrock-Client: 1` or legacy `X-Lattice-Client: 1`.

## A conflict blocks saving

Another client changed the notebook or the database was replaced. Choose **Download drafts**, save the backup outside the app, then **Reload latest**. Compare the latest note with your export and reapply the intended edit. There is no automatic merge. After restoring a different database, reload to receive its notebook UUID.

## A save failed while offline

Wait for connectivity and use Ctrl/⌘ S. `Saved to SQLite` confirms acknowledgement. Drafts stay on the device when storage is available, but the shell cache cannot load a notebook offline. Clearing site storage removes recovery entries. If recovery storage is unavailable, export before closing.

## Windows client will not open

Extract the ZIP first and install Microsoft Edge WebView2 Runtime. The executable is unsigned; Windows may identify an unknown publisher. Compare its SHA-256 with release checksums before deciding whether to run it. The server and desktop client are separate processes.

## Android APK will not install

The APK is ARM64 only and needs Android 7/API 24+. It is debug-signed. Updates must share the installed APK's signing certificate; CI's fresh debug key may differ from a local build. Do not uninstall with unexported drafts just to bypass a mismatch. Production release signing is not configured.

## SQLite permission or recovery errors

The container runs as UID/GID 10001; its data and key mounts need appropriate permissions. On Linux, restrict the key to the container user; on Windows, use NTFS ACLs. Never delete WAL/SHM files to resolve an error. Stop the server and preserve the whole data directory before investigating or restoring.

## Report a reproducible bug

Use the [bug report template](https://github.com/Manicharan01/bedrock-notes/issues/new/choose). Include version, OS/browser, deployment shape, reproduction steps, expected/actual results, and redacted logs. Replace private notes, keys, tokens, cookies, and hostnames with synthetic examples.
