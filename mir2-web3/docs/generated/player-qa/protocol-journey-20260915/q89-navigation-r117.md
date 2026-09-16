# Wizard q89 navigation repair, 2026-09-15

R116 returned to town twice without objective credit because navigation counted one directly attacking Shaman and one passive nearby zombie as two attackers. At sequence 7157 the owner had 53/100 HP; the passive zombie had no direct strike receipt. The next navigation emergency return occurred at sequence 7158.

Wizard q89 now opts into direct-attacker evidence during navigation. Two fresh, nearby living attackers can still trigger the existing .65 emergency-escape eligibility; one attacker triggers escape at the actual .25 critical floor. The combat .75 two-aggressor gate, legal nine-tile spell range, ordinary shop supplies and public travel remain unchanged. Other quests retain prior navigation behavior. Receipts are fenced by current map, current owner, five-second freshness and current life (Death/Revived or owner-matched ObjectDied/ObjectRevived).

Full controller regression: 620/620 passed, no failures, cancellations or skips; duration 11508.9387ms. Raw log: q89-navigation-r117-full.log, SHA-256 D45EF4AF527CAD2D883E7CDB0C1ADF5935FF65753F59582AAFA7D94CEADFF599. The coordinator checked the log and hash independently. Focused navigation: 36/36 passed.

R117 resumed the existing Wizard save on unchanged isolated Gateway R54 at 12:38:02Z. Live completion is pending; this repair does not certify the complete route, UI, animation, full 0-to-30 time or global Crystal parity.
