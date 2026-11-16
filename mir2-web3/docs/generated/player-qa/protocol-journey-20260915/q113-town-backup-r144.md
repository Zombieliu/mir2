# R144 q113 caster TownTeleport backup

Live R143 traces exposed two bounded escape gaps. Wizard q113 exhausted RandomTeleport while retaining two TownTeleport scrolls, then the emergency dispatcher returned false before any public UseItem request. Taoist q113 used a successful RandomTeleport at 2/191 HP but died during the existing three-second critical cooldown because the route carried no TownTeleport reserve.

R144 gives Wizard and Taoist q113 a normal town departure reserve of two TownTeleport scrolls with no field restock floor. Wizard remains RandomTeleport-first while any random scroll remains, using TownTeleport only after depletion. Taoist uses TownTeleport at its existing 35% emergency threshold or after RandomTeleport depletion. Safe-zone, dead or missing player, and no-stock cases fail closed. q89 behavior and all quest requirements remain unchanged.

The controller tests exercise the actual runner dispatch and journeyRestock entry points. This proves wiring only; live q113 completion remains pending. Actual route count remains 157/177. No level, item, currency, quest progress, or completion record was granted or edited.

Full regression 689/689 Node subtests, including48 custom loadout cases. Raw q113-town-backup-r144-final-full.log; SHA256 4CCEFE791863293ED0A4E64017262FCEBB441D916FB49E4A82C704AE35E5AE0F.
