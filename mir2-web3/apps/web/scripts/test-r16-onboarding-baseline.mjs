import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";
import ts from "typescript";

const read = relative => readFileSync(new URL(relative, import.meta.url), "utf8");
const baseline = JSON.parse(read("../../../packages/game-data/data/r16-gameplay-baseline.json"));
const webBaseline = JSON.parse(read("../lib/generated/r16-gameplay-baseline.json"));
const helperPath = "../lib/r16-gameplay-baseline.ts";
const page = ts.createSourceFile("page.tsx", read("../app/page.tsx"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);

function descendants(node, matches) {
  const result = [];
  function visit(current) {
    if (matches(current)) result.push(current);
    ts.forEachChild(current, visit);
  }
  visit(node);
  return result;
}

function compile(source, globals = {}) {
  const result = ts.transpileModule(source, {
    fileName: "r16-onboarding-fixture.ts",
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, esModuleInterop: true },
    reportDiagnostics: true,
  });
  assert.deepEqual((result.diagnostics ?? []).filter(d => d.category === ts.DiagnosticCategory.Error), []);
  const module = { exports: {} };
  new Function("require", "module", "exports", ...Object.keys(globals), result.outputText)(id => {
    assert.equal(id, "./generated/r16-gameplay-baseline.json", `Unexpected baseline runtime dependency ${id}`);
    return webBaseline;
  }, module, module.exports, ...Object.values(globals));
  return module.exports;
}

function helper() { return compile(read(helperPath)); }

function baselineImports() {
  const imports = page.statements.filter(node => ts.isImportDeclaration(node)
    && node.moduleSpecifier.text === "../lib/r16-gameplay-baseline");
  assert.equal(imports.length, 1, "Page must consume the shared R16 baseline helper");
  const bindings = imports[0].importClause?.namedBindings;
  assert(bindings && ts.isNamedImports(bindings));
  return bindings.elements.map(binding => ({ exported: binding.propertyName?.text ?? binding.name.text, local: binding.name.text }));
}

test("the Web R16 onboarding projection is the exact canonical Native gameplay baseline", () => {
  assert.deepEqual(webBaseline, baseline);
  assert.equal(baseline.schema, 1);
  assert.equal(baseline.id, "r16-platinum-fiveclass-newcomer-v2");
  assert.equal(baseline.contentProfile, "platinum_fiveclass");
  assert.equal(baseline.questGuidance, "newcomer-v2");
  assert.equal(baseline.questCadence, baseline.questGuidance);
  assert.equal(baseline.onboarding, "native");
});

test("the Web profile and automatic tutorial policy come from the shared baseline helper", () => {
  const policy = helper();
  assert.equal(policy.r16QuestGuidanceProfile(), baseline.questGuidance);
  assert.equal(policy.shouldAutoStartR16Tutorial(), false);
});

test("all three Page Quest consumers use the same R16 profile helper without a deployment override", () => {
  const imports = baselineImports();
  const profile = imports.find(binding => binding.exported === "r16QuestGuidanceProfile");
  assert(profile, "Missing canonical Quest profile import");
  const calls = descendants(page, node => ts.isCallExpression(node)
    && ts.isIdentifier(node.expression) && node.expression.text === profile.local);
  assert.equal(calls.length, 3, "Quest dispatch, its authority stamp and the shared renderer must use one profile");
  for (const call of calls) {
    assert.equal(call.arguments.length, 0, "R16 baseline may not be replaced by an environment-derived profile");
    assert(ts.isPropertyAssignment(call.parent) && call.parent.name.getText(page) === "profile");
  }
  const oldProfileCalls = descendants(page, node => ts.isCallExpression(node)
    && ts.isIdentifier(node.expression) && node.expression.text === "questGuidanceProfile");
  assert.equal(oldProfileCalls.length, 0, "Page must not retain an independent profile selection path");
  assert.equal(descendants(page, node => ts.isPropertyAccessExpression(node)
    && node.name.text === "NEXT_PUBLIC_MIR2_QUEST_GUIDANCE").length, 0);
});

test("fresh keyboard, touch and controller entry cannot automatically start the legacy card tutorial", () => {
  const policy = helper();
  const globals = Object.fromEntries(baselineImports().map(binding => [binding.local, policy[binding.exported]]));
  const autoEffects = descendants(page, node => ts.isCallExpression(node)
    && ts.isIdentifier(node.expression) && ["useEffect", "useLayoutEffect"].includes(node.expression.text))
    .map(call => call.arguments[0]).filter(callback => callback
      && descendants(callback, node => ts.isCallExpression(node) && ts.isIdentifier(node.expression)
        && node.expression.text === "startTutorial").length > 0);
  for (const input of ["keyboardMouse", "touch", "gamepad"]) {
    for (const family of ["generic", "xbox", "playstation"]) {
      let launches = 0;
      for (const callback of autoEffects) {
        const fixture = compile(`exports.effect = ${callback.getText(page)};`, {
          ...globals,
          screen: "game",
          spectatorMode: false,
          clientProfile: { input, gamepad: { family } },
          window: { localStorage: { getItem: () => null } },
          startTutorial: () => { launches++; },
        });
        fixture.effect();
      }
      assert.equal(launches, 0, `${input}/${family}: Native onboarding must not mount the legacy automatic tour`);
    }
  }
  const initialTutorialState = descendants(page, node => ts.isVariableDeclaration(node)
    && ts.isArrayBindingPattern(node.name) && node.name.elements.some(element => ts.isBindingElement(element)
      && ts.isIdentifier(element.name) && element.name.text === "showTutorial"));
  assert.equal(initialTutorialState.length, 1);
  assert(ts.isCallExpression(initialTutorialState[0].initializer));
  assert.equal(initialTutorialState[0].initializer.arguments[0]?.kind, ts.SyntaxKind.FalseKeyword);
});

test("Native Candidate default TOML and the package parameter resolve to the same canonical Quest profile", () => {
  const script = fileURLToPath(new URL("../../game-client/platform-windows/scripts/candidate-release-profile.ps1", import.meta.url));
  const packageScript = fileURLToPath(new URL("../../game-client/platform-windows/scripts/package-windows-candidate.ps1", import.meta.url));
  const quote = value => `'${value.replaceAll("'", "''")}'`;
  const command = [
    "$ErrorActionPreference = 'Stop'",
    `. ${quote(script)}`,
    "$tokens = $null; $errors = $null",
    `$ast = [Management.Automation.Language.Parser]::ParseFile(${quote(packageScript)}, [ref]$tokens, [ref]$errors)`,
    "if ($errors.Count -ne 0) { throw 'Native package parameter syntax is invalid' }",
    "$parameter = @($ast.ParamBlock.Parameters | Where-Object { $_.Name.VariablePath.UserPath -eq 'QuestGuidance' })",
    "if ($parameter.Count -ne 1) { throw 'Missing unique Native QuestGuidance parameter' }",
    "$packageDefault = if ($null -eq $parameter[0].DefaultValue) { $null } else { $parameter[0].DefaultValue.SafeGetValue() }",
    "$contentParameter = @($ast.ParamBlock.Parameters | Where-Object { $_.Name.VariablePath.UserPath -eq 'ContentProfile' })",
    "if ($contentParameter.Count -ne 1) { throw 'Missing unique Native ContentProfile parameter' }",
    "$contentDefault = if ($null -eq $contentParameter[0].DefaultValue) { $null } else { $contentParameter[0].DefaultValue.SafeGetValue() }",
    // URI syntax validation only: this pure function neither opens a socket nor writes a file.
    "$native = New-CandidateClientConfiguration -GatewayWsUrl 'wss://gateway.numeron.game/ws'",
    "$packaged = New-CandidateClientConfiguration -GatewayWsUrl 'wss://gateway.numeron.game/ws' -QuestGuidance $packageDefault",
    "$pattern = '(?m)^quest_guidance\\s*=\\s*\"([^\"]+)\"\\s*$'",
    "@{ native = [regex]::Match($native, $pattern).Groups[1].Value; packaged = [regex]::Match($packaged, $pattern).Groups[1].Value; nativeContent = (Resolve-CandidateContentProfile); packagedContent = (Resolve-CandidateContentProfile -Value $contentDefault) } | ConvertTo-Json -Compress",
  ].join("\n");
  const executable = process.platform === "win32" ? "powershell.exe" : "pwsh";
  const values = JSON.parse(execFileSync(executable, ["-NoProfile", "-NonInteractive", "-Command", command], { encoding: "utf8", timeout: 20_000 }).trim());
  assert.equal(values.native, baseline.questGuidance);
  assert.equal(values.packaged, baseline.questGuidance);
  assert.equal(values.nativeContent, baseline.contentProfile);
  assert.equal(values.packagedContent, baseline.contentProfile);
  const packageSource = read("../../game-client/platform-windows/scripts/package-windows-candidate.ps1");
  assert.match(packageSource, /\$ContentProfile = Resolve-CandidateContentProfile -Value \$ContentProfile/);
  assert.match(packageSource, /\$QuestGuidance = Resolve-CandidateQuestGuidance -Value \$QuestGuidance/);
});

test("Native explicit historical profiles remain selectable and unknown profile values are rejected", () => {
  const script = fileURLToPath(new URL("../../game-client/platform-windows/scripts/candidate-release-profile.ps1", import.meta.url));
  const command = [
    "$ErrorActionPreference = 'Stop'",
    `. '${script.replaceAll("'", "''")}'`,
    "$profiles = @('crystal', 'newcomer-v1', 'newcomer-v2' | ForEach-Object { (Assert-CandidateClientConfiguration -Text (New-CandidateClientConfiguration -GatewayWsUrl 'wss://gateway.numeron.game/ws' -QuestGuidance $_)).questGuidance })",
    "$content = @('platinum_176', 'platinum_fiveclass' | ForEach-Object { Resolve-CandidateContentProfile -Value $_ })",
    "$rejected = 0",
    "foreach ($bad in @(' ', 'typo', 'NEWCOMER-V2', 'newcomer-v2../crystal')) { try { Resolve-CandidateQuestGuidance -Value $bad | Out-Null } catch { $rejected++ } }",
    "foreach ($bad in @(' ', 'typo', 'PLATINUM_FIVECLASS', '../platinum_176')) { try { Resolve-CandidateContentProfile -Value $bad | Out-Null } catch { $rejected++ } }",
    "$nullGuidance = Resolve-CandidateQuestGuidance -Value $null",
    "$nullContent = Resolve-CandidateContentProfile -Value $null",
    "$callerCopy = Get-R16GameplayBaseline",
    "$callerCopy.questGuidance = 'crystal'",
    "$unchangedCanonical = Resolve-CandidateQuestGuidance",
    "@{ profiles = $profiles; content = $content; rejected = $rejected; nullGuidance = $nullGuidance; nullContent = $nullContent; unchangedCanonical = $unchangedCanonical } | ConvertTo-Json -Compress",
  ].join("\n");
  const executable = process.platform === "win32" ? "powershell.exe" : "pwsh";
  const values = JSON.parse(execFileSync(executable, ["-NoProfile", "-NonInteractive", "-Command", command], { encoding: "utf8", timeout: 20_000 }).trim());
  assert.deepEqual(values.profiles, ["crystal", "newcomer-v1", "newcomer-v2"]);
  assert.deepEqual(values.content, ["platinum_176", "platinum_fiveclass"]);
  assert.equal(values.rejected, 8);
  assert.equal(values.nullGuidance, baseline.questGuidance);
  assert.equal(values.nullContent, baseline.contentProfile);
  assert.equal(values.unchangedCanonical, baseline.questGuidance);
});

test("the Native host uses the canonical fallback and retains explicit TOML profile precedence", () => {
  const nativeMain = read("../../game-client/platform-windows/src/main.rs");
  const start = nativeMain.indexOf("let guidance = session.quest_guidance.as_deref().map_or_else(");
  assert(start >= 0, "Native explicit TOML profile must remain authoritative");
  const end = nativeMain.indexOf("app.insert_resource(", start);
  assert(end > start);
  const resolution = nativeMain.slice(start, end);
  assert.match(resolution, /r16_gameplay_baseline::r16_gameplay_baseline\(\)\.quest_guidance/);
  assert.equal((resolution.match(/QuestGuidance::from_profile_name/g) ?? []).length, 2);
  assert.doesNotMatch(resolution, /from_environment/);
  assert.match(nativeMain.slice(end, end + 250), /NewcomerJourneyCatalog::from_guidance\(&guidance\)/);
});
