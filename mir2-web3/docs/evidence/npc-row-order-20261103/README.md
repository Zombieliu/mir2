# NPC row-order fix — 2026-11-03

Same private ordinary character/map at 288,615, before and after native restart. Image04 is a crop-only side-by-side of images02/03; image05 records a normal walk to288,614. Comparison screenshots used native SHA 7e42ab1ce39fb7cd6fa7d993a37d377e16fe803bd1202449126c1c2cd3eed9ea; final packaged SHA is in source-inputs/verification. Any additional final-EXE smoke is recorded separately, without relabelling older screenshots.

Native groups: 2/34/2/41/56/4 passed (139 selected functions); standalone runtime26 passed, including legacy depth fallback and native sortZ retained-frame recovery. Full original R16 resource SHA/CMS verification:255063 files. No model calls this round. Original character/model private state is not copied into this evidence.

Initial obsolete expectation and shared cdylib/rlib cache failures remain described in test-groups.json; raw initial logs are retained at F:/mir2-builds/npc-row-order-20261103. Runtime Cargo.lock was restored byte-for-byte. This is a local development repair, not a new signed public release or human Accepted. See delivery.json after installation.
