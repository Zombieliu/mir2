import { readFile, writeFile, mkdtemp, lstat, readdir } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { FIXED_R16 } from '../../mir2-web3/apps/web/scripts/lib/fixed-r16-assets.mjs';

export const TEAM = 'team_pXY7oaLtibSTlKSmKXMXzrQR';
export const BRANCH = 'codex/windows-player-journey';
export const REPOSITORY = 'Zombieliu/mir2';
export const SITES = Object.freeze({
  original: { project: 'prj_Bl3ttB4PzcdCpJMYLRhzQjkdXyTz', name: 'mir2-web3-web', baseline: 'dpl_3NBhFUPKxNF4nccRnG94ZhDq1iz6' },
  main: { project: 'prj_Q14gZuvjxkfqOCExhqOrYiR2UbxA', name: 'numeron-legend-of-rebirth', baseline: 'dpl_22qaP2Cn1my46GJk6xrS9tk8m4RK', root: 'mir2-web3/apps/website' },
  game: { project: 'prj_QzS3a2wirTJCixU4tYRVY5nmvSjC', name: 'numeron-r16-web', baseline: null, root: 'mir2-web3/apps/web' },
});
export const ORIGIN = 'https://numeron.obelisk.build';
export const QR_RELEASE = Object.freeze({
  source: "aa524edc76d85e01f2ae8963957bbacd9728eac2",
  sourceRef: BRANCH,
  baseSource: "5d7bf8c2ea58d2ac68bbca5f18c8240fdc929d45",
  assetSource: "f2de762309768b51b39438c2437c14e819df9bf7",
  sourceCommits: [
  "1cac973aa7dfc5a563a05012182341cd0fa22a49",
  "4e43d8ddcb9506cd4f0673335066b65a525a8827",
  "fef34fb07a23880c2e3ac7a87bd79d811677b6ba",
  "8359824ff7d618704121bfb83bff39422b4ad380",
  "6623f15feac980f2db1a57ead74af34301264cec",
  "81c51a58e903c1ec2e55f31c85d675aedbc7fe22",
  "4d30fb8d1275304e17bf3722186d8ea14b2fc6ae",
  "c1d4c85c89e3f1960d349c823cb3a42976e2b2e6",
  "ec26ffa12e79f302bd82f05f97c0489e9d6ca464",
  "9337f00a3665f0af082ea92e57987ae813e52609",
  "f0eb9ea6121080763ec1cad07dd316b10b6febe6",
  "dca152cd15fc5ab9782fd28f2247d89c5faa9e6a",
  "e9aa44751e4ffc1d91a926f22fb8175f585be088",
  "acbe3aea4bf5aaa5ce98efe76807da99427e9575",
  "aa524edc76d85e01f2ae8963957bbacd9728eac2"
],
  changedFiles: [
  {
    "filename": ".github/scripts/numeron-r16-release.mjs",
    "sha": "324b62d9cb2a9c138acd0b3cbb5b42489f26de12",
    "status": "modified"
  },
  {
    "filename": ".github/workflows/promotion-website-release.yml",
    "sha": "6967b8c9af6da7c4ca5962d2f0ce67e7fbefa92f",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/gateway/src/routing.rs",
    "sha": "65607af77b7ec536170b6382f52a091573f44bdf",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/gateway/src/shared_experience_profiles.rs",
    "sha": "c76ecb56fe1273a58dfebce7e370a71dfe0ab2ca",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/gateway/src/zone_experience_admission_handoff_tests.rs",
    "sha": "20b35f2df4fe5791645c76ba278666994ebd797d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/simulation/src/runtime/zone/runtime.rs",
    "sha": "1474264f0fe21c09120055a6ce4665de0ea5cbcf",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/simulation/src/runtime/zone/runtime/idle_roam.rs",
    "sha": "d31455fc5880e2b22ae0b7c2bca1b6a4f5ee1af2",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/simulation/src/runtime/zone/runtime/passive_ai.rs",
    "sha": "22d35c4349920a3447ffbd72228c5281d724cc03",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/simulation/src/runtime/zone/runtime/special_ai.rs",
    "sha": "d36176bfefa09e01a13af2290f0bf5556df3d732",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/simulation/tests/shared_monster_idle_roam.rs",
    "sha": "50eab89284f697879a1d29bfbbb809447d912dd9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/simulation/tests/shared_monster_passive_ai.rs",
    "sha": "5e7bf1f4a4408d16d7746a63d2c444d038c927cf",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/app/components/original-client-shell-types.ts",
    "sha": "a1ee05cca3ca8d43e8ad15b947415fd574ee32e0",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/app/original-client-shell.tsx",
    "sha": "66fbd96a0cc00c85ade6ca57cf0717566a0d8f0f",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/app/page.tsx",
    "sha": "f2080da34fc053f560a203f12891f92dd9131ee7",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/lib/r16-entry-scene.ts",
    "sha": "fa42c1ff7a90df8785efc41e84fc620d840b954a",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/lib/use-bevy-quest-ui.ts",
    "sha": "99c775595cce01ef634d2228b1c28bbff2ee4354",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-bevy-quest-ui.mjs",
    "sha": "53797b30d7a5eb0c2faf7449e5633624a8b81d94",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-map-route-feedback.mjs",
    "sha": "a05e933dccfb5babf15db0ce692059de15cd08dd",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-pinned-bootstrap-chunk.mjs",
    "sha": "3b3bbef33250f0d9377160e78ab0767a3ead4a73",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-r16-quest-close-shortcut.mjs",
    "sha": "9f72cbba74c8e9cdb74f34169f9a7d5309da3b09",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/AGENT-CURRENT.md",
    "sha": "6fe5c3bc8d4bfead60d796918aeb7838ba2bef81",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/AGENT-TASK-QUEUE.md",
    "sha": "3e11c0e41d99ae10932d1764667fdd0f9ef3ef2c",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/BACKEND-1TO1-PROGRESS.md",
    "sha": "d0d8cbbace973e08da8f1e786292a82b56c46e0d",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/CRYSTAL-1TO1-ROADMAP.md",
    "sha": "20b95e67ffd87232dc6c736246a14829fb3f17fb",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/CRYSTAL-SERVER-PARITY.md",
    "sha": "b8cca2fb29ff8cf89dc8548ec549e0781f781d0f",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/FRONTEND-1TO1-GAPS.md",
    "sha": "253fde8b1b21de4153c019b1440a5d1dc952dfaa",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/README.zh-CN.md",
    "sha": "b4c380f66e93211f77ffe386fee0d85e8150421e",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/zone-admission-handoff.calibration2.closed.json",
    "sha": "731ece44df2fb02d35f1522ee43d5d4fa9c0d189",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/zone-admission-handoff.default100.closed.json",
    "sha": "b0cd08df94627801ec36aefed1510c5e162b9cfa",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/zone-admission-handoff.integration.json",
    "sha": "84f8bd178f06c002f53a10b156345fd0e00bc064",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/zone-admission-handoff.linux-native-subset.json",
    "sha": "267b049669701bfc70e0575f168de1944ea8f0f8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/zone-admission-handoff.native35.json",
    "sha": "90709502d7784fbc078d0241a151b130eab33ae9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/zone-admission-handoff.selected-trace2.closed.json",
    "sha": "01116c260135b3dd58bf90f3b783f59ae3e1efb8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/zone-admission-handoff.selected-trace2.log-coverage.json",
    "sha": "4ce967d37b4e32eb17bdbb907e9925dc7b28d2d4",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/zone-admission-handoff.windows-release.json",
    "sha": "6f2b0fb573b382f44ef71b76f750221fe9c24a3d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/README.md",
    "sha": "a4b239549f0d994646cc24a9d8af0e967aa5b583",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/broadened-fixtures.log",
    "sha": "aa2c314936ce15be7bb033f12c93626f820bc920",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/checks.log",
    "sha": "e3e145b7fadf122abcc4b9324fa2a6e928b0c500",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/default-arrival.png",
    "sha": "3b351f2b8ba219ef842b3bdbcd71ebcf6ff2c6ff",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/default-blocked.png",
    "sha": "6a7baf8f57100846d9d73f72d9f9f572f500adeb",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/default-diary-open.png",
    "sha": "3d3fffe2ea60976bc80039a97f2514b856177768",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/diary-before.log",
    "sha": "58a4d7076da17301412858040c29fde5c9d5b273",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/inherited-fixtures-baseline.log",
    "sha": "9b6c92f2f9954212533476643b43001d7775da07",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/private-receipt-pins.json",
    "sha": "b6f5c2e5902c116321eef8a38dc9addbd3e985d5",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/public-state.json",
    "sha": "ef419ea6711440de4402a7bc7711a9e2665b9c8d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/qualification.json",
    "sha": "d9b18691cc3b903889447896e739b39fca6c1bb2",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/shared-arrival.png",
    "sha": "126073a182548d0a9bb90e56eaee9109d85d6ca7",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/shared-blocked.png",
    "sha": "3be3e7758e3184f90d01c707a539f203fc562eca",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/shared-diary-closed.png",
    "sha": "d75b9221b110127bd7b6356b2e29bc8dcb6ebfa5",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/source-binding.json",
    "sha": "dd17f03e6439ad38f05aee84ad843bad56923cfd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-close-20261115/unreachable-before.log",
    "sha": "fc1211cbe70eb51c603de953ca53a2355f17bd29",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/README.md",
    "sha": "ebd163f49deb015af4793db1b84c4a5723feab9a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/baseline-fixture-correction.log",
    "sha": "3cfa5e01ce634b1c81cdcf049957300ab445f3b5",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/before-roam.log",
    "sha": "6f428e73036fdb8232d6063e46aed0bf18b22e6e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/checkpoint.log",
    "sha": "438b2eac5599bd49bcb7423e4f2d9a5289f064e7",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/default-natural-walking.png",
    "sha": "d73d1e72ea5dca83022f3bce124ecd210549a992",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/default-reopened-diary.png",
    "sha": "8a8fefb88419a48a6eddaf0e1d0ce7079f214d42",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/idle-roam-unit.log",
    "sha": "d86dcdeedb827adabe943f5bd10647ae9caa7091",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/integration.log",
    "sha": "1c1a3fc77a4a97f44961df4a6e1f818e24d7bf30",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/private-receipt-pins.json",
    "sha": "e4c01469063b68242e24437ec1997a242f1f135a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/public-state.json",
    "sha": "cae789d88c9ddc36a4a61f54048d059e44deb2f0",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/qualification.json",
    "sha": "90e919d03617415b4898a2de8cf1f739f24071c0",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/shared-natural-walking.png",
    "sha": "3c4d8f01603e2a9181634030dc869dca34e74dc0",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/shared-reopened-diary.png",
    "sha": "fa309226d8d2f429e929b35cb800c6b85684b2ec",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-livestock-roam-20261115/source-binding.json",
    "sha": "549a6f14299a7cfe62b80ab0be48dd38d966394e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/README.md",
    "sha": "8115367c00787c291159a32183ecef46f954842a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/aggregate-after.json",
    "sha": "8e260df78013c57830d0a46d039339af0929c5ef",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/aggregate-after.png",
    "sha": "af6b1ca55dd7b30a1508b145e5c96c97e9109218",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/aggregate-before.json",
    "sha": "97d6dfa5ce3667e79756d72df4b211f845ac0e0a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/default-actual-game.png",
    "sha": "f69b9313d326850b4f7acb1e66c29981c8619d69",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/default-original-fonts.json",
    "sha": "ea6c83db6b3e613fd14aa34d50f9251d380a905d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/independent-after.json",
    "sha": "8de0d8490af4d6cf66f78bc34c1399b24f6be149",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/independent-after.png",
    "sha": "af6b1ca55dd7b30a1508b145e5c96c97e9109218",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/independent-before.json",
    "sha": "28494d1252cb138d2caf3a1b062ad8382f7921f1",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/initial-loading-source-comparison.json",
    "sha": "a81500c6e03bf815b1cce651147c47463303ba60",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/production/README.md",
    "sha": "79f27c1b5b5b79b4170c656b8a4d7cb847d206bc",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/production/hud-after-worker-restart.png",
    "sha": "f2a2135077e0f0b154e9b7ee43f7468e4e8052b9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/production/original-acceptance-r1.json",
    "sha": "25f2ce436c18e02a0b39eb6fe39bb3fa3dad0ba0",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/production/private-receipt-pins.json",
    "sha": "5f8cc638fe11cc54d2e56bad00341457645f71c9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/production/public-r1.json",
    "sha": "f15ac5b17aab876d800fcfe3f51dfef2c0622434",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/production/quest-failure.png",
    "sha": "b41da0298e00238d7160d0d695d244feb207e72b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/production/quest-owner-observations.json",
    "sha": "c2f261f65ed4f3e632c48355fd43e7a3567e3604",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/production/rollback-qr.json",
    "sha": "e544e0acdae7283985d9ffde4cafb6265029e156",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/production/rollback.json",
    "sha": "a1eca7a12b2d76d35cb7b3219aedcd4e8ce739cc",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/qualification.json",
    "sha": "4c27ec41ad609dcd4123e5c5082c461f0e9ccd69",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/regressions.log",
    "sha": "6eb3f7e31502e2c8038ff1f46a81b40270e95741",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/release-checks.log",
    "sha": "2e38ce8d9d12f358e2dfd55529a57883b2b97924",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/release-source.json",
    "sha": "679c86881101ca64f901ced3ac867d5eab191de5",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/webgl2-actual-game.png",
    "sha": "623851e6d855e5e4bafbee7223a7273d059b7f4f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-primary-font-overlap-20261115/webgl2-original-fonts.json",
    "sha": "ea6c83db6b3e613fd14aa34d50f9251d380a905d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/README.md",
    "sha": "b0c19b2ad083fb49f2a0497022280ecb18b93545",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/adjacent-checks.log",
    "sha": "4363a70e26977ab84ff925125b02251adf8d2b4f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/concurrent-before.log",
    "sha": "4536115dd2ad3e9b5bce928ae6e5f74906783044",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/default-reopened-diary.png",
    "sha": "2a2e8230cadaccd479a24f38bbd7db432dc52c5c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/default-town-frame-missing.png",
    "sha": "e0991981b25b30657b1a3f4e12888a041b6f13a9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/initial-live-delivery.json",
    "sha": "b61dfee8ad98f05a96d337e6eb678f192fabd024",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/pack-checks.log",
    "sha": "4bd759ca67e11968ac239d344ce4d532e26b9c62",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/priority-before.log",
    "sha": "9cf01a517ceef2ad8f92d4f8762b5c39e6f60d8c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/private-receipt-pins.json",
    "sha": "ba9aedb02cdc4e132cebbbd4b098145f18ec13a1",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/public-state.json",
    "sha": "9aa0f481f2cf646db28ea0bf5311033d791e9993",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/qualification.json",
    "sha": "3b0e1405ae47bb1b0d29f465fc5e8be1b6d78a60",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/shared-reopened-diary.png",
    "sha": "cbdf1340cb2e86936adaf7a9e2ff738c57eb4e87",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/shared-town-frame-missing.png",
    "sha": "6e095264fa44c9c92309022d5c8204f645ca4b59",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/source-binding.json",
    "sha": "66b9b34f603e81d3bdb2f23518548b314a1dd598",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-delivery-20261115/standing-diagnostic.json",
    "sha": "bd848c10e51d90ba86a964cfe0e1555ef781d96d",
    "status": "added"
  }
],
  entryScene: {
    path: '/bootstrap/r16-scenes/5c08d1fd4e81d92a7be20eb98c4c6288515749d5d459978dcf78fe2b86bdf325.json',
    sha256: '5c08d1fd4e81d92a7be20eb98c4c6288515749d5d459978dcf78fe2b86bdf325',
    bytes: 458475,
  },
  entryFloor: {
    path: '/bootstrap/r16-floor/e4977afa3640bec9d8043ae32d794f92385a309fb4fa68cf407fe575016ce90d.json',
    sha256: 'e4977afa3640bec9d8043ae32d794f92385a309fb4fa68cf407fe575016ce90d',
    bytes: 439896,
  },
  entryPlayerMale: {
    path: '/bootstrap/r16-visuals/164d9bec51c291f17757e3c74d7e2d128d9747f5723165bc1a25c99f94445d08.json',
    sha256: '164d9bec51c291f17757e3c74d7e2d128d9747f5723165bc1a25c99f94445d08',
    bytes: 289851,
  },
  entryPlayerFemale: {
    path: '/bootstrap/r16-visuals/0f52c6a6632c44eea1ab3cf37eb58da52f6c786cf3573813d595fcdd8ca040d0.json',
    sha256: '0f52c6a6632c44eea1ab3cf37eb58da52f6c786cf3573813d595fcdd8ca040d0',
    bytes: 282415,
  },
  entryNpcs: {
    path: '/bootstrap/r16-visuals/9093bb29d7f91ab07a44a47e888e2f1d40151130c1253ecf9451af63c4e057d9.json',
    sha256: '9093bb29d7f91ab07a44a47e888e2f1d40151130c1253ecf9451af63c4e057d9',
    bytes: 313909,
  },
  entryObjects1: {
    path: '/bootstrap/r16-visuals/2530a00967ee0aa60d4802ab50690166d676c2c58b13567dc1fdde9951e0f5b5.json',
    sha256: '2530a00967ee0aa60d4802ab50690166d676c2c58b13567dc1fdde9951e0f5b5',
    bytes: 395412,
  },
  entryObjects2: {
    path: '/bootstrap/r16-visuals/8d4ebe2ab7644fb7abf46cebab8c04c52747a3202049630948ef7726c581d89d.json',
    sha256: '8d4ebe2ab7644fb7abf46cebab8c04c52747a3202049630948ef7726c581d89d',
    bytes: 395320,
  },
  entryObjects3: {
    path: '/bootstrap/r16-visuals/348dbb84596b95d7bfd065afb78cd4a582df808965b880e06fa5dbd1be05c0d1.json',
    sha256: '348dbb84596b95d7bfd065afb78cd4a582df808965b880e06fa5dbd1be05c0d1',
    bytes: 395216,
  },
  entryMonsterStanding: {
    path: '/bootstrap/r16-visuals/d3f04c400820d8cd66b49b8af638a86e719bcf1032082be5a7ec80757bdf4308.json',
    sha256: 'd3f04c400820d8cd66b49b8af638a86e719bcf1032082be5a7ec80757bdf4308',
    bytes: 513566,
  },
  entryMonsterWalking: {
    path: '/bootstrap/r16-visuals/41fcaae2757ad0351da2e3840bf54008952b5c2714441530bb64c21f5c9fd64f.json',
    sha256: '41fcaae2757ad0351da2e3840bf54008952b5c2714441530bb64c21f5c9fd64f',
    bytes: 764724,
  },
  entryGameplayLocale: {
    path: '/bootstrap/r16-locales/b373a5f2cd21d5d0270610e0b7d1d619d5e554e12a67a7190c768d59e01dd64c.json',
    sha256: 'b373a5f2cd21d5d0270610e0b7d1d619d5e554e12a67a7190c768d59e01dd64c',
    bytes: 3283113,
  },
  entryNameAssistantJane: {
    "path": "/original-ui/gdi-text/images/entity-assistant-jane-auto.png",
    "bytes": 366,
    "sha256": "0b8664e2a2385a2ab28409a093bee5f3745f8f591abffa882a23b308fbd7c88c"
  },
  entryNameMerchant: {
    "path": "/original-ui/gdi-text/images/entity-merchant-auto.png",
    "bytes": 288,
    "sha256": "99c2b6e88ad38051cecfbaee8e1bc1b3d5b923a5d0b9e9577ea3da8b3b5e5295"
  },
  entryNameRuben: {
    "path": "/original-ui/gdi-text/images/entity-ruben-auto.png",
    "bytes": 262,
    "sha256": "3894023d68064c89a18b86d908a3e017619c73b1d632bfab382b2cca440cab5c"
  },
  entryAssetWorker: {
  "path": "/mir2-asset-worker.js",
  "sha256": "2e1191cab2b9ede2cf0b09c559c7e4da1e8b84574d4910589ad1f9a968ae7a51",
  "bytes": 47880
},
  assetRunId: '42494701555',
  baseline: "dpl_AWssgszkpKJznMksLsP7hiRFK6U8",
  relay: 'https://relay.dubhe.obelisk.build',
});
const QR_OPERATIONS = ['numeron-qr-stage', 'numeron-qr-check', 'numeron-qr-promote', 'numeron-qr-rollback'];
const WALLET_ORIGIN = 'https://wallet.dubhe.obelisk.build';
const QR_SESSION_PATH = '/api/wallet/dubhe/session';
const DEPLOYMENT = /^dpl_[A-Za-z0-9]+$/;
const SHA = /^[a-f0-9]{40}$/;
const ROOT = fileURLToPath(new URL('../../', import.meta.url));
const requireRelease = (condition, code) => { if (!condition) throw new ReleaseError(code); };
export class ReleaseError extends Error { constructor(code) { super(code); this.code = code; } }
export const production = project => project.targets?.production?.id ?? project.targets?.production?.deploymentId ?? null;
const digest = bytes => createHash('sha256').update(bytes).digest('hex');

export function validateIdentity(env) {
  const operation = env.RELEASE_OPERATION;
  const qr = QR_OPERATIONS.includes(operation);
  const sha = qr ? QR_RELEASE.source : env.RELEASE_SOURCE_SHA || env.GITHUB_SHA;
  requireRelease(qr || ['numeron-context', 'numeron-stage', 'numeron-promote'].includes(operation), 'operationInvalid');
  requireRelease(env.GITHUB_REPOSITORY === REPOSITORY && env.GITHUB_REF === `refs/heads/${BRANCH}`
    && SHA.test(env.GITHUB_SHA || '') && SHA.test(sha || ''), 'sourceIdentityInvalid');
  requireRelease(operation !== 'numeron-stage' || sha === env.GITHUB_SHA, 'stageSourceMustEqualCheckout');
  if (qr) {
    requireRelease(!env.RELEASE_SOURCE_SHA && !env.RELEASE_ASSET_RUN_ID && !env.RELEASE_MAIN_DEPLOYMENT_ID,
      'qrSourceInputsArePinned');
    const id = env.RELEASE_DEPLOYMENT_ID;
    requireRelease(operation === 'numeron-qr-stage' ? !id : DEPLOYMENT.test(id || '') && id !== QR_RELEASE.baseline,
      'qrCandidateInputInvalid');
  }
  requireRelease(Boolean(env.VERCEL_TOKEN) && env.VERCEL_ORG_ID === TEAM
    && env.VERCEL_PROJECT_ID === SITES.original.project, 'originalCiIdentityInvalid');
  return { operation, sha, qr };
}

export function validateProject(key, value) {
  const site = SITES[key];
  requireRelease(value?.id === site.project && value.accountId === TEAM && value.name === site.name, 'projectIdentityChanged');
}
export function guardProjects(projects, expected) {
  for (const key of Object.keys(SITES)) {
    validateProject(key, projects[key]);
    requireRelease(production(projects[key]) === expected[key], 'concurrentProductionChange');
  }
}
export function validateAssetConfig(config, contract = FIXED_R16) {
  requireRelease(config?.schemaVersion === 3 && config.version === contract.version
    && config.objectPrefix === contract.prefix && config.assetBaseUrl === `${contract.publicOrigin}/${contract.prefix}`
    && config.remoteReleaseUrl === `${contract.publicOrigin}/${contract.prefix}/remote-asset-release.json`, 'assetConfigNotFixedR16');
}
export function validateAssetReceipt(receipt, sha, contract = FIXED_R16) {
  requireRelease(receipt?.schemaVersion === 1 && receipt.kind === 'mir2-fixed-r16-publication-receipt'
    && receipt.repository === REPOSITORY && receipt.sourceSha === sha && receipt.sourceClean === true
    && receipt.materialSourceSha === contract.materialSourceSha && receipt.passed === true && receipt.dryRun === false
    && typeof receipt.manifestPublished === 'boolean' && receipt.manifestVerified === true && receipt.phase === 'complete'
    && receipt.verified === contract.fileCount && receipt.unknownWriteOutcomes === 0
    && typeof receipt.finishedAt === 'string' && Number.isFinite(Date.parse(receipt.finishedAt)), 'assetReceiptNotSuccessfulPublication');
  for (const field of ['uploaded', 'existingVerified', 'remoteWrites', 'putAttempts', 'publicGzipWireVerified', 'publicGzipDecodedVerified'])
    requireRelease(Number.isSafeInteger(receipt[field]) && receipt[field] >= 0, 'assetReceiptReadbackCountInvalid');
  requireRelease(receipt.uploaded + receipt.existingVerified === contract.fileCount
    && receipt.remoteWrites === receipt.uploaded + Number(receipt.manifestPublished)
    && receipt.putAttempts === receipt.remoteWrites
    && receipt.publicGzipWireVerified + receipt.publicGzipDecodedVerified === contract.gzipFiles, 'assetReceiptReadbackIncomplete');
  for (const field of ['bundleSha256', 'payloadSha256', 'manifestSha256'])
    requireRelease(receipt[field] === contract[field], 'assetReceiptPinMismatch');
  const review = receipt.review;
  for (const [field, expected] of Object.entries({ prefix: contract.prefix, files: contract.fileCount,
    bytes: contract.payloadBytes, wireBytes: contract.wireBytes, gzipFiles: contract.gzipFiles, fonts: contract.fontFiles,
    payloadSha256: contract.payloadSha256, manifestSha256: contract.manifestSha256, manifestLast: true,
    overwriteExisting: false, allMapsVerified: false, inheritedObjectsReverified: false }))
    requireRelease(review?.[field] === expected, 'assetReceiptReviewMismatch');
}
export function validateAssetRun(run, jobs, artifacts, runId, sha) {
  requireRelease(String(run?.id) === runId && run.repository?.full_name === REPOSITORY
    && run.path === '.github/workflows/web-assets-r2-release.yml' && run.head_sha === sha
    && run.status === 'completed' && run.conclusion === 'success' && run.event === 'workflow_dispatch', 'assetRunIdentityMismatch');
  const matches = jobs.filter(job => job.name === 'publish-fixed-r16-assets');
  requireRelease(matches.length === 1 && matches[0].status === 'completed' && matches[0].conclusion === 'success', 'assetPublicationJobNotSuccessful');
  const named = artifacts.filter(artifact => artifact.name === `fixed-r16-assets-${runId}`);
  requireRelease(named.length === 1 && named[0].expired === false && Number.isSafeInteger(named[0].id), 'assetReceiptArtifactInvalid');
  return named[0];
}
export function validatePublicManifest(bytes, contract = FIXED_R16) {
  requireRelease(bytes.length === contract.manifestBytes && digest(bytes) === contract.manifestSha256, 'publicAssetManifestPinMismatch');
  let manifest;
  try { manifest = JSON.parse(bytes.toString('utf8')); } catch { throw new ReleaseError('publicAssetManifestInvalid'); }
  requireRelease(manifest.schemaVersion === 1 && manifest.kind === 'mir2-remote-asset-release'
    && manifest.version === contract.version && manifest.objectPrefix === contract.prefix
    && manifest.candidateCoverage?.payloadSha256 === contract.payloadSha256
    && manifest.candidateCoverage?.localCandidateFilesVerified === contract.fileCount
    && manifest.bevyRuntime?.version === contract.runtimeVersion, 'publicAssetManifestIdentityMismatch');
}
export function publicEnvironment(key, sha, config) {
  return key === 'main' ? {
    NEXT_PUBLIC_SITE_URL: ORIGIN, NEXT_PUBLIC_GAME_URL: `${ORIGIN}/game`,
    // An explicit empty build/runtime value overrides the repository's .env.production.
    // The current public marketing baseline has no available Windows installer.
    NEXT_PUBLIC_WINDOWS_DOWNLOAD_URL: '',
    GAME_HEALTH_URL: 'https://165.154.65.136.sslip.io/playtest/health', EXPLORER_ORIGIN: 'https://numeron-explorer.vercel.app',
  } : {
    NEXT_PUBLIC_MIR2_GATEWAY_WS_URL: 'wss://165.154.65.136.sslip.io/playtest/ws',
    MIR2_GATEWAY_HTTP_URL: 'https://165.154.65.136.sslip.io/playtest', NEXT_PUBLIC_MIR2_QUEST_GUIDANCE: 'newcomer-v2',
    MIR2_RUNTIME_ENV: 'test', MIR2_MARKETING_ORIGIN: 'https://numeron-legend-of-rebirth.vercel.app',
    MIR2_PASSKEY_ALLOWED_ORIGINS: [ORIGIN, 'https://numeron-r16-web.vercel.app', 'https://numeron-r16-web-obelisk-labs.vercel.app'].join(','),
    MIR2_USE_PREBUILT_BEVY_RUNTIME: '1', MIR2_USE_PREBUILT_CLIENT_CORE: '1', MIR2_REUSE_ORIGINAL_ASSET_MANIFEST: '1',
    MIR2_PRODUCTION_ASSET_CONFIG: 'numeron-r16-web-assets.json',
    MIR2_DEPLOY_REVISION: sha, NEXT_PUBLIC_MIR2_ASSET_BASE_URL: config.assetBaseUrl, MIR2_ASSET_OBJECT_PREFIX: config.objectPrefix,
    MIR2_R2_PROXY_BASE: config.assetBaseUrl, MIR2_ASSET_BASE_URL: config.assetBaseUrl,
  };
}
export function deploymentBody(key, sha, repoId, config, secret, sourceRef = BRANCH) {
  const environment = publicEnvironment(key, sha, config);
  if (key === 'game') requireRelease(typeof secret === 'string' && secret.length >= 32, 'gatewayAuthSecretMissing');
  return { name: SITES[key].name, project: SITES[key].project, target: 'production', autoAssignCustomDomains: false,
    gitSource: { type: 'github', repoId, ref: sourceRef, sha },
    projectSettings: { rootDirectory: SITES[key].root, framework: 'nextjs', nodeVersion: '22.x',
      buildCommand: key === 'game' ? 'bash ./scripts/vercel-build.sh' : 'npm run build',
      installCommand: 'npm ci', outputDirectory: null, sourceFilesOutsideRootDirectory: true },
    env: { ...environment, ...(key === 'game' ? {
      MIR2_PASSKEY_AUTH_SECRET: secret, MIR2_DUBHE_CONNECT_RELAY_URL: QR_RELEASE.relay,
    } : {}) }, build: { env: environment },
    meta: { numeronR16SourceSha: sha, numeronR16Root: SITES[key].root, numeronR16AssetVersion: config.version,
      numeronR16OriginalBaseline: SITES.original.baseline, numeronR16MainBaseline: SITES.main.baseline,
      ...(key === 'main' ? { numeronR16WindowsDownloadPolicy: 'unavailable-baseline' } : {}) },
  };
}
export function candidateGuard(key, candidate, snapshot, sha, config, sourceRef = BRANCH) {
  const site = SITES[key];
  // Modern Git deployments may omit root from their REST snapshots. Every
  // explicit root must agree; missing roots still require our staged metaRoot,
  // exact Git source and the protected page proofs before any promotion.
  const roots = [candidate?.rootDirectory, candidate?.projectSettings?.rootDirectory,
    snapshot?.rootDirectory, snapshot?.projectSettings?.rootDirectory].filter(root => root != null);
  requireRelease(candidate?.projectId === site.project && candidate.target === 'production' && candidate.readyState === 'READY'
    && candidate.gitSource?.sha === sha && candidate.gitSource?.ref === sourceRef
    && candidate.meta?.numeronR16SourceSha === sha && candidate.meta?.numeronR16Root === site.root
    && candidate.meta?.numeronR16AssetVersion === config.version
    && candidate.meta?.numeronR16OriginalBaseline === SITES.original.baseline
    && candidate.meta?.numeronR16MainBaseline === SITES.main.baseline
    && snapshot?.projectId === site.project && roots.every(root => root === site.root), 'candidateIdentitySourceRootMismatch');
  requireRelease(/^[a-z0-9][a-z0-9-]*\.vercel\.app$/.test(candidate.url || ''), 'candidateHostnameInvalid');
}
function downloadMarkup(html) {
  requireRelease(typeof html === 'string', 'downloadPageHtmlInvalid');
  // Next's serialized data and script source do not establish rendered page identity.
  return html.replace(/<!--[\s\S]*?-->|<(script|style)\b[^>]*>[\s\S]*?<\/\1\s*>/gi, '');
}
function trackedDownloadAnchors(markup, track) {
  const marker = new RegExp(`\\sdata-track\\s*=\\s*(?:"${track}"|'${track}'|${track}(?=\\s|/?>))`, 'i');
  return [...markup.matchAll(/<a\b[^>]*>/gi)].map(match => match[0]).filter(tag => marker.test(tag));
}
export function validateDownloadPage(html) {
  const markup = downloadMarkup(html);
  const page = [...markup.matchAll(/<main\b[^>]*>/gi)].some(match => {
    const classes = /\sclass\s*=\s*(["'])(.*?)\1/i.exec(match[0]);
    return classes?.[2].split(/\s+/).includes('download-page');
  });
  requireRelease(/<html\b/i.test(markup) && page
    && ['download_web', 'navigation_promotions', 'footer_promotions'].every(track => trackedDownloadAnchors(markup, track).length > 0), 'downloadPageHtmlInvalid');
}
export function windowsDownloadLinks(html, { allowEmpty = false } = {}) {
  // Empty is a page-specific policy, never the default for arbitrary missing markup.
  if (allowEmpty === true) validateDownloadPage(html);
  const links = trackedDownloadAnchors(downloadMarkup(html), 'download_windows').map(tag => {
    if ([...tag.matchAll(/\shref\s*=/gi)].length !== 1) return undefined;
    return /\shref\s*=\s*(["'])(.*?)\1/i.exec(tag)?.[2]?.replaceAll('&amp;', '&');
  });
  requireRelease((links.length > 0 || allowEmpty === true) && links.every(link => {
    if (typeof link !== 'string' || !link || /[\u0000-\u0020\u007f<>"'\\]|&#|&[A-Za-z]+;/.test(link)) return false;
    try { const url = new URL(link); return url.protocol === 'https:' && !url.username && !url.password
      && !/^https:\/\/[^/?#]*@/i.test(link); } catch { return false; }
  }), 'windowsDownloadLinkMissing');
  return [...new Set(links)].sort();
}
export function verifyPages(read, candidates, baselineMain, sha) {
  const marketing = read(candidates.main, '/zh-CN');
  requireRelease(marketing.includes('/marketing-assets/_next/static/') && marketing.includes('navigation_promotions')
    && marketing.includes('footer_promotions'), 'marketingHtmlInvalid');
  const download = read(candidates.main, '/zh-CN/download');
  const baselineDownload = read(baselineMain, '/zh-CN/download');
  validateDownloadPage(download); validateDownloadPage(baselineDownload);
  requireRelease(download.includes(`${ORIGIN}/game`), 'marketingGameLinkInvalid');
  requireRelease(JSON.stringify(windowsDownloadLinks(download, { allowEmpty: true }))
    === JSON.stringify(windowsDownloadLinks(baselineDownload, { allowEmpty: true })), 'windowsDownloadsChanged');
  requireRelease(read(candidates.main, '/en').includes('lang="en"'), 'marketingEnglishRouteInvalid');
  verifyGamePages(read, candidates.game, sha);
}

function verifyGamePages(read, candidate, sha) {
  let version, manifest;
  try { version = JSON.parse(read(candidate, '/version')); manifest = JSON.parse(read(candidate, '/manifest.webmanifest')); }
  catch { throw new ReleaseError('candidateVersionOrManifestInvalid'); }
  requireRelease(version.revision === sha && manifest.start_url === '/game' && manifest.scope === '/', 'candidateVersionOrPwaMismatch');
  const game = read(candidate, '/game');
  requireRelease(game.includes('/_next/static/') && !game.includes('navigation_promotions'), 'gameHtmlInvalid');
}

function verifyEntryAsset(bytes, entry, code) {
  requireRelease(bytes.length === entry.bytes && digest(bytes) === entry.sha256, code);
  return entry;
}

async function proveQrRelay(read) {
  let created, approval;
  try {
    created = JSON.parse(await read(QR_SESSION_PATH));
    approval = new URL(created.approvalUrl);
  } catch { throw new ReleaseError('qrCreateFailed'); }
  const lifetime = created.expiresAt - Date.now();
  requireRelease(typeof created.intentToken === 'string' && /^[A-Za-z0-9_.-]{1,12000}$/.test(created.intentToken)
    && /^[A-Za-z0-9_-]{1,256}$/.test(created.sessionId || '') && Number.isSafeInteger(created.expiresAt)
    && lifetime > 0 && lifetime <= 125_000 && approval.origin === WALLET_ORIGIN
    && approval.pathname === '/connect/approve' && !approval.username && !approval.password && !approval.hash
    && [...approval.searchParams.keys()].sort().join(',') === 'relay,session'
    && approval.searchParams.get('relay') === QR_RELEASE.relay
    && approval.searchParams.get('session') === created.sessionId, 'qrSessionBindingMismatch');
  let pending;
  try { pending = JSON.parse(await read(`${QR_SESSION_PATH}/status`, created.intentToken)); }
  catch { throw new ReleaseError('qrPollFailed'); }
  requireRelease(pending?.status === 'pending' && pending.login === undefined, 'qrPollNotPending');
  return { relay: approval.searchParams.get('relay'), wallet: approval.origin, pending: true };
}

async function boundedBytes(response, maximum) {
  requireRelease(response.ok, 'publicManifestUnavailable');
  const chunks = []; let size = 0;
  for await (const chunk of response.body) { size += chunk.length; requireRelease(size <= maximum, 'responseTooLarge'); chunks.push(chunk); }
  return Buffer.concat(chunks);
}
async function downloadReceipt(runId, name, env) {
  const directory = await mkdtemp(path.join(env.RUNNER_TEMP || tmpdir(), 'numeron-r16-asset-receipt-'));
  try { execFileSync('gh', ['run', 'download', runId, '--repo', REPOSITORY, '--name', name, '--dir', directory], {
    env: childEnvironment(env, 'github'), encoding: 'utf8', timeout: 120000, maxBuffer: 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'],
  }); } catch { throw new ReleaseError('assetReceiptDownloadFailed'); }
  requireRelease(JSON.stringify(await readdir(directory)) === JSON.stringify(['receipt.json']), 'assetReceiptMembersInvalid');
  const filename = path.join(directory, 'receipt.json'), stat = await lstat(filename);
  requireRelease(stat.isFile() && !stat.isSymbolicLink() && stat.nlink === 1 && stat.size <= 1024 * 1024, 'assetReceiptFileInvalid');
  try { return JSON.parse(await readFile(filename, 'utf8')); } catch { throw new ReleaseError('assetReceiptJsonInvalid'); }
}
export function childEnvironment(env, purpose) {
  const keys = ['PATH', 'HOME', 'USERPROFILE', 'TMPDIR', 'TEMP', 'TMP', 'XDG_CACHE_HOME', 'CI'];
  keys.push(...(purpose === 'github' ? ['GH_TOKEN'] : ['VERCEL_TOKEN', 'VERCEL_ORG_ID', 'VERCEL_PROJECT_ID']));
  return Object.fromEntries(keys.filter(key => env[key] !== undefined).map(key => [key, env[key]]));
}
function protectedResponse(candidate, pathname, env, qrRequest) {
  requireRelease(/^[a-z0-9][a-z0-9-]*\.vercel\.app$/.test(candidate.url || '') && /^\/(?:[A-Za-z0-9_./-])*$/.test(pathname), 'candidateRequestInvalid');
  const requestArgs = qrRequest
    ? ['--request', 'POST', '--header', `Origin: ${ORIGIN}`,
      ...(qrRequest.token ? ['--header', `Authorization: Bearer ${qrRequest.token}`] : [])]
    : ['--location', '--max-redirs', '3'];
  try { return execFileSync('npx', ['--yes', 'vercel@56.4.1', 'curl', `https://${candidate.url}${pathname}`, '--',
    '--fail', '--silent', '--show-error', '--max-time', '30', ...requestArgs], {
    env: childEnvironment(env, 'vercel'), encoding: pathname.endsWith('.png') ? undefined : 'utf8',
    timeout: 180000, maxBuffer: 8 * 1024 * 1024, stdio: ['ignore', 'pipe', 'pipe'],
  }); } catch { throw new ReleaseError('protectedCandidateFetchFailed'); }
}

async function releaseQrGame({ operation, initial, projects, candidate, id, vercel,
  body, readCandidate, fetchImpl, sleep, receipt }) {
  const hostname = new URL(ORIGIN).hostname;
  const { baseline, relay, entryScene, entryFloor, entryPlayerMale, entryPlayerFemale, entryNpcs,
    entryObjects1, entryObjects2, entryObjects3,
    entryMonsterStanding, entryMonsterWalking, entryGameplayLocale,
    entryNameAssistantJane, entryNameMerchant, entryNameRuben, entryAssetWorker } = QR_RELEASE;
  const entryAssets = Object.entries({
    Scene: entryScene, Floor: entryFloor, PlayerMale: entryPlayerMale, PlayerFemale: entryPlayerFemale, Npcs: entryNpcs,
    Objects1: entryObjects1, Objects2: entryObjects2, Objects3: entryObjects3,
    MonsterStanding: entryMonsterStanding, MonsterWalking: entryMonsterWalking, GameplayLocale: entryGameplayLocale,
    NameAssistantJane: entryNameAssistantJane, NameMerchant: entryNameMerchant, NameRuben: entryNameRuben,
    AssetWorker: entryAssetWorker,
  });
  const expectedOther = { original: production(initial.original), main: production(initial.main) };
  async function mapping(allowed) {
    const live = await projects(), game = production(live.game);
    guardProjects(live, { ...expectedOther, game });
    const alias = await vercel(`/v4/aliases/${hostname}`);
    requireRelease(allowed.includes(game) && allowed.includes(alias.deploymentId)
      && alias.alias === hostname && alias.projectId === SITES.game.project
      && alias.redirect == null && alias.microfrontends == null && alias.deletedAt == null, 'qrTrafficMappingChanged');
    return { target: game, alias: alias.deploymentId };
  }
  async function awaitMapping(from, to) {
    for (let attempt = 0; attempt < 30; attempt++) {
      const current = await mapping([from, to]);
      if (current.target === to && current.alias === to) return current;
      await sleep(2000);
    }
    throw new ReleaseError('qrTrafficDidNotConverge');
  }
  const gamePath = SITES.game.project;
  if (operation === 'numeron-qr-stage') {
    receipt.trafficBefore = await mapping([baseline]);
    body.meta.numeronR16ConnectRelay = relay;
    receipt.writes.push({ operation: 'stage', site: 'game' });
    let value;
    try { value = await vercel('/v13/deployments', 'POST', body); }
    finally { receipt.trafficAfter = await mapping([baseline]); }
    requireRelease(DEPLOYMENT.test(value.id || '') && value.id !== baseline && value.projectId === gamePath,
      'stagedDeploymentIdentityInvalid');
    receipt.candidates = { game: { id: value.id, url: value.url, project: value.projectId, readyState: value.readyState } };
    return;
  }
  const reviewed = await candidate('game', id);
  requireRelease(reviewed.meta?.numeronR16ConnectRelay === relay, 'qrCandidateRelayMismatch');
  receipt.candidates = { game: { id: reviewed.id, url: reviewed.url } };
  if (operation === 'numeron-qr-rollback') {
    receipt.trafficBefore = await mapping([reviewed.id]);
    receipt.writes.push({ operation: 'rollback', site: 'game', deployment: baseline });
    await vercel(`/v1/projects/${gamePath}/rollback/${baseline}`, 'POST', {});
    receipt.trafficAfter = await awaitMapping(reviewed.id, baseline);
    return;
  }
  receipt.trafficBefore = await mapping([baseline]);
  verifyGamePages(readCandidate, reviewed, QR_RELEASE.source);
  for (const [name, entry] of entryAssets) {
    const bytes = Buffer.from(readCandidate(reviewed, entry.path), 'utf8');
    receipt['protectedEntry' + name] = verifyEntryAsset(bytes, entry, 'entry' + name + 'DigestMismatch');
  }
  receipt.protectedQr = await proveQrRelay((pathname, token) => readCandidate(reviewed, pathname, { token }));
  receipt.trafficAfter = await mapping([baseline]);
  if (operation === 'numeron-qr-check') {
    receipt.candidateChecksPassed = true;
    return;
  }
  receipt.writes.push({ operation: 'promote', site: 'game', deployment: reviewed.id });
  try {
    await vercel(`/v10/projects/${gamePath}/promote/${reviewed.id}`, 'POST', {});
    receipt.trafficAfter = await awaitMapping(baseline, reviewed.id);
    receipt.publicQr = await proveQrRelay(async (pathname, token) => {
      const response = await fetchImpl(`${ORIGIN}${pathname}`, {
        method: 'POST', redirect: 'error', signal: AbortSignal.timeout(15000),
        headers: { Origin: ORIGIN, ...(token ? { Authorization: `Bearer ${token}` } : {}) },
      });
      return (await boundedBytes(response, 32_768)).toString('utf8');
    });
    const checkedAssets = await Promise.all(entryAssets.map(async ([name, entry]) => {
      const response = await fetchImpl(ORIGIN + entry.path, {
        redirect: 'error', signal: AbortSignal.timeout(30_000),
      });
      requireRelease(response.ok, 'entry' + name + 'Unavailable');
      const bytes = await boundedBytes(response, entry.bytes);
      return [name, verifyEntryAsset(bytes, entry, 'entry' + name + 'DigestMismatch')];
    }));
    for (const [name, entry] of checkedAssets) receipt['publicEntry' + name] = entry;
    receipt.trafficAfter = await mapping([reviewed.id]);
  } catch (error) {
    const current = await mapping([baseline, reviewed.id]);
    if (current.target === reviewed.id) {
      await vercel(`/v1/projects/${gamePath}/rollback/${baseline}`, 'POST', {});
      receipt.trafficAfter = await awaitMapping(reviewed.id, baseline);
      receipt.rollbacks.push({ site: 'game', deployment: baseline });
    }
    throw error;
  }
}

export async function runNumeronRelease({ env = process.env, fetchImpl = fetch,
  readConfig = async () => JSON.parse(await readFile(path.join(ROOT, 'mir2-web3/config/numeron-r16-web-assets.json'), 'utf8')),
  readReceipt = downloadReceipt, readCandidate = (candidate, pathname, request) => protectedResponse(candidate, pathname, env, request),
  sleep = ms => new Promise(resolve => setTimeout(resolve, ms)),
  contract = FIXED_R16, onReceipt = async () => {} } = {}) {
  const receipt = { schema: 1, kind: 'numeron-r16-release', passed: false, domainMovePending: true,
    customDomainChanged: false, oldGameProjectMutatedByHelper: false, writes: [], rollbacks: [], credentialsRecorded: false };
  try {
    const { operation, sha, qr } = validateIdentity(env);
    Object.assign(receipt, { operation, sourceSha: sha, implementationSha: env.GITHUB_SHA });
    async function api(host, pathname, method = 'GET', body) {
      const token = host === 'api.vercel.com' ? env.VERCEL_TOKEN : env.GH_TOKEN;
      requireRelease(Boolean(token), 'apiCredentialMissing');
      let response;
      try { response = await fetchImpl(`https://${host}${pathname}`, { method, headers: {
        Authorization: `Bearer ${token}`, 'Content-Type': 'application/json', Accept: 'application/json',
      }, ...(body === undefined ? {} : { body: JSON.stringify(body) }), signal: AbortSignal.timeout(30000) }); }
      catch { throw new ReleaseError('apiTransportFailed'); }
      requireRelease(response.ok, `apiHttp${response.status}`);
      try { const text = await response.text(); return text ? JSON.parse(text) : {}; } catch { throw new ReleaseError('apiJsonInvalid'); }
    }
    const vercel = (pathname, method, body) => api('api.vercel.com', `${pathname}${pathname.includes('?') ? '&' : '?'}teamId=${TEAM}`, method, body);
    async function projects() {
      const result = {};
      for (const [key, site] of Object.entries(SITES)) { result[key] = await vercel(`/v9/projects/${site.project}`); validateProject(key, result[key]); }
      return result;
    }
    const initial = await projects(), config = await readConfig();
    receipt.projects = Object.fromEntries(Object.entries(initial).map(([key, value]) => [key, {
      project: value.id, production: production(value),
      rootDirectory: typeof value.rootDirectory === 'string' ? value.rootDirectory : null,
      nodeVersion: typeof value.nodeVersion === 'string' ? value.nodeVersion : null,
    }]));
    if (operation === 'numeron-context') { receipt.assetConfigMatches = config.version === contract.version && config.objectPrefix === contract.prefix; receipt.passed = true; return receipt; }
    validateAssetConfig(config, contract);
    const runId = qr ? QR_RELEASE.assetRunId : env.RELEASE_ASSET_RUN_ID;
    requireRelease(/^\d+$/.test(runId || ''), 'assetRunIdRequired');
    const github = pathname => api('api.github.com', `/repos/${REPOSITORY}${pathname}`);
    const assetSha = qr ? QR_RELEASE.assetSource : sha;
    const sourceRef = qr ? QR_RELEASE.sourceRef : BRANCH;
    if (qr) {
      const { baseSource, changedFiles, sourceCommits } = QR_RELEASE;
      const comparison = await github(`/compare/${baseSource}...${sha}`);
      const files = Array.isArray(comparison.files) ? comparison.files : [];
      const byPath = new Map(files.map(file => [file?.filename, file]));
      const commits = Array.isArray(comparison.commits) ? comparison.commits : [];
      const expectedCommits = new Set(sourceCommits);
      const actualCommits = new Set(commits.map(commit => commit?.sha));
      requireRelease(expectedCommits.size === sourceCommits.length && expectedCommits.has(sha)
        && comparison.status === 'ahead' && comparison.ahead_by === sourceCommits.length && comparison.behind_by === 0
        && comparison.merge_base_commit?.sha === baseSource && commits.length === sourceCommits.length
        && actualCommits.size === expectedCommits.size && [...actualCommits].every(commit => expectedCommits.has(commit))
        && files.length === changedFiles.length && byPath.size === files.length
        && changedFiles.every(expected => {
          const actual = byPath.get(expected.filename);
          return actual?.status === expected.status && actual.sha === expected.sha;
        }),
      'qrSourceClosureMismatch');
      receipt.sourceClosure = { base: baseSource, assetSource: assetSha, ref: sourceRef, commits: sourceCommits, changedFiles };
    }
    const run = await github(`/actions/runs/${runId}`), jobs = await github(`/actions/runs/${runId}/jobs?per_page=100`),
      artifacts = await github(`/actions/runs/${runId}/artifacts?per_page=100`);
    requireRelease(!jobs.total_count || jobs.total_count <= 100, 'assetJobsPaginationExceeded');
    requireRelease(!artifacts.total_count || artifacts.total_count <= 100, 'assetArtifactsPaginationExceeded');
    const artifact = validateAssetRun(run, jobs.jobs || [], artifacts.artifacts || [], runId, assetSha);
    validateAssetReceipt(await readReceipt(runId, artifact.name, env), assetSha, contract);
    const publicResponse = await fetchImpl(config.remoteReleaseUrl, { redirect: 'error', signal: AbortSignal.timeout(60000) });
    validatePublicManifest(await boundedBytes(publicResponse, contract.manifestBytes), contract);
    receipt.assetPublication = { runId, artifactId: artifact.id, manifestSha256: contract.manifestSha256, manifestBytes: contract.manifestBytes };
    const link = initial.original.link;
    requireRelease(link?.type === 'github' && link.repo === 'mir2' && /^\d+$/.test(String(link.repoId || ''))
      && String(link.org || '').toLowerCase() === 'zombieliu', 'originalGitRepositoryMismatch');
    const ids = { main: env.RELEASE_MAIN_DEPLOYMENT_ID, game: env.RELEASE_DEPLOYMENT_ID };
    async function candidate(key, id) {
      requireRelease(DEPLOYMENT.test(id || ''), 'exactCandidateIdRequired');
      const value = await vercel(`/v13/deployments/${id}?withGitRepoInfo=true`);
      requireRelease(value.id === id, 'candidateIdMismatch');
      let snapshot, until;
      for (let page = 0; page < 5 && !snapshot; page++) {
        const list = await vercel(`/v7/deployments?projectId=${SITES[key].project}&sha=${sha}&target=production&limit=100${until ? `&until=${encodeURIComponent(until)}` : ''}`);
        snapshot = list.deployments?.find(item => item.uid === id); until = list.pagination?.next; if (!until) break;
      }
      candidateGuard(key, value, snapshot, sha, config, sourceRef);
      return value;
    }
    if (qr) {
      const body = operation === 'numeron-qr-stage'
        ? deploymentBody('game', sha, link.repoId, config, env.MIR2_R16_PASSKEY_AUTH_SECRET, sourceRef)
        : undefined;
      await releaseQrGame({ operation, initial, projects, candidate, id: ids.game, vercel,
        body, readCandidate, fetchImpl, sleep, receipt });
      receipt.passed = true;
      return receipt;
    }
    const expected = { original: SITES.original.baseline, main: SITES.main.baseline, game: null };
    if (production(initial.game) !== null) { const own = await candidate('game', ids.game); requireRelease(production(initial.game) === own.id, 'newAppProductionNotOwnedCandidate'); expected.game = own.id; }
    if (operation === 'numeron-stage') {
      guardProjects(initial, expected);
      const staged = {};
      // Validate secret before either candidate is created.
      deploymentBody('game', sha, link.repoId, config, env.MIR2_R16_PASSKEY_AUTH_SECRET);
      for (const key of ['main', 'game']) {
        guardProjects(await projects(), expected);
        receipt.writes.push({ operation: 'stage', site: key, responseKnown: false });
        let value;
        try { value = await vercel('/v13/deployments', 'POST', deploymentBody(key, sha, link.repoId, config, env.MIR2_R16_PASSKEY_AUTH_SECRET)); }
        finally { guardProjects(await projects(), expected); }
        requireRelease(DEPLOYMENT.test(value.id || '') && value.projectId === SITES[key].project, 'stagedDeploymentIdentityInvalid');
        staged[key] = { id: value.id, url: value.url, project: value.projectId, readyState: value.readyState };
        Object.assign(receipt.writes.at(-1), { responseKnown: true, deployment: value.id });
        guardProjects(await projects(), expected);
      }
      receipt.candidates = staged;
    } else {
      const reviewed = { main: await candidate('main', ids.main), game: await candidate('game', ids.game) };
      if (production(initial.main) === reviewed.main.id) expected.main = reviewed.main.id;
      guardProjects(initial, expected);
      const baselineMain = await vercel(`/v13/deployments/${SITES.main.baseline}`);
      requireRelease(baselineMain.id === SITES.main.baseline && baselineMain.projectId === SITES.main.project, 'marketingBaselineIdentityMismatch');
      verifyPages(readCandidate, reviewed, baselineMain, sha);
      receipt.candidates = Object.fromEntries(Object.entries(reviewed).map(([key, value]) => [key, { id: value.id, url: value.url, root: SITES[key].root }]));
      const touched = [];
      try {
        for (const key of ['main', 'game']) {
          guardProjects(await projects(), expected);
          if (expected[key] === reviewed[key].id) continue;
          touched.push({ key, baseline: expected[key] });
          receipt.writes.push({ operation: 'promote', site: key, deployment: reviewed[key].id });
          await vercel(`/v10/projects/${SITES[key].project}/promote/${reviewed[key].id}`, 'POST', {});
          const before = expected[key]; expected[key] = reviewed[key].id;
          for (let attempt = 0; ; attempt++) {
            const current = await projects();
            requireRelease([before, reviewed[key].id].includes(production(current[key])), 'concurrentProductionChange');
            const allowed = { ...expected, [key]: production(current[key]) }; guardProjects(current, allowed);
            if (production(current[key]) === reviewed[key].id) break;
            requireRelease(attempt < 29, 'promotionDidNotConverge'); await sleep(2000);
          }
          guardProjects(await projects(), expected);
        }
      } catch (error) {
        for (const { key, baseline } of touched.reverse()) {
          const current = await projects();
          if (production(current[key]) !== reviewed[key].id) { receipt.rollbacks.push({ site: key, skipped: 'mappingNoLongerOwned' }); continue; }
          if (!baseline) { receipt.rollbacks.push({ site: key, skipped: 'noPriorDeploymentManualReconcile' }); continue; }
          // Recheck all untouched/immutable identities; never roll back a third-party version.
          requireRelease(production(current.original) === SITES.original.baseline, 'originalProductionChangedDuringRollback');
          await vercel(`/v1/projects/${SITES[key].project}/rollback/${baseline}`, 'POST', {});
          const after = await projects(); requireRelease(production(after[key]) === baseline, 'rollbackDidNotConverge');
          receipt.rollbacks.push({ site: key, deployment: baseline });
        }
        throw error;
      }
    }
    receipt.passed = true;
    return receipt;
  } catch (error) { receipt.failureCode = error instanceof ReleaseError ? error.code : 'unexpectedReleaseFailure'; throw Object.assign(error, { releaseReceipt: receipt }); }
  finally { await onReceipt(receipt); }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runNumeronRelease({ onReceipt: receipt => writeFile('numeron-r16-release.json', `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx', mode: 0o600 }) })
    .then(receipt => console.log(JSON.stringify({ operation: receipt.operation, passed: receipt.passed, domainMovePending: true })))
    .catch(error => { console.error(error instanceof ReleaseError ? error.code : 'unexpectedReleaseFailure'); process.exitCode = 1; });
}
