# snip-snip

A screenshot and annotation tool: capture a region, annotate it, then copy or save the result, optionally through cloud storage in the user's own Cloudflare account.

## Language

**Snip**:
The annotated image the editor produces from a captured region.
_Avoid_: Screenshot, capture, image

**Cloud snip**:
A snip stored as a flattened image in the bucket, reachable from every machine signed in to the same Cloudflare account.
_Avoid_: Synced snip, upload

**Copy to cloud**:
The explicit editor action that copies a snip to the clipboard and also makes it a cloud snip; nothing becomes a cloud snip any other way.
_Avoid_: Upload, sync, share

**Cloudflare account**:
The user's own Cloudflare identity that snip-snip signs in with; snip-snip has no accounts of its own.
_Avoid_: snip-snip account, profile, user

**Bucket**:
The R2 bucket in the user's Cloudflare account that holds their cloud snips.
_Avoid_: Storage, cloud folder

**Library**:
The newest-first list of cloud snips in the bucket.
_Avoid_: Gallery, history
