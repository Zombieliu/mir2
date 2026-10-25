# Formal11 attack timeout analysis

The original Formal11 run `1792942371568-1e322224` remains FAILED. Stages2/10/25 completed; ramp50 stopped at31 admitted actors. P0 remains unaccepted and P2 remains frozen.

- Request366 is a demonstrated driver polling false timeout: the exact owner V2 launched result arrived at888.3282ms and all required owner/observer damage evidence was recorded before the original900ms deadline. The historical wait loop could cross that deadline without a final predicate evaluation. This does not reclassify or promote the original run.
- Request367 has observer damage at about615.8ms but no owner typed result in its untruncated pending trace through924.5ms. Its actual late owner frame, if any, was not recorded after unsubscribe and remains unknown.
- Same-connection input / owner outbound gate spans of852ms and739ms support the owner serialization mechanism. The latter follows an attack queue-residence250ms span. Global flush/save/snapshot rows lack request/connection attribution, and receiver log times are not packet-send timestamps. Durations overlap and must not be added.
- The samples do not prove a CPU or server-capacity ceiling. Lifetime maxima are not phase maxima; monitor histogram deltas are global coarse brackets.

The source history is pinned to the actual5c2e execution commit and historical Node main1cf9/db4e. The original failed raw, monitor and slow-stage files remain external; this package copies only explicit sanitized diagnostic artifacts and scripts.

Evidence: [summary](summary-v1.json), [selected actions and packets](selected-timeouts-v1.json), [telemetry correlation](telemetry-correlation-v1.json), [input stage spans](input-stage-spans-v1.json), [historical driver source assertions](source-assertions-v1.json), [server path source review](formal11-attack-timeout-server-path-v1.json), [original public whitelist](public-whitelist-v1.json).
