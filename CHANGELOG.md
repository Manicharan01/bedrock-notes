# Changelog

## 0.4.1 — public project and finishing touches

- Added a screenshot-led README and dedicated user, hosting, architecture, development, and troubleshooting guides.
- Added CI for Windows/Linux, local documentation-link checks, issue forms, contribution and security guidance.
- Added keyboard shortcuts help, Escape/outside-click panel dismissal, skip navigation, visible input focus, and accessible mode state.
- Fixed native Markdown export filenames containing path separators, reserved Windows names, or unsupported characters.
- Fixed stale connection status after the server becomes reachable again and added an explicit retry when initial loading fails.
- Preserved notebook data, identities, legacy configuration, and compatibility with 0.3 servers.

## 0.4.0 — name and design update

- Renamed the app, native windows, Android label, manifests, export filenames, server binary and public documentation to Bedrock.
- Added an original layered block icon and forest/stone/lime palette across the web, PWA, Windows and Android clients.
- Replaced the vertical tool ribbon with horizontal navigation, a numbered note index and a paper writing surface. Collections fold away; Links opens on demand.
- Introduced the Connection Atlas with square blocks on a grid, plus phone layouts checked at 320 and 390 pixels.
- Preserved installed app identities, notebook UUIDs, note content, database schema and filenames, local recovery keys and backup formats.
- Added BEDROCK environment variables and X-Bedrock-Client while accepting the legacy names, including Compose configuration compatibility.

## Existing backend guarantees

Transactional identity/revision conflict checks, draft recovery, HTTPS proxy support, bounded authentication, safe Markdown rendering and cross-device refresh behavior remain covered by the regression suite. See VERIFICATION.md for this version's checks.
