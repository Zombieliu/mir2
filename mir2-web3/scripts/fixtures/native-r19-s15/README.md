# R19 / signed sequence 15 fixed publication

This is a new bounded channel. It binds game source
`2b04e82c4f42b22f895f5f95b5bd79a8147eacdf`, Candidate
`WN-CANDIDATE-20261007-invited-19`, engine source
`b4390ee4c987bbf000ba1d95761151e39fc0d54f`, and the actual signed
sequence 14 predecessor. Source and root-receipt raw hashes are frozen in
both the Worker and CLI. Proof Base64 lives in code so the fixed request/table
remains below the unchanged 64 KiB control limit.

`GENERATOR-REPORT.json` witnesses full size/SHA checks of every actual source
file: 40 objects, 34 game objects, 1,051,831,921 bytes. The native JSON is
28,137 bytes. Limits remain at most 100 objects, 32 MiB per metadata object,
128 MiB per artifact, with the existing stream/deadline/TLS/CAS policies.
The generator is offline and does not independently reverify CMS. The raw
`actual-cms15` receipt was issued by the root's actual Windows CMS verifier;
the generator authenticates and binds those exact bytes.

Actual local checks:

- `qa-runs/generator-actual-03.log`: full real closure generation passed.
- `qa-runs/node-02.log`: 138/138 Worker policy tests passed, zero skips.
- `qa-runs/python-01.log`: 75/75 CLI and generator checks passed, zero skips.
- `qa-runs/interrupt-01.log`: Windows actual SIGINT passed; external GNU
  timeout check is explicitly skipped pending the Linux CI runner.

The unit transport, R2, `DigestStream` and `FixedLengthStream` are models.
Only the genuine small signed feed pair is consumed as actual object bytes in
the transport suite; synthetic large bytes deliberately fail the real EXE SHA.
Modeled remaining objects or mocked CMS do not prove CDN/origin/network
publication. Only the generator consumed every actual source artifact.
Cloud writes, stage/promote, native HTTPS and public deployment are separate
root-owned acceptance gates.

Retained failures:

- `generator-template-01.log`: Windows path/handle ctime mismatch; corrected
  with same-API full stamps plus cross-API device/inode/size/mtime matching.
- `generator-actual-01.log`: CRLF template hash-anchor substitution refused
  before output creation; the new source is emitted as fixed LF bytes.
- `node-01.log`: 136/138 passed; two old test constants retained the previous
  EXE size and 36+pointer count. `failed-generation-02` preserves that test
  source and its original generator report. New assertions bind the actual
  R19 size and complete current table; CAS winner assertions remain intact.

`qa-runs/fixtures` and `qa-runs/interrupt-fixtures` are retained local negative
fixtures, including deliberate links and malformed outputs. They are test
artifacts, not publication inputs. Portable proof fixtures are the raw
`actual-cms15`, `predecessor14`, and `preview/artifacts/feed` files.

Run from the project root:

```text
node --experimental-strip-types --test infra/cloudflare/mir2-r2-bulk-upload/test/native-r19-s15-publication.test.mjs
python -B scripts/test_native_r19_s15_r2_delivery.py
python -B scripts/test_native_r19_s15_interrupt.py
```

The generator accepts only the root-issued immutable plan/receipt, feed pair,
and exact Bootstrap executable via its three required input arguments. It
refuses changed existing outputs before writing any new output. No secret,
private key, URL/host/key override, network action or old-channel write is used.
