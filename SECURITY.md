# Security policy

Bedrock is early-stage personal-server software. The latest release receives fixes; older releases have no separate maintenance branch. Passing CI does not imply a security audit.

## Report privately

Use [GitHub private vulnerability reporting](https://github.com/Manicharan01/bedrock-notes/security/advisories/new). Include the affected version, synthetic reproduction steps, impact, and a proposed fix if available. Exclude real notes and credentials. If private reporting is unavailable, ask the maintainer through their GitHub profile for a private channel before sending details.

Avoid public vulnerability reports until the maintainer can investigate. No response-time or bounty commitment is offered.

## Deployment boundaries

One notebook and shared key; no accounts, roles, or end-to-end encryption. Server administrators can read SQLite. Device drafts are plaintext localStorage. Remote access needs a key and TLS reverse proxy; keep the upstream port private.

Keep keys and sessions out of URLs, source control, screenshots, logs, and bug reports. Rotate the key and restart if exposed. Back up independently before upgrades. See [architecture](docs/architecture.md#data-and-security-model) and [hosting](docs/self-hosting.md).
