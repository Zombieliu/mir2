import { verifyNumeronSourceClosure } from './numeron-source-closure.mjs';
import SOURCE_CHECKPOINTS from './numeron-r16-source-checkpoints.json' with { type: 'json' };
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
  source: "ef6d20c095a20460908ae9fab311ae39aaa55f6e",
  sourceRef: BRANCH,
  sourceCheckpoints: SOURCE_CHECKPOINTS,
  baseSource: "20de5ea4fd93b3c61e4aa590dca2ea2d8f5d2339",
  assetSource: "f2de762309768b51b39438c2437c14e819df9bf7",
  sourceCommits: [
  "f51306c5f1527a025b1a7a23dc2e0e624e05aa0d",
  "b784b2f22d06d273dab1784d811d6c882b4fdddb",
  "810f181fac18597f220235fea66b7e4b02c8846f",
  "9aa2b0c4acb2a8b38b83023a678eb78626817b92",
  "9d06953d7bbb94d960686bed51048d0628996d7e",
  "05d233ed248e8b78128580ae71e7618f4338ab5c",
  "94497a6094a3ac29c9366c26f325df19a3b0a19e",
  "146bdf257665205a672db3fed5c984d63c7ea0ca",
  "47ef2650392808f5bfce05d197cdb8eacc719cea",
  "ceb1c1132f9677ca18f7467cba5278cdca1ec9c0",
  "4aead752f62143199344bd7584c5b9120b6d30ba",
  "5d7bf8c2ea58d2ac68bbca5f18c8240fdc929d45",
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
  "aa524edc76d85e01f2ae8963957bbacd9728eac2",
  "39832d328e40406257942f1b9385a823effd8c07",
  "e157c2d132995132497c7bc2750ad96283bae794",
  "213fc1737ffc6da3b84de98eed2f92b59c30a51d",
  "32eb62774dc8ac1a49876808f6118948f137d5b6",
  "d3aefa26d9b9a2d3aa4c12c6aebee94372cea9a6",
  "d35e7327aa396bd0e5f15d6df4567107e9269882",
  "f28dc9453856955fce19e005647d9612a2930f7c",
  "b80759fe3b6a1e636268ae8b2c40f17ff4410de9",
  "362ea11c2498ffb7dda28d38299ed4aa6ca6c425",
  "1b5a9d1b4ccd06a007103a393cb7a670fe5fa422",
  "1149616b3cd7e071599c3674aa9644cdfc0bd34b",
  "9eac66ed3290ed5c4e60eb8bea07cf88619a8847",
  "4ad940554d3588bde9e4aceac282a923265dacfe",
  "99b959cef4286cc0c11bfa9f7a892bac061344ee",
  "b1c7da999877507f147fbe19c448507a021f4d2c",
  "b33533f4aa4c3fa00d9f35f2ad9d4942dd638045",
  "b3bff60c1d91941fcb39e7ebe1ff2a5ed8e92eba",
  "766d81d4dc786fcc48275ae020b5550a18e9601b",
  "88002a0000943f221f494cc61b8bb28486c923db",
  "0bcd61e0f27147f005dee3d6c49abb6a1af15157",
  "353a94bff10e57a6dbd2177047930e988c4fde40",
  "44b986b8a51682f1c72c552b82c0f21c32ce0d2a",
  "4204e920edde1d2c4a27e7bb7a2bba50ebd00860",
  "d809add41e167be5092de06b238d6632a3396a49",
  "627b68ae4dd5575a40ea37df16603087ed68a522",
  "276df66ab3b02cf7ebcc5555836647c6d66ef39b",
  "20e79131a23a4c22dbadf118dc514f33bdf85a80",
  "b299be39fbace1d57fcb2d29efcffad23e4d0f57",
  "bf0ed0ce019a587b3bd50554708b62682bbde8f6",
  "ef6d20c095a20460908ae9fab311ae39aaa55f6e"
],
  changedFiles: [
  {
    "filename": ".github/scripts/numeron-r16-release.mjs",
    "sha": "7359114987cd765c6388842c9e4700de54ee1d7a",
    "status": "modified"
  },
  {
    "filename": ".github/workflows/promotion-website-release.yml",
    "sha": "62db49ff56402b38451e58a115496c175a9c28b6",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/gateway/Cargo.toml",
    "sha": "725948d0afd19095ca32467587bc6924c3176930",
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
    "filename": "mir2-web3/apps/gateway/src/web.rs",
    "sha": "6dd09c63fe031e2ed388691fe8339f7b3c7e2c91",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/gateway/src/web/catalog_transport.rs",
    "sha": "e94dd375f60bce743b8a394aa8db3111750e4a34",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/gateway/src/web_activities_tests.rs",
    "sha": "a691f7157de6681e6a565a7d50c0712756d7fff1",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/gateway/src/web_background_execution.rs",
    "sha": "77737f16776e7c8a61cf1cbcaefa033e0551955b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/gateway/src/web_billing_tests.rs",
    "sha": "2d2079702b1cbfc881099dfc35a133cac5e348c6",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/gateway/src/web_game_shop_gift_tests.rs",
    "sha": "e10f1331903a0def3ae67cb0de0b21373122880c",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/gateway/src/web_mining_tests.rs",
    "sha": "0c1cf44dd0d1aa9627cf603ad036c47f4b275484",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/gateway/src/web_monthly_card_tests.rs",
    "sha": "43ddde7b40c500447d8bb475aa802d352036e59a",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/gateway/src/web_native_resume_handoff_ws_tests.rs",
    "sha": "eb80b14eeb330865d4f34ff52b549dd014e53699",
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
    "filename": "mir2-web3/apps/web/app/components/asset-cache-registrar.tsx",
    "sha": "7f193189fb109915afea18940229a621cfe0fe2e",
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
    "sha": "d08abe72b671a3eda9cd7e3bd55593d934a3eed9",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/lib/asset-cache-progress.ts",
    "sha": "e48afa923d8824d156802708bbfec65990302802",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/lib/bevy-runtime-font-prewarm.ts",
    "sha": "5be9d633b6d764f40550990ded49795cddacf6f8",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/lib/generated/r16-entry-bootstrap-chunks.ts",
    "sha": "2e7d97a19ccdb255ef1d88b2115085149546f73b",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/lib/generated/r16-entry-walking-parts.json",
    "sha": "38d4b23fe592da18d4f12e780578a63a234b6cc9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/lib/pinned-bootstrap-chunk.ts",
    "sha": "74e3b974a04e517207e8a14cda6e2847957831c5",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/lib/pinned-entry-actor-parts.ts",
    "sha": "92608828de457c5eb9b2564ca486a9c45c6d9f9e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/lib/r16-browser-localization.ts",
    "sha": "5bd6342e5ecd317df876082fe0cae2df42a552a7",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/lib/r16-entry-scene.ts",
    "sha": "10437197b77d9d2274d5784003733540a33a6b8a",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/lib/use-bevy-quest-ui.ts",
    "sha": "99c775595cce01ef634d2228b1c28bbff2ee4354",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/package.json",
    "sha": "dcb2148d7b513f6091d8184e0d4b05d7be6b66fc",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/public/bootstrap/r16-visual-parts/41fcaae2757ad0351da2e3840bf54008952b5c2714441530bb64c21f5c9fd64f/1f8307486897ae7be3af2e0f64f95dc4d2e21222c6e99ed1a1ae919300c2daa9.json",
    "sha": "d84d7f0aee65e78120fe912232c74bf6e0eda3e4",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/public/bootstrap/r16-visual-parts/41fcaae2757ad0351da2e3840bf54008952b5c2714441530bb64c21f5c9fd64f/65db6b2b5abdfa41a08c81f8ee577eb306aafc21c2b14b0c18281d13a9daec97.json",
    "sha": "b7cf6f6e4a9ce27842e723a89965adaac3870542",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/public/bootstrap/r16-visual-parts/41fcaae2757ad0351da2e3840bf54008952b5c2714441530bb64c21f5c9fd64f/774e7639568a7cf305cf62ea0273a2d0820abb6f5a696afdd8c349f6eb8a662d.json",
    "sha": "a86de05b4b53dbcf0447886ae748cc704880df2d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/public/bootstrap/r16-visual-parts/41fcaae2757ad0351da2e3840bf54008952b5c2714441530bb64c21f5c9fd64f/add27ee1a13b42f367f5dddde27581712bcb85b8cd670a9f8765eec43d248dfa.json",
    "sha": "3efafba4836671ff574f46a7171921d46f2c29ad",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/public/bootstrap/r16-visual-parts/41fcaae2757ad0351da2e3840bf54008952b5c2714441530bb64c21f5c9fd64f/d47c92dfcbaa0e7bb9d2790c2f8204cd75c7e5fd5673cc08628d1a90d45d8f4b.json",
    "sha": "5c1db3b61ec56dc3b3678df07b25d9d2ff2865e2",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/public/mir2-asset-worker.js",
    "sha": "a86aa7031698190197527d7d9f1f1234c7070ac1",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/generate-font-range-pins.mjs",
    "sha": "09490fc3e2ee96ebf14f479df6f52ee1beed636f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/generate-r16-bootstrap-chunks.mjs",
    "sha": "050dc8c395e57925fa80bd0628249ea00fea39ce",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/lib/static-cache-audit.mjs",
    "sha": "049d76512da9bd9ee54a915fe2287fa2550fb8e8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/lib/verify-r16-walking-chunks.mjs",
    "sha": "646f7e3950b6abd7d616191fcd3e8795c7675d13",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/smoke-asset-font-ranges.mjs",
    "sha": "d2c66b2a83a4cb364110ea00040d2b104966a885",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/smoke-asset-hud-admission.mjs",
    "sha": "30047328048f797b024cc2cff0e56ca944a67321",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/smoke-asset-hud-order.mjs",
    "sha": "33108c5f3e50927b58f31063b4cc1e8ff4dc1caf",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/smoke-asset-static-admission.mjs",
    "sha": "7f88a67bff8ab88dc0c87e9b2d6c9f27c9f8e8a2",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-asset-cache-progress.mjs",
    "sha": "5e41810965465e6da5059e0f91b04a55618ed064",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-asset-prewarm-request.mjs",
    "sha": "1535c2ea03aae60ee0d8a051738630cb6be15a25",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-asset-worker-font-ranges.mjs",
    "sha": "eb1d80184e162e310c7b466d78119819cf0e3c2d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-asset-worker-lifecycle.mjs",
    "sha": "79bb16af3e330e643d883dd93a2800ad60f6f24c",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-asset-worker-static-admission.mjs",
    "sha": "f099b69d357944926f5ecf620b950115f4ee592e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-bevy-quest-ui.mjs",
    "sha": "53797b30d7a5eb0c2faf7449e5633624a8b81d94",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-bevy-runtime-font-prewarm.mjs",
    "sha": "b435bdf60c7009e7470b9d942ba21bc9e01ea167",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-login-first-visible-edit.mjs",
    "sha": "ef83455f6b089277753bc18958e512fd864b2274",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-map-route-feedback.mjs",
    "sha": "a05e933dccfb5babf15db0ce692059de15cd08dd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-pinned-bootstrap-chunk.mjs",
    "sha": "d9c240fddc2467cf9cb72da75adc02ef2b51244d",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-pinned-entry-actor-parts.mjs",
    "sha": "436d2e109c93ac6a41bfc0f9f332d41870bec1a9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-r16-browser-localization.mjs",
    "sha": "a2105ce527a634d3a9cc8a7c35b15038694821bc",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-r16-quest-close-shortcut.mjs",
    "sha": "9f72cbba74c8e9cdb74f34169f9a7d5309da3b09",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-r16-quest-navigation.mjs",
    "sha": "b0df0e200c95d66814d046bd9f14d0f5a65c68e1",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-r16-static-source-coverage.mjs",
    "sha": "fabea7c3f1be1d7919cd605a6c6922c4a8acfe4f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-r16-walking-chunk-verification.mjs",
    "sha": "1fbc97c594d4b716f40e67607f0bb4cb3a11d223",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-static-cache-audit.mjs",
    "sha": "d2f97fc7511394b811d5bc7bda7455fc6ac6aaf0",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/verify-r16-walking-build.mjs",
    "sha": "ecf2a7b8cf31be2dbe32bf1bfff0e96c7f71b30a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/AGENT-CURRENT.md",
    "sha": "575869c3bd5e5ca8f3d989eb69c9f3c2323f0778",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/AGENT-TASK-QUEUE.md",
    "sha": "ebdf0bab4ad0931536d15718c17ef63da459a55e",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/BACKEND-1TO1-PROGRESS.md",
    "sha": "bd645dcfd05feb7d3c4b677d17b3890e6ee34cb3",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/CRYSTAL-1TO1-ROADMAP.md",
    "sha": "788a0775ddaf4f5862cfce9444404e8d9ce8694a",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/CRYSTAL-SERVER-PARITY.md",
    "sha": "6e3a111c143b9f40d8da4507e3b98ec4eb722d33",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/FRONTEND-1TO1-GAPS.md",
    "sha": "ec529ab31f98ab4c84abe9d2336a0e9bd968572e",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/WEB-R16-PARITY-GOAL.zh-CN.md",
    "sha": "11658137b2a1000a9ae3a80eb142d5871f0750ed",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/README.zh-CN.md",
    "sha": "d60ee28bcee85967463c0abdeae3b1ed4202f198",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/background-budget.calibration2.closed.json",
    "sha": "a1319016967338c2588fb5a6b3efb88a55247f69",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/background-budget.default100.closed.json",
    "sha": "be76b56ceaf9e4d897322bea7d9621ba90e84463",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/background-budget.default100.resource-analysis.json",
    "sha": "7d8289ee062953b90d55ba14b13d3df78353e8d5",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/background-budget.linux-native-subset.json",
    "sha": "0a9ad94640d5c5e86702f0d80c0d389ee37c8de6",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/background-budget.native143-and-integration.json",
    "sha": "29adac6f716e6343a0adf6f7d0f5877806267399",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/background-budget.windows-release.json",
    "sha": "a1e7c4ed3eb7b88f94edbc3656e9296d0a267e06",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/cloud-fd3-readonly-20261115.public.json",
    "sha": "1a908b9003b00270451aca04fba79193556f0ce1",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/crystal-paced-admission.calibration2.closed.json",
    "sha": "7719d8305d19caf13eff4ffeab2519b9e0541241",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/crystal-paced-admission.dense100.closed.json",
    "sha": "364d8bf56738694590e5895b35b6d030dc1d53c6",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/mentor-parse-default-scales.resource-analysis.json",
    "sha": "7163ec57deedf02b5ea8f867b611e640c29a779b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/mentor-parse-default100.message-types.json",
    "sha": "0ff057b204e103c8c2d1086b7550b6ad181491e4",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/mentor-parse-slow20-calibration2.closed.json",
    "sha": "3ff17bfcfe507f1e7f0099dac1994a4cd92d988c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/mentor-parse-slow20-dense100.closed.json",
    "sha": "9302569278aef7bdff1873c3cfdfa870470b3a04",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/mentor-parse-slow20-dense100.elapsed-audit.json",
    "sha": "8726a1511c4e8c1b133387d068ea79a597542f35",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/projection-mentor-parse-default25.closed.json",
    "sha": "9ed0d9b1feef2c4619ddec1ee3c4e8a0ce6dee5a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/projection-mentor-parse-default50.closed.json",
    "sha": "5f5ade10ee426e89d36b79e3bbb75c9e584965ef",
    "status": "added"
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
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/zone-admission-handoff.selected-trace100.analysis.json",
    "sha": "0e1eade6f1a17d5d69d475f9d575d84242e94d72",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/comparable-capacity-20261113/zone-admission-handoff.selected-trace100.closed.json",
    "sha": "ab7c5ca8a4c11564a9705618034624994391d07b",
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
    "filename": "mir2-web3/docs/evidence/r16-font-coalescing-20261115/README.md",
    "sha": "2cf7a49bfd46a5aa8e81e6026aa986a737ed2633",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-coalescing-20261115/candidate-en-reentered.png",
    "sha": "157d746d3f67827414dd987ddfc463e25bfa4356",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-coalescing-20261115/candidate-tw-quest.png",
    "sha": "cdcf74047e43c233cda336983606705126e0a3f3",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-coalescing-20261115/private-evidence-pins.json",
    "sha": "beb9f6089a009723776dad295fdd75c33a25b6c1",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-coalescing-20261115/public-r1-quest-and-loading.png",
    "sha": "5fee8a269cdfd3d75bd8047888564f6929141ab7",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-coalescing-20261115/source-pins.json",
    "sha": "7bddcaadf82efd6cbf916a1140110de748ea75ab",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-coalescing-20261115/verification.json",
    "sha": "b5829bd95ee4a8111ef467243c1f727bb0ac605d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-edge-cache-20261115/README.md",
    "sha": "6d92658aa3775ae9f25a665eba9c68252988d6ff",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-edge-cache-20261115/checks.tap.txt",
    "sha": "70ae924198b7a9545fe73b4bc8c807fcdf0a6cf8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-edge-cache-20261115/default-diary-open.png",
    "sha": "ec6c21ad7596835c2dddddd1def6e0a1bc5e8b8f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-edge-cache-20261115/default-reopened-diary-closed.png",
    "sha": "c03707218354702cb48bfd8ecfecf55c6903ec61",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-edge-cache-20261115/private-receipt-pins.json",
    "sha": "27642f5c71bb5c1f003a28cf127a10692da6a149",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-edge-cache-20261115/qualification.json",
    "sha": "01cb3b03e69794bc9e54fd54c73cccd25c0baa5b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-edge-cache-20261115/shared-tw-diary-open.png",
    "sha": "e90d8e98a3f07c251ed6a5672045dc906757ca20",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-edge-cache-20261115/shared-tw-reopened-diary-closed.png",
    "sha": "c2738809b98865a4f650ea117f3347977bb9cddb",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-fallback-chain-20261115/README.md",
    "sha": "7e352442150e88a2ce1ef9abd01bd0e67fcc93a5",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-fallback-chain-20261115/production/README.md",
    "sha": "ed0fed6a80eb014322d9a07dfdaba2aad583c73c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-fallback-chain-20261115/production/final-readback.json",
    "sha": "aee7701518d7886f911db2318af6fa6020efb227",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-fallback-chain-20261115/production/loading-observations.json",
    "sha": "c31f629fd4ff99b42a0c2e03e6c4f5a8ffaf104d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-fallback-chain-20261115/production/original-acceptance.json",
    "sha": "03644ba792f96c045ed12daa99e6669a1d76f441",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-fallback-chain-20261115/production/private-artifact-pins.json",
    "sha": "cd5bfd61c334c2f3ada3a76544ca7d2f1b60269e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-fallback-chain-20261115/production/public-hud-after-worker-restart.png",
    "sha": "ec0b78ff8cc45a52a89b4bb27eb12d720ccdc141",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-fallback-chain-20261115/production/public-result.json",
    "sha": "8c65bd4fc7cc093ec5da496be8b3f70257ef990c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-fallback-chain-20261115/production/qr-smoke.json",
    "sha": "fb66661304dbfece7848a2832adb0f9a8a9cf1e6",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-fallback-chain-20261115/production/qualification.json",
    "sha": "aa2ed873a72c8fca02247f21ab9e20e019df302d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-fallback-chain-20261115/production/quest-shared-zh-TW.png",
    "sha": "de58d7ac16a4a74c9c81e0b6c1bc69ec10f65efe",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-fallback-chain-20261115/production/release-workflows.json",
    "sha": "baca60b7049865233fbb074ee713f51069bea937",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-fallback-chain-20261115/release-source.json",
    "sha": "2a6ca833aaef94c55783b271e368c9d44c26c350",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-resume-20261115/README.md",
    "sha": "8a01dc6ccf9bf90fa7ef55b9781e61568c158402",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-resume-20261115/controlled-six-fonts.png",
    "sha": "c7b97b8a73e5213c564866c52bcb0caf268bf9d5",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-resume-20261115/font-checks.tap.txt",
    "sha": "b4e840653fcf8b77a1a025fe4ab3ff8d20434541",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-resume-20261115/login-checks.tap.txt",
    "sha": "991fdfefa5a40e845dfd781321762d755a9328be",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-resume-20261115/private-receipt-pins.json",
    "sha": "a18c9807515cf9b35fb45dd1ee8e74ba1da0b68c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-resume-20261115/qualification.json",
    "sha": "dee6533fac0cda270a9a0b5a3f36d2a9092803e7",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-resume-20261115/resumed-reopened-diary-closed.png",
    "sha": "321b46d9eff793ce2bd6b2c709c39f970cc50d2e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-resume-20261115/upgrade-default-diary-failure.png",
    "sha": "51f94f0c6534214e48bfda128769c39e11d53d26",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-resume-20261115/upgrade-shared-native-timeout.png",
    "sha": "b0df26930560e954f56fb9d8adec85d5318362de",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-admission-20261115/README.md",
    "sha": "18b371f0d90be7481ac8b000b44482396a4e658b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-admission-20261115/game-loading-en.png",
    "sha": "1c73e5ba7c1eaa0a4a5fb24c55795a7ff3591f04",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-admission-20261115/private-evidence-pins.json",
    "sha": "cbf36e2d0039dc81004aff8eb7a0e88521b77a66",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-admission-20261115/production/README.md",
    "sha": "280835a68fa8758faff0557eb2fe8abf2b58e2d4",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-admission-20261115/production/private-evidence-pins.json",
    "sha": "ff471648645a3a87ac05cadd1710add9d2f9754c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-admission-20261115/production/public-shared-quest.png",
    "sha": "c25df304de29a85c59be70ac58d08ca0ed35473f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-admission-20261115/production/public-window-end.png",
    "sha": "6c59daff3fb24761c34e8ec30f1d2b830239b330",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-admission-20261115/production/verification.json",
    "sha": "91a4c8e0c06d9094f1e4caf88bbb3ed5724ddb58",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-admission-20261115/quest-shared-zh-TW.png",
    "sha": "6d5e9a31305d51737b652240b6073d5f251103f9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-admission-20261115/source-pins.json",
    "sha": "6ae72cdc319ded7eeca49fe3f0a55a28dd2cf2bf",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-admission-20261115/verification.json",
    "sha": "25966d43224a5bd952f7855b448631c19e5acff4",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-foundation-order-20261116/README.md",
    "sha": "ac3270c364b5281e29f02df647d53564c398a38c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-foundation-order-20261116/baseline-dom-burst.log",
    "sha": "d9f74b5a2aefb05c9efda83d1ea78bcc7b3e535c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-foundation-order-20261116/english-hud.png",
    "sha": "868f65c6fa02b3927d396e2b7073ff6bcf9b1bb8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-foundation-order-20261116/font-chain-baseline-r1.log",
    "sha": "c312487af66e2f0d5b5681074f980df9f782e350",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-foundation-order-20261116/font-chain-focused-r2.log",
    "sha": "aeccfcada25ade5e636ffae14c42bab02c893cd1",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-foundation-order-20261116/traditional-hud.png",
    "sha": "efc2e9a414e20b5b675b1e127e4e696de70ae7a7",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-foundation-order-20261116/validation.json",
    "sha": "09cfa7b3f4c4776c70a5e5d376603a5f94b96a83",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-foundation-order-20261116/walking-baseline-r1.log",
    "sha": "13d2491218fca2a93e30473c6037f9bb3fb99302",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-foundation-order-20261116/world-r1-font-network.json",
    "sha": "a406916f49c8c2b4966ff5bbb4b4d395371cad3d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-foundation-order-20261116/world-v2-headers-font-network.json",
    "sha": "a155f1a483710b37a5a809a07ec21565bbc73b9f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-foundation-order-20261116/world-v2-r1-font-network.json",
    "sha": "8f76b941152e9da5f31dc679ae50fc7872904c37",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-foundation-order-20261116/world-v2-r2-font-network.json",
    "sha": "6888def2e7d863505faa85cbf2f8f3b2cca4e23a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-public-budget-20261115/README.md",
    "sha": "b03c76fbba932873ae72bea40f7aefd6738381cc",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-public-budget-20261115/component-ar-failed.png",
    "sha": "a567f0cd40db9a26829919e41ba8661162f725da",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-public-budget-20261115/component-en-loading.png",
    "sha": "030459527b55eaa1628e460ae779288377eebf26",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-public-budget-20261115/component-tw-retrying.png",
    "sha": "fb24fed828613334e753b8dfeb927fb3b9abce76",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-public-budget-20261115/private-evidence-pins.json",
    "sha": "69003423d10bd59a63d3e5929a8ee615966ddd29",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-public-budget-20261115/public-quest-after-native-budget.png",
    "sha": "9ebf81c097b6d7560f6e5a84143ba08771b9f636",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-public-budget-20261115/source-pins.json",
    "sha": "ae79e6b49d98cf684626eddd2b247be59200ec35",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-hud-public-budget-20261115/verification.json",
    "sha": "456d46f1cdcbc27b1848316ecca29bb30b304b04",
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
    "filename": "mir2-web3/docs/evidence/r16-map-world-baseline-20261115/README.md",
    "sha": "7e0e1998c22316944ed4db310a2fd4b28518f0d7",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-map-world-baseline-20261115/production/README.md",
    "sha": "a835b4f857fc5d31c5341310fd569840e3562945",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-paired-release-20261115/CI-ACCEPTANCE.json",
    "sha": "2c90ac29286e88b75488818b8513a843aec4591a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-paired-release-20261115/README.md",
    "sha": "7111510dba79eb4bf914f1a6b383f4826ce5710c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-paired-release-20261115/SHARED-ZONE-ACCEPTANCE.json",
    "sha": "1801713db446dff4bf70c998531de338c7d45e9c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-paired-release-20261115/private-receipt-pins.json",
    "sha": "0e4ad4e4c2e8439acfeecd96e79f39728599a91b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-paired-release-20261115/public-r1-original-classification.json",
    "sha": "00d60e364cba793aa76218025137774445217529",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-paired-release-20261115/public-r1-quest-available.png",
    "sha": "bcb7afac5cd0840f5822451ccbb0e9174b27acc6",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-paired-release-20261115/public-r1-terrain-window-end.png",
    "sha": "dd5053dd9232a10b98da982138b837036e57c096",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-paired-release-20261115/qualification.json",
    "sha": "5590a69d59550e69ec6fde8efa667b4c08d1fe4b",
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
    "filename": "mir2-web3/docs/evidence/r16-r2-range-20261115/README.md",
    "sha": "f7742d9c58c6eaee85d6ccccb5be3d18c8652058",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-r2-range-20261115/private-receipt-pins.json",
    "sha": "4a80a3744326ce9d8ee350a6638c878a090deb8b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-r2-range-20261115/qualification.json",
    "sha": "bdd4e2b8cfc122995b0486ca2ebc1eb0f7fb890d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-route-feedback-20261115/README.md",
    "sha": "2f0b6b53276ad6156f5d48c354c8638854c19ccc",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-route-feedback-20261115/default-minimap-timeout.png",
    "sha": "541937ea07081fe08ba1864ae8ed396dbefa08bd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-route-feedback-20261115/default-new-goal-and-movement.png",
    "sha": "769dad543c5988eb5445b87d96a605f858ef9d96",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-route-feedback-20261115/default-original-fonts.json",
    "sha": "ea6c83db6b3e613fd14aa34d50f9251d380a905d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-route-feedback-20261115/navigation-regressions.log",
    "sha": "1ee2226469e3dc3723364447256720ea02b1f610",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-route-feedback-20261115/qualification.json",
    "sha": "e55ad7e3435c9213b7c5c72c4b235a8520f1f7c9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-route-feedback-20261115/webgl2-minimap-timeout.png",
    "sha": "7d655c0110b7b1312cd7fd3a474e8c95fb7e08a2",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-route-feedback-20261115/webgl2-new-goal-and-movement.png",
    "sha": "8b4e74a3312e388d358a3bea8ae63057edfadada",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-route-feedback-20261115/webgl2-original-fonts.json",
    "sha": "ea6c83db6b3e613fd14aa34d50f9251d380a905d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/README.md",
    "sha": "cf76e9283aa223f76c588ff626e13a62d38360e4",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/browser-after.json",
    "sha": "665a4b0338225b723505216364a1eae8378446c1",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/browser-before.json",
    "sha": "9fc71c6ad52c27c7162cd12894917e5ddc371984",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/default-exit.png",
    "sha": "43f55bfd955d0eeda2bee98396769159eeeb8adf",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/default-fresh-goal.png",
    "sha": "5c2cb1fc31d5007511942c691774f54994a454bf",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/default-game.json",
    "sha": "bd5a110110342c1ccdc788f03b949788c849938b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/default-room.png",
    "sha": "f7137f4fb76677c4280f1430a1e9fcb6179d5f25",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/image-admission.png",
    "sha": "50100b02ef46e8beb5dbd667c823095fb175a8ab",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/live-asset-check.json",
    "sha": "4edb49785ed204c1db33473469c9450944284485",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/README.md",
    "sha": "4ac28f67feb0c8ea8845c08fa81900bf7509c8a8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/final-cache-bodies-r2.json",
    "sha": "11d9d325688e4803965a9d5d3d925ff292c8be4b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/final-readback.json",
    "sha": "56b58f6a5381e98936487805b8f8bc8098b1267a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/font-order-observation.json",
    "sha": "dc3696a05cf73bc135d07d87a4900360b20b9a1d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/observation-correction.json",
    "sha": "e2772bcc36042d11a4ab1d5263e32a1170e0158e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/observer-browser-r2.json",
    "sha": "82f59c989b36c9e5d807c89e91b27e5cd7d4d450",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/observer-checks.txt",
    "sha": "f7b87928850ca4f039f163b74dd08baac3325e56",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/original-acceptance-r1.json",
    "sha": "ed12fc5595bea0b850e99f82eda80e260a218c2f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/original-acceptance-r2.json",
    "sha": "198578e12aeb9cb31673c76444f0219ab9397b2f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/private-artifact-pins.json",
    "sha": "e84198277a5ef4bfae23c1ba073436f840ba5792",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/public-r1.json",
    "sha": "3fad4e99b30b201f45e2746935356bb724d75366",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/public-r2.json",
    "sha": "0e652870d44654e2f08f5d6b87e946e0de560eaf",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/qualification.json",
    "sha": "a7904ee18a44b9c97d69e5b82706dcad7fd81074",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/r1-public-hud-after-worker-restart.png",
    "sha": "e43339f71585a6826bba1cb814ac2504a0daa656",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/r1-quest-shared-zh-TW.png",
    "sha": "a548aba8d8ea76093701d9a4712d13212a9a9ccd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/r2-public-hud-after-worker-restart.png",
    "sha": "45f7d5324759f1356523d5282ff1297e46c49705",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/r2-quest-shared-zh-TW.png",
    "sha": "42117beaaabc240bcaa3f3354f876eeddd7dc604",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/r2-tail-diagnostics.json",
    "sha": "96ef68c7bf2683a45ddb8c55204ccd99abe0ff6a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/release-workflows.json",
    "sha": "5d4d850f2b1199ec5f22efcccc2a7e9d4a53200a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/production/rollback-qr.json",
    "sha": "bf161ff07a2fe12927731e51810a1ea8be001823",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/progress-panels.png",
    "sha": "63e80f45ddfce7d18825c3a803fe34f1079544aa",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/progress-visual.json",
    "sha": "72933d6e5cc49a3932591e2d063c574c5a59c040",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/qualification.json",
    "sha": "7304bfbffce3b08a57464d74bf54f3a93507d401",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/regressions.txt",
    "sha": "246ad5f863f3bb8f9b8e35595b228b3c84f21ae8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/release-source.json",
    "sha": "6e978430b908b8f6500d441f08bf3251899e96fc",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/resource-path-audit.json",
    "sha": "840c40c96c89cab09d33e61790d055c14f5826e5",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/retained-failures.json",
    "sha": "16d5c6eb753de07e319c158e40535ec79cc95f7e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/source-inputs.json",
    "sha": "e42379e3b594257358d1813fa3c6c7538390b76e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/webgl2-exit.png",
    "sha": "71814a1c67e75360fc5a53fcb4c56434e5e2697c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/webgl2-fresh-goal.png",
    "sha": "ee30daf7e32c42432c3deb79ec2dcca4365c719f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/webgl2-game.json",
    "sha": "b19b67dbeecf76a5732a754b155c4d08a182d126",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-static-admission-20261115/webgl2-room.png",
    "sha": "df66482ddf325fd9a1d4e5227ac76fce9177ab65",
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
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-parts-20261115/README.md",
    "sha": "b873e4e43eafe12705842795884a8b7925553964",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-parts-20261115/checks.log",
    "sha": "f0b10cee5fda0e7fc7c6f11ad00e76494e80b263",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-parts-20261115/default-natural-walking.png",
    "sha": "9ae214fd97083b43baddb4d12535dd2f262dbfcd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-parts-20261115/default-shared-diary.png",
    "sha": "a4adb09712655e1d8fd8275bd5f8453af9c873ee",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-parts-20261115/font-before.log",
    "sha": "d60de15929e1f52d7570005cb2fb4940cad7963d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-parts-20261115/private-receipt-pins.json",
    "sha": "fef30c25dfb48a5d58794901872f4844d10340cd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-parts-20261115/qualification.json",
    "sha": "9c3daaf585ea0c56e93c29b0dcebb0899b24512e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-walking-parts-20261115/shared-tw-native-timeout.png",
    "sha": "9c06062960a5d0832d3047f5dae6ae959a2bf02d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/infra/cloudflare/mir2-r2-asset-cache/README.md",
    "sha": "627faa42fe69b00380e326fc8f5b292a2a388abe",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/infra/cloudflare/mir2-r2-asset-cache/src/index.ts",
    "sha": "dcc5b4171fb63a8bbbf6171216177fac1421afa0",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/infra/cloudflare/mir2-r2-asset-cache/test/ranges.test.mjs",
    "sha": "d181440e4def1c9e3b0f32add8d61af32692ac7e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/infra/cloudflare/mir2-r2-asset-cache/wrangler.jsonc",
    "sha": "b72b01e273df6206497dce223594c826ced7a779",
    "status": "modified"
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
  "bytes": 65861,
  "sha256": "4ea2465084ba90eafe9f969a5ba3c42c8b0d2ff61fb05febb14a72f21916de40"
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
      const closure = await verifyNumeronSourceClosure(QR_RELEASE, sha,
        (base, source) => github(`/compare/${base}...${source}`), requireRelease);
      receipt.sourceClosure = { ...closure, assetSource: assetSha, ref: sourceRef };
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
