# Cloud

Signed in with Cloudflare, snips can be copied to an R2 bucket in the user's
own account and pasted on any machine signed in to the same account. See
`docs/adr/0002-cloud-snips-in-the-users-own-r2-bucket.md`.

## Sub-features

- Settings: Sign in with Cloudflare (browser consent, loopback callback on
  `127.0.0.1:8976`), account picker, R2-not-enabled prompt with Retry, Sign out.
- The session restores on launch without a network call.
- Editor Export menu: Copy to cloud (`mod+u`) copies, uploads, then
  closes; a failed upload keeps the editor open with the error.
- Tray / menu bar, only while signed in: Copy latest cloud snip and Cloud library.
- Library: newest 100 snips as thumbnails; click copies; Delete takes a second click.

## How to get to it (user POV)

Settings → Sign in with Cloudflare, approve in the browser. Then snip,
Export → Copy to cloud. On the other machine, tray → Copy latest cloud snip,
or tray → Cloud library and click a snip.

## Driving it with snipctl

Preconditions: sign-in needs a person to approve Cloudflare's consent page in
the browser, and only members of the account that owns the OAuth client can
sign in. Everything after sign-in touches that person's real bucket, so name
the snips you upload and delete only those.

- **Copy to cloud.** Snip the fixture, use the Export menu's Copy to cloud with
  `click-screen`, and confirm the editor closed. Read the clipboard.
- **Round trip.** Tray → Copy latest cloud snip, read the clipboard, and
  `compare` it with the Copy to cloud clipboard capture.
- **Library.** Open Cloud library, `shot` it, click the newest tile, and
  compare the clipboard. Delete the uploaded snip with two clicks.
- **Errors, when affected.** Controlled tests cover upload, listing and
  session failures with an in-memory bucket and a local Cloudflare stand-in.

## Gotchas

- Copy to cloud avoids `mod+shift+c`: another app reserves it globally on the
  Windows verification machine, along with `ctrl+shift+u` and `ctrl+shift+x`.
  If `mod+u` doesn't reach the editor, diagnose with `probe-shortcut`.
- Signing out clears this machine's saved session and thumbnails only.
