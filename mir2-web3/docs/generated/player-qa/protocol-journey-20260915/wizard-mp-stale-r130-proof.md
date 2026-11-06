# Wizard R130 MP stale trace proof

Trace: `C:\mir2-protocol-journey-20260911\Wizard.2026-09-15T21-34-23-672Z.trace.jsonl`.

This is a bounded active-trace sample, not a final run. At the one read, it contained 8,985 JSON lines through sequence **8985**, timestamp **2026-09-15T21:52:12.049Z**, 10,898,339 bytes, sampled trace SHA256 `618967F80C6BB7020819784066119525B70C703ED5949911C58A651E20042A48`.

Facts observed:

- World snapshot seq **4325** at `21:42:38.176Z` showed raw MP **115/398** before the first potion use.
- All five sends used `uniqueId:20` and were acknowledged by received `UseItem` packets:
  - 4330→4331 (`21:42:38.885Z`→`21:42:39.169Z`)
  - 4348→4349 (`21:42:42.984Z`→`21:42:43.260Z`)
  - 4569→4570 (`21:43:01.645Z`→`21:43:01.979Z`)
  - 4678→4679 (`21:43:10.498Z`→`21:43:10.781Z`)
  - 4753→4754 (`21:43:20.155Z`→`21:43:20.556Z`)
- The received `ObjectMana` packets for object 1000 at seqs **4339, 4341, 4343, 4345** reported **31%, 33%, 36%, 38%** respectively.
- The last relevant D2031 world snapshot was seq **5216** at `21:43:56.156Z`: map D2031, position `(254,254)`, HP 100, MP **190/398**, and the MP-potion inventory projection was empty (`mpItems: []`). q89 was Priest **2/3**, ShiZombie/CursedZombie **3/3**.
- The exit sequence was sent `run` seq **5217** (`21:43:56.184Z`); the next map packets were map `11` seq **5288** (`21:44:09.426Z`) and map `1` seq **5593** (`21:45:10.897Z`). Snapshot seq **5297** showed map 11, MP 190, and no MP-potion inventory projection.

The trace proves successful `UseItem` acknowledgements, resource changes, an empty MP-potion projection at the last D2031 snapshot, and the subsequent run/map exit. It does not by itself prove why the restock gate made that decision; attributing the cause to a particular controller/source branch requires source inspection. The JSON artifact preserves the selected fields and explicitly marks the hash as a sampling cutoff, not a final trace hash.
