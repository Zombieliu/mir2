# q89 post-retreat recovery and earned funding — R134/R135

Completed route units remain **155/177**: Warrior 55+4; Wizard and Taoist each 45+3. Taoist's actual q89 Priest objective advanced from 1/3 to 2/3 in the preceding natural run, without completing a whole route unit. ObjectDied3817 for Priest339510 and owner snapshot3822 confirm this; the closed trace is 38,559,037 bytes, SHA-256 b08469ad86c74f35086fbcb0714a4225f03cc85b53a78b43efb77b1815d46875, EOF16702. See taoist-q89-second-priest-r132-proof.json/.md. These functional results do not accept UI, animations or paired original Crystal visuals.

R134 fixes a measured Taoist recovery omission: after retreat, exact MP stayed 1 with four held MP bottles while self-Healing was unaffordable. Only q89 Taoist post-retreat recovery may use the MP bottle, refresh a public exact world snapshot and then cast affordable self-Healing. The first evasive recovery callback retains other class/quest behavior; the second callback is newly supplied only for q89 Taoist. No offensive cast, invented mana percentage, added recursion or forced return is introduced. All 48 custom loadout cases pass, including unavailable skills, cooldown, default-off, other classes and still-insufficient post-dose exact MP.

R135 permits the existing bootstrap funding helper to finish already ReadyToTurnIn optional quests. Wizard q62 was inherited genuinely ready with KekTal/VioletKekTal 1/1; its normal Master_Shok finish on map1(312,73) rewards 750 Gold and 3000 EXP. The helper only finishes authoritative ready positive-gold quests and verifies completed state, exact wallet increase and item rewards. It never accepts or advances another quest. Mandatory chapter IDs and the 177-unit denominator remain unchanged. Its original helper regression remains part of the full stable controller run. Live R135 confirms finishQuest1456, ChangeQuest1457, CompleteQuest1458 and owner snapshot1459 completed/gold221→971 (+750). See wizard-ready-cofarm-r135-live-proof.json/.md. This is an active trace sample with cutoff2832 and SHA9480386537A92E4FE9CA3B7DF9481AB104CAB1364C8B811A99C7C80EAD52C7EA, not final saved-route proof. No acceptance in this run establishes its older co-farming origin.

The preceding Wizard expedition entered D2031 at sequence5002 then escaped with an ordinary TownTeleport: last cave snapshot7602 HP42/MP155, UseItem7599, ACK7634, map0 receipt7635. No player death or D2032 entry was observed. This is a low-health protected-blocker retreat, not an accepted cave route. See the two bounded independent trace reviews accompanying this note.

Independent Luna review found no release blocker. Syntax and scoped diff checks pass. All raw logs below preserve original bytes and disable normalization.

| Raw log | Result | SHA-256 |
| --- | --- | --- |
| q89-taoist-healing-mp-r134-focused.log | 48/48 custom loadout cases | 7242B57B094AFCF5BE518D31C8D34088243E3DD147D704911FB66BB6E1B1CCFA |
| q89-caster-r134-r135-full.log | 668/668 Node subtests; includes 48 custom loadout cases | 3EA46047111BEE5C3B9F44CEDE136F9C6E00CF1B31C07AE626B37B872E56400D |

Same ordinary roles restarted with the tested helpers: Taoist JT3f4a4e1f index10 at22:33:13UTC PID92424/PTY20421 (Taoist.2026-09-15T22-33-13-539Z.trace.jsonl); Wizard JWdebac6d4 index9 at22:33:23UTC PID94836/PTY42231 (Wizard.2026-09-15T22-33-23-712Z.trace.jsonl). Natural gateway R54 and persisted progress are unchanged. No admin/item/money/level grant or save injection is used. The prior Ctrl-C stops are partial disconnect saves; they are not final normal-logout acceptance.
