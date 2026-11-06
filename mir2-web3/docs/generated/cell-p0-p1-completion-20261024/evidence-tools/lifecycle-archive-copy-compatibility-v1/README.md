# Lifecycle archive copy compatibility

The [observed preview failure](root-preview-v1-failure-facts.json) came from two byte-identical copies of one full16 run: the standalone lifecycle archive and probe18's prerequisite copy. Original game reports and sidecars are unchanged.

The verifier now requires exactly one owned realm in each report's own directory. Duplicate run IDs require identical report bytes before success/failure classification, and identical realm, metrics, audit and complete trace-name/digest sets. Every copy still passes the original functional and cleanup checks; identical copies count as one run. Conflicts, missing evidence and evidence borrowed from another directory are rejected.

The [implementation record](implementation-and-unit-result-v1.json), [source preservation](source-preservation-v1.json), [actual final test receipt](unit-execution-v2.json), [stdout](unit-v2.stdout.log) and [stderr](unit-v2.stderr.log) record 23/23 verifier tests with unchanged sources before/after. The original 15-test prefix is preserved; these tooling tests are separate from the capacity driver's 226 tests. Preliminary runs, preimages and diff artifacts remain in the referenced external evidence directory.

[Copy provenance](copy-provenance-v1.json) covers six explicit original records. No game or capacity result was promoted; P0 remains incomplete and P2 has not started. Root performs the integrated topic preview separately.
