# Newcomer V2 graduation acquisition audit

The Native and Web handoff is a display-only candidate: all 22 main quests,
four growth claims and authoritative level 30 must be present before the three
goal choices appear. Selection grants no item, EXP, skill or quest completion.

The equipment targets are real level-30 items: JudgementMace 244,
WarMageStaff 259 and SoulSpringWand 272. Their manifest prices are 50,000,
50,000 and 40,000; these are catalog values, not verified vendor quotations.
No normal P176 NPC stock was verified. GM/GM-Weapon.txt is administrative
and must not be used as an acquisition path.

One normal, active P176 drop source is verified for all three:
`WoomyonWoods/WoomaTemple/WoomaTaurus.txt`, table 1594, Weapons entries
7/8/9, each 1/90. WoomaTaurus 256 has level 60, HP 3,000 and DC 35–90.
Its D024 WoomaTemple spawn is at (50,50), count one, 60-minute delay.
P176 recommends D024 for levels 37–43. Normal movement rows connect
D021 → D022 → D023 → D024. This proves an ordinary source, not realistic
level-30 acquisition or an acceptable immediate newcomer objective.

Skill book targets have source-backed future hunts: ShoulderDash 977 and
ThunderStorm 1001 from EvilCentipede in D606 Life Death Coffin; Purification
1028 from WhiteBoar in D714 Angled Stone Tomb B4. These encounters have
HP 2,000 and recommended ranges 31–36 / 33–39. Acquisition and class skill
animation remain unverified by an ordinary run.

The shared free-roam challenge is D605 Insect Cave N 2F, recommended 30–35.
Its verified normal entrances are D602 Insect Cave W 1F and D604 W 2F.
Centipede 152 has HP 210 / count 77 and Tongs 158 HP 240 / count 70, both
with three-minute respawn. Dynamic density, survivability and actual travel
are open gates. This choice does not create an instance or artificial quest.

Source manifests: crystal_item_manifest.json, crystal_monster_manifest.json,
crystal_respawn_manifest.json and crystal_drop_manifest.json under
packages/game-data/data/generated, with content_profiles/platinum_176.json.
The graduation equipment copy deliberately promises no vendor, reward or
easy level-30 acquisition. Equipment choice is not yet an actionable completed
handoff; source suitability needs further product review before acceptance.

`accepted=false`, `visualAccepted=false`, `ordinaryGraduationAccepted=false`.
