# Native R17 importer — actual local workerd evidence

The archive binds 222 frozen files plus the original inventory and freeze.
Every regular member was independently read back as exact bytes without
extracting or executing the archive. The four Worker files match the integrated
importer. Local workerd 2026-10-01/Miniflare with compatibility date 2026-05-18
exercises native FixedLengthStream, DigestStream and local R2 checksum/conditional
write behavior. Outbound is an explicit closed mock; no credential or remote R2
request is made. All workerd instances were disposed.

Selected evidence covers 14 distinct cases and 152 successful assertions. Two
original small signed-feed objects first import 201, resume 200 without another
fetch, and a forced R2 creation race returns 201/412. Same-length corruption,
source guards, missing authentication and all 34 unavailable literal artifacts
store nothing; the pointer stays absent. No placeholder object is imported.

Failures are retained: attempt 02 is 10/11 because its extra-body assertion
expected 502. The mock supplies 1160 bytes but the HTTP bridge frames 1159;
follow-up R2 full GET proves the stored 1159 bytes and original SHA are exact.
Attempt 03 is 3/4 because its native-underflow assertion incorrectly requires
every upstream promise to reject. Attempt 04 validates the actual downstream
consumer/digest rejection boundary, without importer changes or dependency
injection. The short HTTP body really waits the original 120-second deadline.

This is actual local platform evidence, not remote Cloudflare authorization,
remote R2, CMS cryptographic revalidation, all 36 real payloads, live public
aliases, CDN behavior or a download-speed measurement. The original worker README
and all raw failed/passing runs are inside the archive.
