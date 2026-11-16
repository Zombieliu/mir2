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
  source: "f05fb757ef4dcac75dd5bfbdd2b23ddb67ce810b",
  sourceRef: BRANCH,
  sourceCheckpoints: SOURCE_CHECKPOINTS,
  baseSource: "e0ceea22964c03041d45e49e506b3a8badd6984e",
  assetSource: "f2de762309768b51b39438c2437c14e819df9bf7",
  sourceCommits: [
  "2dd2d1ef3c3ca899e1a488caa7d7873be97fddb8",
  "7de1b38dd26d969bc5daf7cae64261ab70e3285c",
  "306066f29590d1e2e14155d83644fbc19f11a4c3",
  "b87aa402a38aeb70f30e5afe285862cd8ecf7a46",
  "2b7b5d7079300f45315e172d4c58784aaed6fb5c",
  "8bf3dbcbeae86c0f4782e2c7dcdd8dd203756efe",
  "133d1c790d2069a31ac8698d418034fa609b1176",
  "7c50e19e8152ef172bf877073657356eaa0a0efe",
  "fea64484b6a25ccd59a22d8f52a5718aa6e57df6",
  "2cf385ceaa1a99ad0dc472f6123d703bb4064010",
  "bf97cdd479c25d71bcb7f51438740fc5d3ee95c8",
  "8ebc4f2c2eb06dbe7a6d3455effcebb693623041",
  "c4b2e44c5ed2ca59050f092dbeb8ec7702b7a8a5",
  "f05fb757ef4dcac75dd5bfbdd2b23ddb67ce810b"
],
  changedFiles: [
  {
    "filename": ".github/scripts/numeron-r16-release.mjs",
    "sha": "b3d224c561b45b44aca129811e909e4df5899af9",
    "status": "modified"
  },
  {
    "filename": ".github/scripts/numeron-r16-source-checkpoints.json",
    "sha": "c027289256cc7fdd98975ed6e31649324b59e3b6",
    "status": "modified"
  },
  {
    "filename": ".github/scripts/test-numeron-r16-release.mjs",
    "sha": "4443c1dc66bfa968920c96483bc7b9c513a6371b",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/app/components/original-client-shell-types.ts",
    "sha": "3af5a0817b3a03416f23097e31bda930456fea59",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/app/original-client-shell.tsx",
    "sha": "a880dc7691861a26a426761d363f1dff599408a4",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/app/page.tsx",
    "sha": "52499fb2a833d4fec606c74b2b1405a0f9ec8aa2",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/lib/dubhe-wallet-login.ts",
    "sha": "aead9ca323d34f139811d1ca08dd9ffdb22d5bea",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/lib/r16-entry-scene.ts",
    "sha": "6503b69c193613481f5d2ab588e22308a61d9828",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/lib/server/dubhe-connect-auth.ts",
    "sha": "9f5ff78fb8fa33cbfe812dc4e965f798f06f59f3",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/lib/use-bevy-quest-ui.ts",
    "sha": "b75c7c97f0b4e22f9d8e89585a4b7d412645eb58",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/package.json",
    "sha": "5fa6bdabd4ad0a47bb5c382a41fdf6eb43c6e451",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/public/mir2-asset-worker.js",
    "sha": "9e9c23eaebd462ff7f51d0a103670834e032e36b",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/generate-font-range-pins.mjs",
    "sha": "e08765e1c1db23cb8e6b74f91606e3c64bdda0c1",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/smoke-asset-font-ranges.mjs",
    "sha": "fb165c94f919f1d52b4c037a226df80955f77e19",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-asset-worker-cjk-progress.mjs",
    "sha": "6b2f1f8a48cccdf59473e5ed1d9d2718c8ec9275",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-asset-worker-font-ranges.mjs",
    "sha": "80122f6bd65b716d0255044ee80db486eca75d6b",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-bevy-quest-ui.mjs",
    "sha": "b05adf25a71d8c8e18b6bd3805e0f0206c008945",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-dubhe-connect-auth.mjs",
    "sha": "d84c38c6b90d5b5d78dd5bf336a9e5b178a74f81",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-dubhe-wallet-login.mjs",
    "sha": "d277501614ece5d9270e4ad679ac87aa2b2dfe7d",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-r16-entry-npc-recovery.mjs",
    "sha": "8a4be83a43aae4ecac8213e6f676380259faf8b0",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-r16-entry-object-recovery.mjs",
    "sha": "09b4c4716a5f4a03ca55cea319c0481f70299d9a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-r16-quest-close-shortcut.mjs",
    "sha": "b3b2d13e8a5f904589cd58db8bf2172b15df65aa",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/apps/web/scripts/test-r16-quest-navigation-handoff.mjs",
    "sha": "ca4327bac6c3d8c5b32da284d122cb4a1bc75f6d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/AGENT-CURRENT.md",
    "sha": "33032ed51981c2748a694eafbf33d0da65c5d761",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/AGENT-TASK-QUEUE.md",
    "sha": "9a27335079d1ac7ccd065452dc1761fb1b3f3f97",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/BACKEND-1TO1-PROGRESS.md",
    "sha": "3886dcb85bc359481e2815ed700d2fd635ea1a2b",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/CRYSTAL-1TO1-ROADMAP.md",
    "sha": "ae11c04a54014e18aa2cef6ae627c03f26f3f906",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/CRYSTAL-SERVER-PARITY.md",
    "sha": "d7f4e5234557312bda8464bd7725dc9963daada7",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/FRONTEND-1TO1-GAPS.md",
    "sha": "223ecf351faffda6cb6f091e55362a0ff00b00de",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/WEB-R16-PARITY-GOAL.zh-CN.md",
    "sha": "4a909b2f912b2b04c65c9dccb4b341264c97045c",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/README.md",
    "sha": "ad860a54962f38c9f7d069de16ff6cf991bec21e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/cleanup-confirmation.json",
    "sha": "65ea12bd84226596e40d7dc87312239d4824c392",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/control-api-probe.json",
    "sha": "89f6abff24061a5967745740b51a28797916851f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/control-findings.json",
    "sha": "f8040a0e28280d296c7167ca5813e6f47fb03c47",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/control-loaded-auth.json",
    "sha": "afe9723c310ec908cbaf63f2f654e1765a186f7d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/control-loaded-summary.json",
    "sha": "bf7027e463ea3e935b48593308ef3a02790bf2de",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/isolated-build.json",
    "sha": "995701d86279e1cd7dd0f4c558b7732041fa6b51",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/local-auth-r1.json",
    "sha": "654c14d72fd7e9a2eec43f6a335059df8572161c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/local-auth-r2.json",
    "sha": "875a3adde5a4b77b318ec337f3ecdcea1beff387",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/pending.png",
    "sha": "fdc9a70dc42c5323b669a9275beed7d93e1d4984",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/README.md",
    "sha": "98c7cc3f3c1445c3b8254490dfc6a487eccfedb3",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/alias-after-rollback.json",
    "sha": "1edceab6966125619bd196afc7b7dfd011085a9b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/artifact-transfer.json",
    "sha": "9d9148f612550ed05d424f773faf1d096e19cccb",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/backend-rollback.json",
    "sha": "e2200bb8fde41c167cca5756ac4f80140f5c5a63",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/backend-rollout.json",
    "sha": "e5d126a8157335eb847a6d7d424fa84ca7f70e21",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/evidence-manifest.json",
    "sha": "243176cab05d194f40df74d8915d1d87620c8a82",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/first-playable-terrain.png",
    "sha": "ce51926072fdb1cdc8f8f0297acf3814fd6161ce",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/linux-package-verification.json",
    "sha": "d487cf2ae17447ab96a428345670017a769ca661",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/mobile-browser-closed.json",
    "sha": "1078ac49091e495b209aade464a5cdbcb6af58de",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/owned-ssh-closed-after-rollback.json",
    "sha": "ab11460ee4e66c3fffda0a568c2939a6e3b77ddc",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/passive-failure-analysis.json",
    "sha": "2f4ac973a02548ab1d2979bcc2eac1678d73f016",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/phone-approval.json",
    "sha": "bec2a1033c4816cf0cbf9844372d74badd32e10c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/postrollback-independent-http.json",
    "sha": "afbd3593a620b3d66f290695661b5d0b14096950",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/postrollback-preservation.json",
    "sha": "1c71283caf84e3618f9e3e2372714f765306cbf3",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/public-r1-original-acceptance.json",
    "sha": "3545bfc5260787c06555a756835299d93458b121",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/public-r1-result.json",
    "sha": "5809c5cef14c3b2f93f8cee4cfbdd8e06fb6aca8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/quest-failure-stage.json",
    "sha": "844d8ec51fdd4ecfe2e90c2e8d12aadc4e2b2daf",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/quest-key-observations.json",
    "sha": "e4148a51cf941d7d761e3ad52bd81207a5199c32",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/quest-left-open.png",
    "sha": "4fe06fce6ea81805b44fa10f47b8767166a563dd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/restored-qr-zh-cn.json",
    "sha": "8fc2142174044365bdd7a301f5cdcc08624d4c44",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/restored-qr-zh-tw.json",
    "sha": "695a3d5c0ae6621bc8cae9db40ebdf4e907f0b47",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/same-source-local-results.json",
    "sha": "6dc7c146fa3ab7f2520a7fee7633517310ce8df4",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/server-backup-summary.json",
    "sha": "a2c09a9164a3adecc8251b793a55dac9a8e346af",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/server-canary.json",
    "sha": "33ea42f330edce679f463b9fddc95ef8ce0fd97c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/status.json",
    "sha": "048f969ff74d6b2a3797ccd0ac4ae84404c021c9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/wallet-network-guard.json",
    "sha": "91ceac0c156246faef0cda440db0971279047753",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/web-check-summary.json",
    "sha": "e6a9041e68287ccdb96da86fef0c73f655d2d622",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/web-promote-summary.json",
    "sha": "59a183f23e4cbe7f55717fa914aa4ef727b27b91",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/web-rollback-summary.json",
    "sha": "cec8b130a7250ed4761f986199f6fe5b59a33eb8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/production/web-stage-summary.json",
    "sha": "443c14ad998ee16db5b5af0c5fa2edb56999438b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/release/README.md",
    "sha": "093f00d82d319a6e9849ae5424bf77d43539f286",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/release/baseline-readback.json",
    "sha": "24bc80d11ae4a0b87982525ad93ed969aac3631e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/release/evidence-manifest.json",
    "sha": "a9cfb2a7622cb8cb1b5f91e4919ea58e9790d371",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/release/release-guards.log",
    "sha": "ea2a138a107032b00d97307affa3abdac8c85d4c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/release/release-pin.json",
    "sha": "02b9ff197066869e89f0981a9196a63cf7f3fa2b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/release/source-closure.json",
    "sha": "ebed5dfaec95161bba82586a84a24081818ddecd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/restoration.json",
    "sha": "ee9ccad139605b271184b5532ebc6c6ec50dfe97",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/select-raw-cdp.png",
    "sha": "45ee9ee5499cf66cf48d7501c85edf0e3b13111c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-auth-handoff-20261116/validation.json",
    "sha": "19490438172abb88d5b21eff4d95286cb3c4b38e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/.gitattributes",
    "sha": "905813169aade6a5340ca6b9122430f60e17b380",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/README.md",
    "sha": "206db3ad8d471ff0f233cec0c4f5e8ba684963cb",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/adjacent-regression.log",
    "sha": "35f61105c8ef55ca9dba02be103da14f1123b563",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/chrome-sc-prefix-adjacent.json",
    "sha": "77717d3617722b4541b4000822a2f3067bb08908",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/chrome-sc-progress.json",
    "sha": "41788d9367fb8ee9957660f00e7774528ccd64c1",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/chrome-tc-baseline.json",
    "sha": "364af58f4b59f2ffb5f105a45304a4ceda7132bc",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/chrome-tc-prefix.json",
    "sha": "ba0f83ce996f8a40ebfafa8f0b6e412d54cdb9c7",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/chrome-tc-progress.json",
    "sha": "7b5fc25f8edd01436fa7abe2e0148baaa95fabd9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/chrome-tc-range-stall.json",
    "sha": "fc7a7dcff54a2095521f3841010170ca5b2a6374",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/english-native-quest.png",
    "sha": "b2e54ec6d780f455302f737033a7cf072f4ce5cd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/evidence-manifest.json",
    "sha": "d4c283b4a7f7ca6e565c8787b9d8a8f587a5dd77",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/focused-regression.log",
    "sha": "c64ee86e99eabeac6ec67416f68d901c1aa3bc95",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/old-worker-expected-failure.log",
    "sha": "a9372605efa1f06252ba43a7a864af1445f6e622",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/preceding-public-observations.json",
    "sha": "5b9e2865f54abb711d2988f3c5f97344457c258a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/.gitattributes",
    "sha": "b87185bfcf6b78d8e740d29f3fa1ff92f595c2c6",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/README.md",
    "sha": "35a0e47881744b7abbe0f4c96c6073dc0240b1c5",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/alias-final.json",
    "sha": "6fcde2bfabd3a568d20baac1363035195a4b3b54",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/backend-artifact.json",
    "sha": "c788f331247e8cf8010f48296a3aba5d692c39bb",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/backend-rollback.json",
    "sha": "a8bbd5ebd6f65a047fb6558e59b9132ae9b78e82",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/backend-rollout.json",
    "sha": "414862b27e0a36b159608c217a91de3095b284c6",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/canary-protocol.json",
    "sha": "912eaa1d7fde2a3d5ba854d891f5f234fff2afc3",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/evidence-index.json",
    "sha": "1e7454da702b92adc92aa70a5cc4eba431ce196d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/font-tail-diagnosis.json",
    "sha": "28d952a7fa787dda36c1ba471c5fc05c0d2bcbcd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/independent-candidate.json",
    "sha": "d54ba54d0dbb0eb413f00640c261ce07d8365f93",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/outcome.json",
    "sha": "54570e1c1b9063be23ecd265ef40b02efcfc4918",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/preservation.json",
    "sha": "fea2fd22a906997bbe29afd2543b93ef262b2445",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/public-q-pixels.json",
    "sha": "eadb9e0ae5f879ae1af6bd7903c6edc49e0e859e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/public-r1-acceptance.json",
    "sha": "cd57c5abc6870ef4381c344be786e43ffc11287c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/public-r2-acceptance.json",
    "sha": "d251f3e63e7db72e16c9f2c61cbefb436aa65dc8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/r1-first-q-black.png",
    "sha": "1426a70f48edb42d3855d4b90850f46ff2faf180",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/r1-native-q.png",
    "sha": "5fd0f428929ee15689b5637073d0db2faa424cfd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/r2-first-q-fallback.png",
    "sha": "398b88f665cfd62e0522ba34f2d144e747c2f01e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/r2-native-q.png",
    "sha": "557c9999bdf5367bc13acca243198f79c9b1ae40",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/web-check.json",
    "sha": "674faa9543797bfa9f8c59a5372f6b3adc9a4743",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/web-promote.json",
    "sha": "d932045de196d92462a07c31748a34b3819ffc2c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/web-rollback.json",
    "sha": "a29de3ed43bc9e1be8847ca584830161ed44fda6",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/production/web-stage.json",
    "sha": "01ddcbe5b31209ffa9c2837831aff38de7f6ac1e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/public-e0-failed-font-trace.json",
    "sha": "06c344828ab7aa5baf5cfb12e43a1f11c9535af5",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/release/baseline-readback.json",
    "sha": "130cb2d5d6a51ab94e825312b97c0f91b2118672",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/release/manifest.json",
    "sha": "4dc4236bdd17d87cec2ced4a849c4a69ffb1f16e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/release/release-guards.log",
    "sha": "5dc69b0d8390b20581d028df54f50e41822065f8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/release/release-pin.json",
    "sha": "2e2989660a855e384a390242e3fcc32926f4d464",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/release/source-closure.json",
    "sha": "48aa1d134f840ab9b32c8e6e45e4d81d8d99278f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/runtime-catalog-observation.json",
    "sha": "168afb6ab817a93939f6bbf8d580b6b256e608d7",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/six-original-fonts.png",
    "sha": "c7b97b8a73e5213c564866c52bcb0caf268bf9d5",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/traditional-native-quest-supplement.png",
    "sha": "866b1417e0262b8b2cb7fee6366a6dd52fcdb066",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-cjk-progress-20261116/validation.json",
    "sha": "ae38c0edf2cfb95cc6d80b0ffeb13ddfffd0f372",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/README.md",
    "sha": "4daa20bdea9f156ed664a7c308325d7fa1b9e9bc",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/alias-readback.json",
    "sha": "ec9a32efb4137cb9c3b774180a5d4de02bb6a5c2",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/evidence-manifest.json",
    "sha": "7b378c068d21f11a418f8199124d46956e5bc64e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/isolated-build.json",
    "sha": "f6a32423e98ef0f591aebfbb9fed4798f62f9cf5",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/public-readback.json",
    "sha": "7352f71813c5ff778bf3e5925a3b5eabaa9c42ce",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/restoration.json",
    "sha": "d52f04090ee49238c2bc5977eb0e4bdd1db22768",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/source-ownership.json",
    "sha": "ffc4ec286e7f90a5d741cd8542b0d308bfd0bea6",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/status.json",
    "sha": "8718f0145e2d1ac68375dc51a6dd089caa01b274",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/unchanged-control-diary-observations.json",
    "sha": "cfa29d849116744236de095afe0a43058c41965c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/unchanged-control-summary.json",
    "sha": "9465a364d35b16488fa631bc28f6bacc6e168fe1",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/validation.json",
    "sha": "8bc554eb92b9e754a51762db358c80bf137b3ab9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/webgl2-tc-diary-closed.png",
    "sha": "18d93da1ddd25d014ac571c311f7d4cbfd0a8ad8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/webgl2-tc-diary-open.png",
    "sha": "fea95c1b6fd4456539ad5272a5fa54db51a32455",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/webgl2-tc.json",
    "sha": "501435515c47b9a509f7e3d1a05859aeaefdb7e9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/webgpu-tc-diary-closed.png",
    "sha": "183f161196920c84aba31b93df35bab5fd9a83e4",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/webgpu-tc-diary-open.png",
    "sha": "e716b0402ac4075685e7ecdfdde3330d3b5ccd3a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-nav-handoff-20261116/webgpu-tc.json",
    "sha": "01cde1eb5bcaa7e1630c87b5eeb31356a73eb926",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/README.md",
    "sha": "a6846aba9e5dfc0330b2d32b78b43e1c2b1f683a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/alias-readback.json",
    "sha": "86fa840f3d3449582ea98edccb979e10202403be",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/evidence-manifest.json",
    "sha": "9d4425948b43475cbdaefb3dd708787eac6a5351",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/isolated-build.json",
    "sha": "8aa8830c3527bc49bedfe201c27f7b312d11e855",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/prior-coverage-correction.json",
    "sha": "0765d06bb8278726004bf350388e2286dd91ef95",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/public-readback.json",
    "sha": "63c188211b496b17896a1807072b12d8a0707f9a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/r1-original-capture-failure.json",
    "sha": "05e0050a7af6f596c210f1e9228b386593573f20",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/red-regression.json",
    "sha": "d95785e7ece0c0ba642b7cee5241f7fb80d61f4e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/restoration.json",
    "sha": "a7687f3efc2dbd18fb511a03a40696ed8e043f32",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/status.json",
    "sha": "ea56b6e1dc36d1093ad9a2ea9502f3059636769f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/validation.json",
    "sha": "a5b55d5c35f2381ecc5f0a2211a89804a6b01876",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/webgl2-tc-diary-closed.png",
    "sha": "e98db90d6fbb5b78f0226e707a37ce8be9e7d75c",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/webgl2-tc-diary-open.png",
    "sha": "9146611369675571958c4ec97440e9b180081f3e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/webgl2-tc.json",
    "sha": "095f292a379840fa8d72e1ae2358bd5189aba888",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/webgpu-tc-diary-closed.png",
    "sha": "27047fdfc79a57864b70df602a8d107adb6c0ca3",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/webgpu-tc-diary-open.png",
    "sha": "38567ebc8ca99e2e68b0830bdb8f490296877814",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-diary-world-churn-20261116/webgpu-tc.json",
    "sha": "948780a3dba497b3678617795eb3ff898de63289",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/README.md",
    "sha": "211cc31184d5d462431fdcc0c2bf4f80533cf0ce",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/alias-readback.json",
    "sha": "45afe8bb0c1664c3389bf499eba66a1a498b96a2",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/evidence-manifest.json",
    "sha": "8d4bafce95d4c90839894c2648491561eeef6e13",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/isolated-build.json",
    "sha": "2e5be4bf4813977215294b66d03749ae82dd13fd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/next-Q-investigation.json",
    "sha": "ecac6ed126538c4fddc4b1cdc5aa262202579c0b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/public-readback.json",
    "sha": "36b6527b19efb8855a075f988eb91081ccff56bd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/restoration.json",
    "sha": "d8c52a896c435cd8f4b8dd7df2756c4eed706d7b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/source-ownership.json",
    "sha": "50bafdb4e785a2b675acc27be460700b4420f868",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/status.json",
    "sha": "263c9635a5e0f3fa729c0221ad21863c9f3ce7f3",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/validation.json",
    "sha": "31e1caae010b71e8bef7dbec257216038951c44f",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/webgl2-original-Q-observations.json",
    "sha": "14315ccd23aa3fd9f37b15af0ace55605445a462",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/webgl2-tc-objects-recovered.png",
    "sha": "a03da6e4a1827968e7c678fde71497c5654b691b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/webgl2-tc.json",
    "sha": "77b6a5475086436751eb231c6d883e42143f69f4",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/webgpu-tc-objects-recovered.png",
    "sha": "4884bcb30ab18c349b5aa25c987f96b9684439de",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-entry-object-recovery-20261116/webgpu-tc.json",
    "sha": "f971c04e10c7dbc3d100de076ffcb5b5add4c215",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/README.md",
    "sha": "91bb62161dfcbbf53a42a66d0698b9c348c0c9ee",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/adjacent-final.log",
    "sha": "edc91b80584b7ec4d2590b940a95f12699c0cb33",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/before-regressions.log",
    "sha": "b06bb6b73f457f0eb1dc40faa6f4bed70fc945f0",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/chrome-baseline-chunks.json",
    "sha": "7575d294f9ac9679ec582c52102c5f0913b00663",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/chrome-header-failover-body-observer.json",
    "sha": "4f8db8197fac5f3ba2bff3ee6f14bb19e7b731ab",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/chrome-header-failover.json",
    "sha": "882b8fe45579ea7be1d8d451ff03b53c8cc25130",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/chrome-sc-chunks.json",
    "sha": "2e65588b05d81f6546777e5473a8589a625925b3",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/chrome-tc-chunks.json",
    "sha": "c0fd8c250fd6a1f06f51bb4721e68ab23bca30b4",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/disk-preflight-failure.json",
    "sha": "61c3446ad10fc5b6ab48d86d6e3d65b8fd0f02f9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/english-native-quest.png",
    "sha": "24ca7454e3322df23d7c211b2d46f793e8654889",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/evidence-manifest.json",
    "sha": "1a3f187c639954b9105de6eafd22f74eb094f4e7",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/focused-final.log",
    "sha": "da0e1071c25d73c36d0c08b9eb6021a09c44908b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/preceding-public-failure.json",
    "sha": "28d952a7fa787dda36c1ba471c5fc05c0d2bcbcd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/production/README.md",
    "sha": "2f8fac6555e6ad2cecec43c8eebf6e97c7deb1ff",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/production/artifact-transfer.json",
    "sha": "2da84d5443e7b688790c30bebc02d37e071fb115",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/production/auth-handoff-diagnosis.json",
    "sha": "e0b47e8fd334491337612f04bda06d5161516525",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/production/candidate-asset-readback.json",
    "sha": "ee81e80a74a7493e62659a84e0e329a9d0b51830",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/production/evidence-index.json",
    "sha": "eb93cf9cf7f440dd1d7af59508a26377f16c2342",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/production/linux-package-verification.json",
    "sha": "f198d52c1ca8f397bd0b0eb9b3677891a6f8ab05",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/production/ordinary-canary.json",
    "sha": "1e116c4a441a0d9bef53b7916b13562535df23a9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/production/original-r1-gates.json",
    "sha": "53f6ea9e901f172cc7c2b8da986d26c20d0b1382",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/production/outcome.json",
    "sha": "722345db694100527356069a269d57cba901a3ee",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/production/postrollback-qr.json",
    "sha": "29c66a0196eb2a0278339fa8d909165f0dca7dc1",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/production/web-jobs.json",
    "sha": "85cb880120352207537109250f9f42327622d560",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/release/baseline-readback.json",
    "sha": "df9df593d7aecaa72efa77088dda77cd05e19643",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/release/evidence-manifest.json",
    "sha": "e4f7a0355dcf39d6c421d49edc123ba5efa4e4d9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/release/release-guards.log",
    "sha": "ca2c10c85cc884bd6ec3ca2cfb1138dc2210438d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/release/release-pin.json",
    "sha": "bc7e33043e01f9dfc7b112ffa0a4fa293e8b802a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/release/source-closure.json",
    "sha": "2d749fc067eca5950a5f9608b4e662b93ba58c2e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/six-original-fonts.png",
    "sha": "c7b97b8a73e5213c564866c52bcb0caf268bf9d5",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/traditional-native-quest-supplement.png",
    "sha": "0f98d3a64269c74a32aaf879c97f0a457925d1ff",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-body-progress-20261116/validation.json",
    "sha": "3d2531cb5ce31c29aa82a9f815b7bd1f9efeece8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/README.md",
    "sha": "929d1ef4380ebbe0d84925a6bc8f4f40e24dcde6",
    "status": "modified"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/.gitattributes",
    "sha": "fcbe398c597b2d35bdc16c529d6bd56d4985159a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/README.md",
    "sha": "28e075055066dbf29a03e9288d6aa2e4e7f482fd",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/alias-final.json",
    "sha": "0bb1e38b60705e00fda70def0c72dd0847ae78a2",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/backend-artifact.json",
    "sha": "192b620d1b027941507796ddea8c3e484bea55ea",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/backend-rollout.json",
    "sha": "3514463cfbc80038866d3bde3a48cda6ad62db55",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/evidence-index.json",
    "sha": "f4018aa4039923b3dfacbaff4ecfdc68a6760303",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/final-public-readback.json",
    "sha": "f047b7590b1d7186e8763b396bcabf19ea049b4e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/independent-candidate.json",
    "sha": "d73b9584b8ca0b5225031b7884c2b3cc93e13755",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/independent-preswitch.json",
    "sha": "67900982421ea1469eb9da10f2cd78fcdd5b3b0d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/preservation.json",
    "sha": "64b97e30fff58c28ca6fef944b66dae5684d60a7",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/public-q-phases.json",
    "sha": "7a21a1ef215e41a5599b6bec31baadad9446bceb",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/public-q-pixels.json",
    "sha": "e25ce1bb2277ba9df9cdb63c9abf9d463de7408b",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/public-qr.json",
    "sha": "c8c15735a8af53213a5a035c274fa879709a8239",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/public-r1-acceptance.json",
    "sha": "e77c482633c2478a633f1178ad677dfb3cc96bf8",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/public-r2-acceptance.json",
    "sha": "23c1d72e9d2a2d18a4c7a70e8918c7a25e2212e9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/public-summaries.json",
    "sha": "92acfd61c5291835c88084a71f61c3c39f268585",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/r1-first-q-black.png",
    "sha": "1426a70f48edb42d3855d4b90850f46ff2faf180",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/r1-native-q.png",
    "sha": "de1ca5bb248386631939713d39835673fe261d9d",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/r2-after-worker-restart.png",
    "sha": "cdc636c1ad7947d319bdc494dd3149233da4850e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/r2-first-q-fallback.png",
    "sha": "cbb41f498f75f6815237e9b7a45bffa1b2b9af1e",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/r2-native-q.png",
    "sha": "efc25618c8664dc28c2ecff3d9dff89b69b48814",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/server-canary-qualification.json",
    "sha": "ee06d14463ff3252c7b04e2b8de8091dbb4e34b9",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/web-check.json",
    "sha": "d6cd1a9aede3e403a5202e8b40c88d212e3d921a",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/web-promote.json",
    "sha": "2af7033d8e5afb28e89a03c11f4a37327abf1bb1",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/production/web-stage.json",
    "sha": "01a3efbc1d2d530d031405d8ce46acd875b981cb",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/release/README.md",
    "sha": "7730f27f0089ba7a4c095d2c86e51dbcff3f36b4",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/release/release-guards.log",
    "sha": "f9a296d7eac51ecb83b9baf2097b5d2282e090cb",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/release/release-pin.json",
    "sha": "5c1c8b6dd32b11dbacc7095c11c0c664754ea1fe",
    "status": "added"
  },
  {
    "filename": "mir2-web3/docs/evidence/r16-font-prefix-recovery-20261116/release/source-closure.json",
    "sha": "dfb1adcfa677426df57ac2e5418cdec51a2ef5f6",
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
  "bytes": 73875,
  "sha256": "2333c5f6c64c05c2485c65bf4965df859e25d0bc446f72ac72ab716885f4aba2"
},
  assetRunId: '42494701555',
  baseline: "dpl_FoNUgjSRpw56hwUFDNrSs7LZZQFN",
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
