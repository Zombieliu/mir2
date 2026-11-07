# Full Windows installer reader

The reader exposes five fixed HTTPS paths for one immutable release: the complete
`Mir2Setup.exe` and its four public companions. It reads independently uploaded
R2 objects of at most 64 MiB and streams the requested bytes in order. Host, path,
release layout, size, identity encoding and public ETag contracts remain fixed.
Internal cache URLs cannot be requested through the public handler.

## Streaming and caching

A normal GET remains one client-driven stream. At most one R2 object is open at
a time. The reader does not buffer an installer or an entire 64 MiB object.
`FixedLengthStream`, when supplied by Workers, preserves the native known length
of the public response and complete cache writes.

Caching is best effort. A complete part may feed a separate cache stream with a
1 MiB queue and an independent copy of each queued chunk, plus native in-flight
stream chunks. Only one fill may be active per request. When the cache cannot
keep up, the fill is aborted, further fills for that request are disabled, and the
authenticated public stream continues. Later parts do not wait for a previous
cache write. Cache write failures are observed through `ctx.waitUntil`; they cannot
turn valid R2 bytes into a download failure. Existing cache entries still fail
closed on incorrect identity, size, encoding, status or range metadata.

Small companions are fully size/hash checked before returning their bytes. Their
cache writes also run independently of the response.

## Byte ranges and the trust boundary

A request that intersects only part of an R2 object uses `get(key, {range:
{offset, length}})`. It reads that intersection, checks EOF at that exact bound,
and does not drain the rest of the object. Such a partial body is never stored
as a complete cache entry. Complete part intersections may still fill the full
part cache. A warm cache uses the Cache API's Range slicing and must return the
exact 206, Content-Range, length, identity encoding and pinned whole-part ETag.

Both full and ranged R2 reads require the native **stored whole-object SHA-256**,
total size, object key, upload version, strong ETag, custom SHA/source-installer
binding and identity encoding. A ranged read additionally requires the exact
returned offset and length; the optional native `suffix` property may only be
absent or `undefined`. Missing checksums, null/numeric suffixes, extra range
fields and truncated/oversized bodies fail closed. No alternate origin or
redirect is introduced.

The native checksum is the same R2 storage trust boundary used by the original
full-object path. It is **not** a hash freshly computed by this Worker over the
returned range. The client must still authenticate `release.json` using its
pinned detached CMS verifier, download every ordered byte, and verify the exact
complete installer size/SHA-256 before executing it. A partial checksum is never
represented as independent whole-object verification.

The existing deployment facade for R2's default full-range metadata remains
necessary where that runtime shape is returned. It only masks an exact complete
default range; explicit range options pass through unchanged. This change does
not edit that facade, release data, route configuration or publication tools.

## Local verification and release evidence

From the repository root:

```powershell
node --test mir2-web3/infra/cloudflare/mir2-full-installer-downloads/test/reader.test.mjs
```

Tests exercise actual Web Streams, blocked/rejected cache consumers, bounded
cache abandonment, client/request cancellation, known-length streams, cross-part
ranges, an 8 MiB read from a virtual 64 MiB object, invalid native identities and
malformed cached/ranged bodies. They do not establish Windows download speed.
Ordinary Windows cold download, installation and first launch require separate
measurement on the accepted 4-core/8 GiB/SSD, at-least-20-Mbps environment.

References checked against current documentation and workers-types 5.20261101.1:

- [R2 Workers API: stored checksums and ranged reads](https://developers.cloudflare.com/r2/api/workers/workers-api-reference/)
- [Cache API: Range matching and 206 put restriction](https://developers.cloudflare.com/workers/runtime-apis/cache/)
- [Cache API locality and tiering](https://developers.cloudflare.com/workers/reference/how-the-cache-works/)

Changing download distribution or updater metadata does not change existing game
bytes. Keep their actual source identity when reusing them; a new engine needs
its own truthful source and matching signed generation/feed bindings. Avoid
shipping loose files plus duplicate packed resources merely to hide first-launch
network transfer.
