import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, writeFileSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import ts from "typescript";

const require = createRequire(import.meta.url);
const { getPathMatch } = require("next/dist/shared/lib/router/utils/path-match");
const { prepareDestination } = require("next/dist/shared/lib/router/utils/prepare-destination");
const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = path.join(webRoot, "next.config.ts");
const temporaryModule = path.join(webRoot, `.marketing-routing-${process.pid}-${Date.now()}.mjs`);
const compiled = ts.transpileModule(readFileSync(sourcePath, "utf8"), {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
  fileName: sourcePath,
  reportDiagnostics: true,
});
assert.deepEqual((compiled.diagnostics ?? []).filter((value) => value.category === ts.DiagnosticCategory.Error), []);
writeFileSync(temporaryModule, compiled.outputText);
let sequence = 0;

async function loadConfig(origin, hosted = false, standalone = false) {
  const previous = { MIR2_MARKETING_ORIGIN: process.env.MIR2_MARKETING_ORIGIN, MIR2_NEXT_STANDALONE: process.env.MIR2_NEXT_STANDALONE, VERCEL: process.env.VERCEL, VERCEL_ENV: process.env.VERCEL_ENV };
  if (origin === undefined) delete process.env.MIR2_MARKETING_ORIGIN;
  else process.env.MIR2_MARKETING_ORIGIN = origin;
  process.env.VERCEL = hosted ? "1" : "0";
  process.env.VERCEL_ENV = "preview";
  process.env.MIR2_NEXT_STANDALONE = standalone ? "1" : "0";
  try {
    return (await import(`${pathToFileURL(temporaryModule).href}?case=${sequence++}`)).default;
  } finally {
    for (const [key, value] of Object.entries(previous)) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
  }
}

function rewrite(rules, pathname, query = {}) {
  for (const rule of rules) {
    const params = getPathMatch(rule.source)(pathname);
    if (params) return prepareDestination({ destination: rule.destination, params, query, appendParamsToQuery: true }).parsedDestination;
  }
  return null;
}

function allRewriteDestinations(rules, pathname, query = {}) {
  return rules.flatMap((rule) => {
    const params = getPathMatch(rule.source)(pathname);
    return params ? [prepareDestination({ destination: rule.destination, params, query, appendParamsToQuery: true }).parsedDestination] : [];
  });
}

try {
  await test("marketing routes cover all supported pages and public assets without sharing game chunks", async () => {
    const config = await loadConfig();
    const routes = await config.rewrites();
    const expectedHost = "numeron-legend-of-rebirth.vercel.app";
    assert.equal(rewrite(routes.beforeFiles, "/").pathname, "/zh-CN");
    assert.equal(rewrite(routes.beforeFiles, "/", { utm_source: "invited", ref: "public-code" }).query.utm_source, "invited");
    const locales = ["zh-CN", "en", "ja", "ko", "zh-TW", "es", "pt-BR", "de", "fr", "ru", "pl", "tr", "th", "vi", "id", "ar"];
    for (const locale of locales) {
      for (const suffix of ["", "/", "/download", "/play", "/membership", "/legal/privacy", "/updates/world-launch", "/explore", "/archive/map/bichon-province"]) {
        const destination = rewrite(routes.beforeFiles, `/${locale}${suffix}`);
        assert.equal(destination?.hostname, expectedHost, `${locale}${suffix}`);
        assert.equal(destination.pathname.replace(/\/$/, ""), `/${locale}${suffix}`.replace(/\/$/, ""));
      }
    }
    for (const pathname of ["/gameplay/character-select.png", "/gameplay/shared-world.png", "/gameplay/skills-and-inventory.png", "/og.png", "/explore/en/archive/world/world-01", "/api/attribution", "/api/founder-interest", "/api/game-status", "/robots.txt", "/sitemap.xml", "/icon"]) {
      const destination = rewrite(routes.beforeFiles, pathname);
      assert.equal(destination?.hostname, expectedHost, pathname);
      assert.equal(destination.pathname, pathname);
    }
    const chunk = rewrite(routes.beforeFiles, "/marketing-assets/_next/static/chunks/site.js");
    assert.equal(chunk.hostname, expectedHost);
    assert.equal(chunk.pathname, "/marketing-assets/_next/static/chunks/site.js");
    const chunkWithQuery = rewrite(routes.beforeFiles, "/marketing-assets/_next/static/chunks/site.js", { dpl: "dpl_marketing_release", version: "42", lang: "zh-CN" });
    assert.equal(chunkWithQuery.hostname, expectedHost);
    assert.equal(chunkWithQuery.pathname, "/marketing-assets/_next/static/chunks/site.js");
    assert.deepEqual(chunkWithQuery.query, { dpl: "dpl_marketing_release", version: "42", lang: "zh-CN" });
  });

  await test("localized homepages match exactly one marketing rule without adding a trailing slash", async () => {
    const routes = await (await loadConfig()).rewrites();
    const locales = ["zh-CN", "en", "ja", "ko", "zh-TW", "es", "pt-BR", "de", "fr", "ru", "pl", "tr", "th", "vi", "id", "ar"];
    for (const locale of locales) {
      for (const suffix of ["", "/"]) {
        for (const query of [{}, { utm_source: "partner", ref: "invitation-code", lang: locale }]) {
          const pathname = `/${locale}${suffix}`;
          const destinations = allRewriteDestinations(routes.beforeFiles, pathname, query).filter((destination) => destination.hostname);
          assert.equal(destinations.length, 1, `${pathname} must not be shadowed by a zero-segment child rule`);
          assert.equal(destinations[0].hostname, "numeron-legend-of-rebirth.vercel.app");
          assert.equal(destinations[0].pathname, `/${locale}`);
          assert.deepEqual(destinations[0].query, query);
        }
      }
    }
  });

  await test("localized child pages match one marketing rule and preserve nested paths and query parameters", async () => {
    const routes = await (await loadConfig()).rewrites();
    for (const locale of ["zh-CN", "en", "ja", "ko", "zh-TW", "es", "pt-BR", "de", "fr", "ru", "pl", "tr", "th", "vi", "id", "ar"]) {
      for (const suffix of ["/download", "/play", "/legal/privacy", "/updates/world-launch", "/explore", "/archive/map/bichon-province"]) {
        const pathname = `/${locale}${suffix}`;
        const query = { utm_source: "partner", ref: "invitation-code", dpl: "dpl_marketing_release", lang: locale };
        const destinations = allRewriteDestinations(routes.beforeFiles, pathname, query).filter((destination) => destination.hostname);
        assert.equal(destinations.length, 1, pathname);
        assert.equal(destinations[0].hostname, "numeron-legend-of-rebirth.vercel.app");
        assert.equal(destinations[0].pathname, pathname);
        assert.deepEqual(destinations[0].query, query);
      }
    }
  });

  await test("marketing cannot capture game, promotion, authentication, metadata or neighboring APIs", async () => {
    const routes = await (await loadConfig()).rewrites();
    for (const pathname of ["/game", "/game/", "/game/nested", "/promotions", "/promotions/play", "/promotions/invite/test", "/api/promotions/bootstrap", "/api/promotions/auth/challenge", "/api/passkey/challenge", "/api/identity/sessions", "/api/channels/session/exchange", "/api/asset-manifest", "/api/scene/crystal", "/api/attribution/private", "/api/game-status/private", "/api/founder-interest/private", "/_next/static/chunks/game.js", "/marketing-assets/_next/image", "/manifest.webmanifest", "/mir2-asset-worker.js", "/pwa/icon-192.png", "/favicon.ico", "/favicon.svg", "/original-ui/Title/30.png", "/original-map/WemadeMir2/Tiles/1000.png", "/health", "/version", "/spectate", "/ai-live", "/world-report", "/enough", "/zh-CN-extra"]) {
      assert.equal(rewrite(routes.beforeFiles, pathname), null, pathname);
    }
  });

  await test("existing immutable runtime and same-origin asset fallback routes remain intact", async () => {
    const routes = await (await loadConfig()).rewrites();
    const runtime = rewrite(routes.beforeFiles, "/bevy-runtime/v/build-123/pkg-webgl2/mir2_bevy_runtime.js");
    assert.equal(runtime.pathname, "/bevy-runtime/pkg-webgl2/mir2_bevy_runtime.js");
    assert.ok(!runtime.hostname);
    for (const prefix of ["original-map", "original-ui", "generated", "bevy-entity-atlases", "Sound"]) {
      const destination = rewrite(routes.fallback, `/${prefix}/folder/asset.png`);
      assert.equal(destination.pathname, `/api/r2-proxy/${prefix}/folder/asset.png`);
      assert.ok(!destination.hostname);
    }
  });

  await test("a configured deployment or local development origin changes only marketing destinations", async () => {
    for (const origin of ["https://marketing-release.example.test/", "http://127.0.0.1:7444", "http://localhost:7444"]) {
      const routes = await (await loadConfig(origin)).rewrites();
      const destination = rewrite(routes.beforeFiles, "/en/download");
      assert.equal(destination.hostname, new URL(origin).hostname);
      assert.equal(destination.protocol, new URL(origin).protocol);
      assert.equal(destination.pathname, "/en/download");
      assert.equal(rewrite(routes.beforeFiles, "/game"), null);
    }
  });

  await test("standalone game bundles retain their root game and all original asset routes", async () => {
    const config = await loadConfig("https://mir2.obelisk.build", false, true);
    assert.equal(config.output, "standalone");
    const routes = await config.rewrites();
    assert.equal(routes.beforeFiles.length, 1);
    for (const pathname of ["/", "/game", "/en", "/zh-CN/download", "/marketing-assets/_next/static/chunks/site.js", "/gameplay/shared-world.png", "/api/attribution", "/api/game-status", "/robots.txt", "/icon", "/promotions"]) {
      assert.equal(rewrite(routes.beforeFiles, pathname), null, pathname);
    }
    assert.equal(rewrite(routes.beforeFiles, "/bevy-runtime/v/build-123/pkg-webgl2/runtime.js").pathname, "/bevy-runtime/pkg-webgl2/runtime.js");
    assert.equal(rewrite(routes.fallback, "/original-map/region/tile.png").pathname, "/api/r2-proxy/original-map/region/tile.png");
  });

  await test("invalid, looping or hosted loopback marketing destinations fail configuration", async () => {
    for (const origin of ["https://user:secret@marketing.example.test", "https://marketing.example.test/nested", "https://marketing.example.test?override=1", "https://marketing.example.test#fragment", "http://marketing.example.test", "http://[::1]:7444", "https://[::1]", "file:///tmp/site", "https://mir2.obelisk.build"]) {
      await assert.rejects(loadConfig(origin), /MIR2_MARKETING_ORIGIN/);
    }
    await assert.rejects(loadConfig("http://127.0.0.1:7444", true), /MIR2_MARKETING_ORIGIN/);
  });
} finally {
  rmSync(temporaryModule, { force: true });
}
