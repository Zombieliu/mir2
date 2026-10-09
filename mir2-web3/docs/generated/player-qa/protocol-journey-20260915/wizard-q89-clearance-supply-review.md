# Wizard q89 D2031 clearance and supply review

Trace: `C:\mir2-protocol-journey-20260911\Wizard.2026-09-15T22-04-24-777Z.trace.jsonl`; bounded stopped segment seq **5002–7635** (`22:15:24.216Z`–`22:18:48.864Z`).

Facts:

- Sent offensive spells: **78** total: GreatFireBall **47**, FireBall **31**. Received authoritative `Magic` and `ObjectMagic` acknowledgements: **71 each** (142 paired acknowledgements). Targets progressed through Shaman IDs 341105, 341100, 341109, 340210, 340903, and 341103; spell casts used the normal alternating GFB/FB pattern where available.
- Authoritative damage indicators totaled **89**. Damage to the four observed CursedShamans: 341105 **205**, 341100 **205**, 341109 **205**, and 341103 **77**. Damage to ShiZombie 340903 was **230**. The player also received **135** damage; attacker receipts at seqs 7439, 7479, 7490, 7517, 7534, 7555, 7564, 7583, and 7593 show simultaneous/overlapping CursedShaman attackers 341103, 341108, and 340207, followed by HungryZombie 340610.
- Four CursedShaman `ObjectDied` receipts were observed: seq **5238** (341105), **5448** (341100), **6169** (341109), and **6545** (340210). These are clearance kills, not q89 objective Priest kills.
- MP-small useItem receipts: uid20 once (send seq 5203, ack 5204); uid23 **seven** times (send seqs 5349, 5496, 6056, 6236, 6272, 7217, and 7530). The replenished uid23 stack was 11 at snapshot seq5205 and 4 by seq7532/7602; uid20 remained 1, so literal aggregate MP-small stock at the exit snapshot was 5.
- Last D2031 owner snapshot before escape, seq **7602**: `(246,217)`, HP **42**, MP **155**; RandomTeleport 1, TownTeleport 1, MP-small uid23 quantity4 plus uid20 quantity1.
- Exit was caused by `navigationEmergencyEscapeAttempt` seq **7598**, uid22 escape use seq **7599**, acknowledgement seq **7634**, and direct map-0 `MapInformation` seq **7635**. No player death, D2032 fallback, or ordinary door entry occurred.

Source comparison:

- `protocol-survival.mjs` `minimumJourneyMpStockForQuest` returns **4** for Wizard/Taoist by default; q54 is 12 and Taoist q42 is 0.
- `run-protocol-journey.mjs` supplies the q89 Wizard expedition departure floor through the journey floor and passes the MP trigger to the supply gate. The existing q89 Wizard setup uses protected Shaman clearance **6**, firing approach distance **7–9**, and at most **2** protected blockers per bounded attempt (source around lines 925–939).

Assessment:

The expedition had enough MP inventory to satisfy the field trigger and did not leave because the MP count reached zero. It spent eight replenished MP units while clearing four Shamans and continued casting while multiple Shamans shared the player’s combat area. The direct evidence supports a supply-pressure interaction: the q89 departure reserve is consumed during the clearance/approach, leaving HP42 and MP-small aggregate5 before an emergency escape. It does not establish that simply increasing the MP target would prevent the multi-Shaman overlap; the trace shows the protected six-tile clearance/7–9-tile firing policy was exercised but still encountered multiple attackers. No game economy or drop-rate conclusion follows from this run.
