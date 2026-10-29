# Original mining collision fixtures

These are byte-identical copies of the existing tracked Crystal map pack, not
invented floor/wall geometry. Lowercase filenames match the production loader
on case-sensitive CI and Windows. Ordinary production
resources continue to come from the normal map pack.

| Fixture | Original repository path | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `0.map.gz` | `apps/web/lib/generated/crystal-map-pack/0.map.gz` | 512762 | `bbd02c7d0125fe78983e2dea5f4aecc73b53d4b91517bde6fdbee9853bb183a5` |
| `d401.map.gz` | `apps/web/lib/generated/crystal-map-pack/d401.map.gz` | 22951 | `734f2c5ce06defd46ebb3fc9a7d02a38c00ac7b070be03b317b0e6fa9aa78a71` |

Tests default here relative to `CARGO_MANIFEST_DIR`. An explicit
`MIR2_CRYSTAL_MAP_PACK` takes precedence. Use `--test-threads=1`, because the
existing original-map collision opt-in is process-global.
