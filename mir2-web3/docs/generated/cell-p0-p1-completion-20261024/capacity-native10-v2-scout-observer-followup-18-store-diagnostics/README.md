# Probe18 Store diagnostics

The independent Store-only dataset contains 21 completed thresholded rows, with complete capture/count verification. Its largest completed single stage was 106ms (transaction Store hold); the threshold was 50ms. Completed records cannot exclude unfinished, subthreshold or uninstrumented holders, and overlapping/nested spans cannot be added. This dataset does not establish a gameplay root cause or contribute to game acceptance.

See the [catalog](store-artifact-catalog.json), [Store verification](store-public-verification.json), [capture status](store-stage-public-status.json), [base diagnostics](../capacity-native10-v2-scout-observer-followup-18-diagnostics/README.md) and [independent ended analysis](../capacity-probe18-ended-verification-analysis-v1/README.md). The short probe passed; capacity/P0 acceptance remains false and P2 is frozen.
