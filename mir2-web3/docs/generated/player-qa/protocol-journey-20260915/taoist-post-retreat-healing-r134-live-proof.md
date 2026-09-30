# Taoist R134 post-retreat healing proof

Trace: `C:\mir2-protocol-journey-20260911\Taoist.2026-09-15T22-33-13-539Z.trace.jsonl`.

The active-trace sample ended at seq **11377**, `2026-09-15T22:53:28.208Z`, with **22,700,351 bytes** and source SHA256 `5B4461B187D991E0F3D7A5EB5758C47D4BD6F40D676E20AA738BCADDFA380610`. This is a cutoff, not a closed final trace.

A real MP recovery/self-healing sequence was observed in D2031:

1. Seq **10221**, `22:52:17.192Z`: HP97, MP **2**, alive.
2. MP potion uid9 sent seq **10250**, acknowledged successful `UseItem` seq **10252**.
3. Seq **10321**, `22:52:22.304Z`: HP168, MP **42**, MP stack uid9 quantity9.
4. Self-Healing sent/acknowledged after fresh MP was restored:
   - **10446→10458** (`22:52:29.855Z`→`22:52:30.871Z`)
   - **10479→10495** (`22:52:31.819Z`→`22:52:32.708Z`)
   - **10719→10740** (`22:52:43.911Z`→`22:52:44.635Z`)

Thus the requested MP<6 → ordinary potion → fresh MP≥6 → self-healing chain is proven. The latest observed hostile set was CursedPriest `339502` `(34,201)`, CursedPriest `339504` `(33,190)`, and HungryZombie `340506` `(32,212)`. q89 remained Priest **2/3**, ShiZombie/CursedZombie **3/3**; no new mandatory unit was inferred.

Recent emergency-retreat diagnostics were seqs **11000** success, **11001** attempt, **11002** deferred, **11042** attempt, and **11087** success. All remained on D2031; the proof does not claim a map exit.
