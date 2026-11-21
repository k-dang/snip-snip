# 2. Cloud snips in the user's own R2 bucket

Date: November 21, 2026. Status: accepted.

Cloud snips let a snip copied on one machine be pasted on another. snip-snip
has no backend and no accounts of its own: the user signs in with their
Cloudflare account (Cloudflare's OAuth, Authorization Code with PKCE, as a
public client), and the app reads and writes a `snip-snip` R2 bucket in that
account through Cloudflare's REST API using the OAuth token. We chose this so
the project hosts nothing, pays nothing, holds no one else's screenshots, and
has no user limit; the cost is that users need a Cloudflare account with R2
enabled.

## Considered options

- **Pasted R2 credentials (S3 API).** No browser sign-in, but asks
  non-experts to create scoped API tokens and paste secrets.
- **A hosted Worker and bucket behind Cloudflare Access (Managed OAuth).**
  Easiest sign-in (Google, GitHub, email PIN) and no Cloudflare account needed,
  but Access is built for an organization's own users: the free plan stops at
  50 seats, each sign-in holds a seat, and an open "everyone" policy is what
  Cloudflare warns against. We would also run a backend, pay for everyone's
  storage, and be custodian of strangers' private screenshots.
- **A public hosted service with its own auth.** Scales past Access's seat
  model but is a much larger project carrying the same cost and custody
  burden.

## Consequences

- The only stored credential is the OAuth refresh token, kept in the OS
  credential store. Refreshes may rotate it, so the app saves each one it
  receives.
- The OAuth client is registered in the maintainer's Cloudflare account and
  stays private for now, so only members of that account can sign in. Opening
  it to every Cloudflare user is a one-way step that needs a verified domain.
- The REST API allows 1,200 R2 requests per five minutes per account and
  300 MB per upload, which is far beyond snip-sized traffic.
- Retention is the bucket's lifecycle rules, set by the user; the app never
  expires cloud snips.
