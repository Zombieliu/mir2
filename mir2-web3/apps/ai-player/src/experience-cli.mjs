import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { ExperienceLedger, participantCsv } from "./experience/experience-ledger.mjs";
import { validateRuntimeService } from "./companion-runtime-service.mjs";

const APP_ROOT = fileURLToPath(new URL("../", import.meta.url));
export async function runExperienceCommand(args) {
  const remaining = [...args], command = remaining.shift(), options = {};
  if (!["inspect", "record", "report"].includes(command)) throw new Error("Use inspect, record or report.");
  while (remaining.length) {
    const flag = remaining.shift();
    if (!["--profile", "--input", "--output"].includes(flag) || !remaining.length || Object.hasOwn(options, flag.slice(2))) throw new Error("Invalid experience arguments.");
    options[flag.slice(2)] = remaining.shift();
  }
  if (!options.profile || command === "record" && !options.input || command === "report" && !options.output) throw new Error("Provide profile/input/output.");
  const profile = JSON.parse(await readFile(resolve(options.profile), "utf8")), service = validateRuntimeService(profile), experiment = service?.experiment;
  if (!experiment) throw new Error("Configure an experiment ledger first.");
  const ledger = new ExperienceLedger({ path: resolve(APP_ROOT, experiment.ledgerPath ?? ".data/experience.sqlite"), experimentId: experiment.experimentId,
    realmId: profile.realmId, worldEpoch: profile.companion.worldEpoch, currency: experiment.currency ?? "CNY", environment: experiment.environment ?? "trial", offer: experiment.offer ?? null });
  try {
    const ownerRef = profile.companion.ownerRef, archiveId = profile.companion.archiveId;
    ledger.registerParticipant(ownerRef, experiment.participantRef);
    if (command === "record") {
      const input = JSON.parse(await readFile(resolve(options.input), "utf8"));
      if (!input || Object.keys(input).some(key => !["operationId", "kind", "payload"].includes(key))) throw new Error("Unsupported operator record.");
      ledger.recordOperator({ source: "local-operator", operationId: input.operationId, ownerRef, archiveId, kind: input.kind, payload: input.payload });
      return { type: "experienceOperatorRecordSaved", participantRef: experiment.participantRef, kind: input.kind, environment: experiment.environment ?? "trial" };
    }
    if (command === "report") {
      const rows = ledger.participantRows();
      await writeFile(resolve(options.output), `\uFEFF${participantCsv(rows)}`, { flag: "wx", mode: 0o600 });
      return { type: "experienceParticipantReportSaved", participantCount: rows.length, environment: experiment.environment ?? "trial" };
    }
    return ledger.ownerView(ownerRef, archiveId);
  } finally { ledger.close(); }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runExperienceCommand(process.argv.slice(2)).then(value => console.log(JSON.stringify(value))).catch(() => {
    console.error("Experience operation stopped: check experiment/participant binding, fixed offer, exact operator record, currency and output path. No payment was performed.");
    process.exitCode = 1;
  });
}
