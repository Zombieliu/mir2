#!/usr/bin/env node
// Fetch Crystal `.map` source files from a Google Drive folder via the Drive API.
//
// In the Claude Code on the web execution environment, `drive.google.com` is not
// on the network allowlist but `www.googleapis.com` (the Drive REST API host) is.
// So with a short-lived OAuth2 access token (scope `drive.readonly`) we can pull
// the real `.map` layout files straight to disk — including Bichon's 0.map
// (~12.7MB), which is far too large to round-trip through an MCP tool response.
//
// The downloaded files feed `generate-crystal-map-runtime-data.mjs --mapsOnly`,
// which produces the committed `lib/generated/crystal-map-pack/*.map.gz` pack the
// runtime serves. (The sibling library-meta is already committed, so --mapsOnly
// is enough; no `.Lib` files are needed here.)
//
// Auth (pick one; never hard-code the token):
//   --token-file <path>   read the access token from a file (recommended)
//   GDRIVE_TOKEN=...       read the access token from the environment
//   --token <value>        pass it inline (shows up in shell history — avoid)
//
// Usage:
//   GDRIVE_TOKEN=ya29.xxx node scripts/fetch-crystal-maps-from-drive.mjs \
//     --out /tmp/crystal-map-src/Map
//   # only specific maps (comma-separated stems, no extension):
//   ... --only 0,0100,0101,0102,0103,0104,0105
//
import fs from "node:fs";
import fsp from "node:fs/promises";
import path from "node:path";
import { Readable } from "node:stream";

const API = "https://www.googleapis.com/drive/v3";
// Default: the "Map" folder that holds the real Crystal `.map` files on Drive.
const DEFAULT_FOLDER_ID = "1ZJDEQrh5mZt0pxpuU5JTuzFC-8SxTWRd";

const args = parseArgs(process.argv.slice(2));
const token = resolveToken(args);
const folderId = args.folder ?? DEFAULT_FOLDER_ID;
const outDir = path.resolve(args.out ?? "Map");
const only = args.only
  ? new Set(args.only.split(",").map((s) => s.trim().toLowerCase()).filter(Boolean))
  : null;

if (!token) {
  console.error(
    "Missing access token. Provide --token-file <path>, GDRIVE_TOKEN env, or --token <value>.",
  );
  process.exit(1);
}

main().catch((error) => {
  console.error(error?.message ?? error);
  process.exit(1);
});

async function main() {
  await fsp.mkdir(outDir, { recursive: true });
  const files = (await listFolder(folderId)).filter((f) => /\.map$/i.test(f.name));
  let selected = files;
  if (only) {
    selected = files.filter((f) => only.has(f.name.replace(/\.map$/i, "").toLowerCase()));
  }
  selected.sort((a, b) => a.name.localeCompare(b.name));

  console.log(
    `Folder ${folderId}: ${files.length} .map files; downloading ${selected.length} -> ${outDir}`,
  );

  let done = 0;
  let newBytes = 0;
  for (const file of selected) {
    const dest = path.join(outDir, file.name);
    const want = Number(file.size ?? 0);
    if (fs.existsSync(dest) && want > 0 && fs.statSync(dest).size === want) {
      console.log(`  skip ${file.name} (already ${want} bytes)`);
      done += 1;
      continue;
    }
    const got = await downloadToFile(file.id, dest);
    if (want > 0 && got !== want) {
      throw new Error(`size mismatch for ${file.name}: got ${got}, expected ${want}`);
    }
    console.log(`  ok   ${file.name} (${got} bytes)`);
    done += 1;
    newBytes += got;
  }
  console.log(`Done: ${done}/${selected.length} maps (${newBytes} new bytes) in ${outDir}`);
}

async function listFolder(folder) {
  const out = [];
  let pageToken;
  do {
    const q = encodeURIComponent(`'${folder}' in parents and trashed = false`);
    const url =
      `${API}/files?q=${q}` +
      `&fields=nextPageToken,files(id,name,size,mimeType)` +
      `&pageSize=1000&supportsAllDrives=true&includeItemsFromAllDrives=true` +
      (pageToken ? `&pageToken=${encodeURIComponent(pageToken)}` : "");
    const res = await fetch(url, { headers: { Authorization: `Bearer ${token}` } });
    if (!res.ok) {
      throw new Error(`Drive list failed (${res.status}): ${truncate(await res.text())}`);
    }
    const json = await res.json();
    out.push(...(json.files ?? []));
    pageToken = json.nextPageToken;
  } while (pageToken);
  return out;
}

async function downloadToFile(id, dest) {
  const url = `${API}/files/${encodeURIComponent(id)}?alt=media&supportsAllDrives=true`;
  const res = await fetch(url, { headers: { Authorization: `Bearer ${token}` } });
  if (!res.ok || !res.body) {
    throw new Error(`Drive download ${id} failed (${res.status}): ${truncate(await res.text())}`);
  }
  const tmp = `${dest}.part`;
  await new Promise((resolve, reject) => {
    const ws = fs.createWriteStream(tmp);
    const rs = Readable.fromWeb(res.body);
    rs.on("error", reject);
    ws.on("error", reject);
    ws.on("finish", resolve);
    rs.pipe(ws);
  });
  await fsp.rename(tmp, dest);
  return fs.statSync(dest).size;
}

function resolveToken(parsed) {
  if (parsed.tokenFile) {
    return fs.readFileSync(path.resolve(parsed.tokenFile), "utf8").trim();
  }
  return parsed.token ?? process.env.GDRIVE_TOKEN ?? "";
}

function truncate(text) {
  const s = String(text ?? "");
  return s.length > 300 ? `${s.slice(0, 300)}…` : s;
}

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === "--token") out.token = argv[(i += 1)];
    else if (a === "--token-file") out.tokenFile = argv[(i += 1)];
    else if (a === "--folder") out.folder = argv[(i += 1)];
    else if (a === "--out") out.out = argv[(i += 1)];
    else if (a === "--only") out.only = argv[(i += 1)];
    else throw new Error(`Unknown argument: ${a}`);
  }
  return out;
}
