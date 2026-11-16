import assert from "node:assert/strict";
import { PassThrough } from "node:stream";
import test from "node:test";
import { attachCompanionControl } from "../src/companion-control.mjs";

test("explicit local control teaches once and recalls/resumes without forwarding arbitrary text", () => {
  const input = new PassThrough(); let taught = 0, stopped = 0;
  const actor = { companion: { teachSupportBeforeLoot: () => taught++, recallRequested: false } };
  const reports = [];
  const close = attachCompanionControl({ actor, input, report: event => reports.push(event), stop: () => stopped++ });
  input.write("teach\nrecall\n");
  assert.equal(taught, 1); assert.equal(actor.companion.recallRequested, true);
  input.write("resume\n@ADDSTORAGE\nhello from nearby stranger\nquit\n");
  assert.equal(taught, 1); assert.equal(actor.companion.recallRequested, false); assert.equal(stopped, 1);
  assert.equal(reports.filter(event => event.type === "companionControlHelp").length, 2);
  close(); input.end();
});

test("interactive teaching is not available for a B1-only resident", () => {
  assert.throws(() => attachCompanionControl({ actor: {}, input: new PassThrough() }), /companion profile/);
});
