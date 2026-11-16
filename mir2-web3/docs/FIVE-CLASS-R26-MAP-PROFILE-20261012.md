# Five-class R26 map-profile selection

The actual R26 package attempt failed before creating a Candidate because the
Node coverage report still selected `platinum_176`. The strict packager correctly
rejected its profile identity. Both profiles contain the same 208 runtime and
209 native maps, so checking the count alone cannot detect this fallback.

The two original Node calls now receive the selected `ContentProfile`. This
audit reads that profile's real JSON bytes and route sources, validates their
identity, and retains the exact D71653 resource-only topology and closure proof.
The global route default remains `platinum_176`; missing and unsupported CLI
values fail. No original acceptance, asset, attestation or CI gate was relaxed.

Actual checks pass all 15 Node tests with zero failure or skip. The original
PowerShell configuration/scope fixtures pass, including a new bidirectional
rejection of three-class reports in the five-class scope and vice versa. An
independent review accepts this source change and fresh CI preparation only.

[Exact scope and checks](../artifacts/qa-evidence/five-class-candidate-20261011/r26-map-profile-fix01.json)
and [unaltered red attempts, logs and actual review](../artifacts/qa-evidence/five-class-candidate-20261011/r26-map-profile-fix-original01.zip)
are retained. The earlier genuine 987-pass Windows artifact belongs to Source
02457 and is not relabelled. This change requires a new clean Source build and
V2 attestation, fresh packaging inputs, guarded normal packaging and paired
publication. No R26 package, server switch or human acceptance is claimed.
