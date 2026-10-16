# Bedrock HTTP API v0.4

All `/api/*` requests need `X-Bedrock-Client: 1`. The legacy `X-Lattice-Client: 1` remains accepted for existing clients. Send JSON commands with `Content-Type: application/json`. The Host header must equal the configured public authority. Untrusted forwarded-host headers are ignored. Credentials must never be placed in a URL.

| Method | Path | Authentication | Result |
|---|---|---|---|
| GET | `/healthz` | none | `{ "status": "ok" }`; no notebook content |
| GET | `/api/info` | none | `{ "version": "0.4.1", "authRequired": true }` |
| POST | `/api/session` | access key in JSON body | `{ "token": "..." }` plus HttpOnly cookie |
| GET | `/api/session` | session | `{ "authenticated": true }` |
| DELETE | `/api/session` | session | Revokes that token and clears cookie |
| GET | `/api/version` | session | `{ "notebook_id": "UUID", "revision": 0 }` |
| GET | `/api/notebook` | session | Authoritative notebook snapshot |
| GET | `/api/search?q=TEXT&tag=TAG` | session | `{ "revision": 0, "ids": [] }` |
| POST | `/api/commands` | session | `{ "notebook": SNAPSHOT, "selected_id": "ID or null" }` |

Login body: `{ "key": "your private server access key" }`. The returned token can be sent in `Authorization: Bearer TOKEN`. Otherwise the server reads the `lattice_session` cookie. An invalid explicit Authorization header never falls back to a valid cookie. Bearer tokens are random and stored hashed in process memory; sessions expire after 12 hours or server restart. The login request is limited to 4 KB, globally 60 attempts/minute and 128 simultaneous sessions. Rotate the key file and restart to revoke all access.

All origins require an exact configured match. CORS preflight permits GET/POST/DELETE and only Authorization, Content-Type, X-Bedrock-Client and legacy X-Lattice-Client headers. No wildcard origins. API responses have `Cache-Control: no-store` and `Vary: Origin`. HTTPS public origins produce Secure/HttpOnly/SameSite=Strict cookies; native clients should use the ephemeral bearer token because cross-site cookies may be restricted.

Snapshots contain `notebook_id`, `revision`, `notes`, `metadata`, `backlinks` and `edges`. A note has `id`, `title`, `body` and millisecond `updated` time. Metadata/rendered HTML, links, tags, heading IDs and UTF-16 heading offsets are computed by Rust.

Every command includes the current notebook UUID and revision. For example:

```json
{"notebook_id":"UUID returned by this server","revision":0,"kind":"create","title":"An idea","body":"# Heading\n\n[[Another idea]]"}
```

Other command kinds:

- `update`: `id`, `body`.
- `rename`: `id`, `title`; resolved incoming wiki-link targets are updated transactionally.
- `delete`: `id`.
- `import`: `notes: [{"title":"Name","body":"Markdown"}]`; merges and suffixes collisions, rewriting internal imported links.

An obsolete UUID or revision returns HTTP 409, code `revision_conflict`, without a write. Duplicate titles return 409, code `title_exists`. Invalid note input returns 400, missing notes 404, unauthenticated requests 401, blocked origins/hosts/client headers 403, login limits 429. Storage failures return 500 without exposing SQL details. Invalid JSON may use Axum's standard error text; clients must not assume every error body is JSON.

Limits: 1000 notes, 2 MB UTF-8 per note body, 8 MB total bodies, 180 Unicode characters per title. The JSON body cap is 50 MB to permit escaped content. Search query caps: 4096 bytes for q and 1024 for tag. SQLite uses WAL, FULL synchronous writes, parameterized SQL and an immediate transaction for mutations.
