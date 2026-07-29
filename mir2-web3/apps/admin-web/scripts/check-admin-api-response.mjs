import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const sourceUrl = new URL("../lib/admin-api-response.ts", import.meta.url);
const source = readFileSync(sourceUrl, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: sourceUrl.pathname,
  reportDiagnostics: true,
});
const errors = (compiled.diagnostics ?? []).filter(
  (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
);
assert.deepEqual(errors, []);
const loaded = { exports: {} };
new Function("exports", "module", compiled.outputText)(loaded.exports, loaded);
const { parseAdminApiResponse } = loaded.exports;

function response(status, body, contentType = "application/json", statusText = "") {
  return {
    status,
    statusText,
    headers: {
      get(name) {
        return name.toLowerCase() === "content-type" ? contentType : null;
      },
    },
    async text() {
      return body;
    },
  };
}

test("parses a valid JSON response", async () => {
  assert.deepEqual(
    await parseAdminApiResponse(response(200, '{"ok":true}')),
    { ok: true, data: { ok: true } },
  );
});

test("reports an empty upstream response without leaking a JSON parser error", async () => {
  assert.deepEqual(
    await parseAdminApiResponse(response(404, "", "", "Not Found")),
    {
      ok: false,
      error: "Admin API HTTP 404 Not Found returned an empty response",
    },
  );
});

test("reports a bounded preview for a non-JSON proxy response", async () => {
  const result = await parseAdminApiResponse(
    response(502, "<html>upstream unavailable</html>", "text/html", "Bad Gateway"),
  );
  assert.equal(result.ok, false);
  assert.match(result.error, /HTTP 502 Bad Gateway returned invalid JSON/);
  assert.match(result.error, /upstream unavailable/);
});
