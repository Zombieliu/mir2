# R23 paired release evidence

R23/sourcefaef, signed feed18 and the same-source playtest Gateway are live.
EVIDENCE.json binds the actual CI/stage/promote/native/public receipts and
complete credential-sanitized protocol traces. No full Goal/human acceptance
claim is made. See [the released scope](../../../../CLASSIC-TARGET-MOVEMENT-PUBLISHED-R23-20261009.md).

The actual original7,449,081-byte archive is transported as three bounded
regular files to retain the existing3MiB Git API blob gate. Download all three
parts, EVIDENCE.json and Restore-original-evidence.py into the same directory,
then run `python Restore-original-evidence.py`. The script verifies every part
and the whole SHA/ZIP CRC; it never overwrites an existing original archive.

- [Part001](original-release-evidence.zip.001)
- [Part002](original-release-evidence.zip.002)
- [Part003](original-release-evidence.zip.003)
- [Verified reconstruction script](Restore-original-evidence.py)

Concatenation reproduces every byte of the original archived evidence; this is
only a transport representation. The first local oversized-blob rejection is
retained in the Root publication03 records and did not advance the remote ref.
