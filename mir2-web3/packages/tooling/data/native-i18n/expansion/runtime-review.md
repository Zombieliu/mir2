# Runtime integration corrections — 2026-09-30

These overrides correct findings from actual native integration, rather than declaring machine drafts reviewed.

- `server.CustomNameMastersPet`: retain both opaque parameters and the authored underscore, with explicit pet/companion wording. An Arabic draft containing only two slots separated by a space matched unrelated player prose as a global template. Runtime template indexing now rejects slot/whitespace/punctuation-only patterns; direct stable-key substitution remains available.
- `npc.prose.3be7ce1249d2246a`: preserve the source sentence ending immediately at the opaque held-item argument. Do not append punctuation to a standalone wrapped player/item value or translate it a second time. The separate wearing-item key remains owned by `review-overrides.json`.
- Reject translations that lose every authored letter while retaining only parameters, punctuation or numbers. This caught collapsed collection/progress/login/status messages and NPC separator headings. The OC section code remains literal because its expansion is not established by that prose line.
- Hindi and Vietnamese buff quantity connectors are corrected in the content generator that owns those two keys, not shadowed by conflicting runtime overrides. Increase/decrease, stat, amount and unit remain separate original arguments.
- Vietnamese Hell Knight variants and Arabic DY Smithy now have explicit display translations with their original variant numbers/code preserved.

All six overlays pass the source-key, parameter, command, number, replacement-character and review-provenance checks after these corrections. This is structural and selected-string review, not native-speaker acceptance of the remaining offline drafts.
