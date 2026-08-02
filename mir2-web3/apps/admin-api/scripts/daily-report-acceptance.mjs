import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import http from "node:http";

const repoRoot = new URL("../../..", import.meta.url).pathname;
const adminPort = Number(process.env.MIR2_DAILY_REPORT_ACCEPTANCE_ADMIN_PORT ?? 17420);
const mockPort = Number(process.env.MIR2_DAILY_REPORT_ACCEPTANCE_MOCK_PORT ?? 19440);
const databaseUrl =
  process.env.ADMIN_DATABASE_URL ??
  "postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2";
const adminBase = `http://127.0.0.1:${adminPort}`;
const mockBase = `http://127.0.0.1:${mockPort}`;
const operatorHeaders = {
  "content-type": "application/json",
  "x-operator-id": "daily-report-acceptance",
  "x-operator-email": "daily-report@mir2.test",
  "x-operator-role": "ops_admin",
  "x-operator-permissions": "content_read,content_publish,audit_read",
};

let discordDeliveries = 0;
let modelRequests = 0;
const mock = http.createServer(async (request, response) => {
  const body = await readBody(request);
  if (request.url?.startsWith("/v1/chat/completions")) {
    modelRequests += 1;
    const parsed = JSON.parse(body);
    assert.equal(parsed.model, "daily-report-acceptance");
    const content = JSON.stringify({
      operationsMarkdown:
        "# 验收运营日报\n\n指标来自确定性聚合；这是模拟模型生成的受约束叙事。",
      playerMarkdown:
        "# 验收玛法世界报\n\n昨日的冒险足迹已经由世界事件流完成聚合并通过人工审核。"
    });
    return json(response, 200, { choices: [{ message: { content } }] });
  }
  if (request.url?.startsWith("/api/webhooks/acceptance/token")) {
    discordDeliveries += 1;
    const parsed = JSON.parse(body);
    assert.deepEqual(parsed.allowed_mentions, { parse: [] });
    assert.equal(parsed.embeds.length, 1);
    return json(response, 200, { id: `discord-acceptance-${discordDeliveries}` });
  }
  return json(response, 404, { error: "not found" });
});

await listen(mock, mockPort);
const child = spawn(
  "cargo",
  ["+1.89.0", "run", "--locked", "-p", "mir2-admin-api", "--bin", "mir2-admin-api"],
  {
    cwd: repoRoot,
    env: {
      ...process.env,
      ADMIN_API_ADDR: `127.0.0.1:${adminPort}`,
      ADMIN_DATABASE_URL: databaseUrl,
      ADMIN_CLICKHOUSE_URL: "http://127.0.0.1:1",
      ADMIN_CLICKHOUSE_USER: "mir2",
      ADMIN_CLICKHOUSE_PASSWORD: "acceptance-only",
      ADMIN_DAILY_REPORT_AI_ENDPOINT: `${mockBase}/v1/chat/completions`,
      ADMIN_DAILY_REPORT_AI_API_KEY: "acceptance-model-key",
      ADMIN_DAILY_REPORT_AI_MODEL: "daily-report-acceptance",
      ADMIN_DAILY_REPORT_DISCORD_WEBHOOK_URL: `${mockBase}/api/webhooks/acceptance/token`,
      ADMIN_DAILY_REPORT_DISCORD_DESTINATION_LABEL: "acceptance-channel",
      ADMIN_DAILY_REPORT_SCHEDULER_ENABLED: "false",
      ADMIN_DAILY_REPORT_TIMEZONE: "Asia/Shanghai",
      ADMIN_DAILY_REPORT_TIMEZONE_OFFSET_MINUTES: "480",
    },
    stdio: ["ignore", "pipe", "pipe"],
  },
);

let output = "";
child.stdout.on("data", (chunk) => {
  output += chunk;
});
child.stderr.on("data", (chunk) => {
  output += chunk;
});

try {
  await waitForHealth();
  const reportDate = completedShanghaiDate();
  let report = await post("/admin/daily-reports/generate", {
    reportDate,
    trigger: "acceptance",
    force: false,
  });
  assert.equal(report.reportDate, reportDate);
  assert.match(report.inputSha256, /^[a-f0-9]{64}$/);
  assert.match(report.contentSha256, /^[a-f0-9]{64}$/);

  if (report.status === "draft") {
    assert.equal(report.generationSource, "ai");
    report = await post(`/admin/daily-reports/${encodeURIComponent(report.reportId)}/approve`, {
      reason: "Acceptance reviewer verified aggregate evidence.",
    });
  }
  if (report.status === "approved") {
    const published = await post(
      `/admin/daily-reports/${encodeURIComponent(report.reportId)}/publish`,
      { reason: "Acceptance publication to isolated Discord mock." },
    );
    report = published.report;
  } else if (report.status === "published") {
    await post(`/admin/daily-reports/${encodeURIComponent(report.reportId)}/retry-discord`, {
      reason: "Acceptance retry against isolated Discord mock.",
    });
  }
  assert.equal(report.status, "published");

  const detail = await waitForDelivery(report.reportId);
  assert.equal(detail.deliveries[0]?.status, "delivered");
  assert.equal(detail.deliveries[0]?.destinationLabel, "acceptance-channel");
  assert.ok(discordDeliveries >= 1);

  const publicReport = await get("/public/daily-report/latest", false);
  assert.equal(publicReport.reportId, report.reportId);
  assert.equal(publicReport.playerMarkdown.includes("运营"), false);
  assert.equal("operationsMarkdown" in publicReport, false);
  assert.equal("evidence" in publicReport, false);

  const metrics = await fetch(`${adminBase}/metrics`).then((response) => response.text());
  assert.match(metrics, /mir2_daily_reports_configured 1/);
  assert.match(metrics, /mir2_daily_reports_published_total [1-9]/);

  console.log(
    JSON.stringify(
      {
        ok: true,
        reportId: report.reportId,
        reportDate,
        generationSource: report.generationSource,
        modelRequests,
        discordDeliveries,
        deliveryStatus: detail.deliveries[0]?.status,
        publicReportFields: Object.keys(publicReport).sort(),
        metricsVerified: true,
      },
      null,
      2,
    ),
  );
} catch (error) {
  process.stderr.write(`${output}\n`);
  throw error;
} finally {
  child.kill("SIGTERM");
  await Promise.race([
    new Promise((resolve) => child.once("exit", resolve)),
    new Promise((resolve) => setTimeout(resolve, 2_000)),
  ]);
  mock.close();
}

async function waitForHealth() {
  for (let attempt = 0; attempt < 120; attempt += 1) {
    try {
      const response = await fetch(`${adminBase}/health`);
      if (response.ok) return;
    } catch {
      // The Rust process is still compiling or binding.
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error("Admin API did not become healthy");
}

async function waitForDelivery(reportId) {
  for (let attempt = 0; attempt < 40; attempt += 1) {
    const detail = await get(`/admin/daily-reports/${encodeURIComponent(reportId)}`);
    if (detail.deliveries[0]?.status === "delivered") return detail;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("Discord delivery did not reach delivered state");
}

async function get(path, authenticated = true) {
  const response = await fetch(`${adminBase}${path}`, {
    headers: authenticated ? operatorHeaders : undefined,
  });
  const data = await response.json();
  assert.equal(response.ok, true, `${path}: ${JSON.stringify(data)}`);
  return data;
}

async function post(path, body) {
  const response = await fetch(`${adminBase}${path}`, {
    method: "POST",
    headers: operatorHeaders,
    body: JSON.stringify(body),
  });
  const data = await response.json();
  assert.equal(response.ok, true, `${path}: ${JSON.stringify(data)}`);
  return data;
}

function completedShanghaiDate() {
  const parts = new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Shanghai",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).formatToParts(Date.now() - 24 * 60 * 60 * 1_000);
  const value = Object.fromEntries(parts.map((part) => [part.type, part.value]));
  return `${value.year}-${value.month}-${value.day}`;
}

function listen(server, port) {
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", resolve);
  });
}

function readBody(request) {
  return new Promise((resolve, reject) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => resolve(body));
    request.on("error", reject);
  });
}

function json(response, status, value) {
  response.writeHead(status, { "content-type": "application/json" });
  response.end(JSON.stringify(value));
}
