import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";

const profile = process.argv[2] ?? "release";
const requiredRuntimeFiles = [
  "public/bevy-runtime/pkg/mir2_bevy_runtime.js",
  "public/bevy-runtime/pkg/mir2_bevy_runtime_bg.wasm",
];

if (requiredRuntimeFiles.every((filePath) => existsSync(filePath))) {
  console.log(`Using prebuilt Bevy runtime package for ${profile} build.`);
  process.exit(0);
}

const powershell = process.platform === "win32" ? "powershell" : "pwsh";
const result = spawnSync(
  powershell,
  ["-ExecutionPolicy", "Bypass", "-File", "./scripts/build-bevy-runtime.ps1", profile],
  { stdio: "inherit" },
);

if (result.error) {
  console.error(
    `Unable to build Bevy runtime: ${result.error.message}. ` +
      "Install PowerShell or commit a prebuilt public/bevy-runtime/pkg package.",
  );
  process.exit(1);
}

process.exit(result.status ?? 1);
