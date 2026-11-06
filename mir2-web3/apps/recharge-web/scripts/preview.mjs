import http from "node:http";
import {readFile, realpath} from "node:fs/promises";
import {fileURLToPath} from "node:url";
import path from "node:path";
const root = await realpath(fileURLToPath(new URL("../public/", import.meta.url)));
const types = {".html":"text/html; charset=utf-8",".css":"text/css; charset=utf-8",".js":"text/javascript; charset=utf-8",".svg":"image/svg+xml"};
const server = http.createServer(async (req, res) => {
  if (!["GET","HEAD"].includes(req.method)) { res.writeHead(405); res.end(); return; }
  try {
    const url = new URL(req.url, "http://127.0.0.1:7347");
    const relative = decodeURIComponent(url.pathname).replace(/^\/+/, "") || "index.html";
    const file = await realpath(path.join(root, relative));
    if (!file.startsWith(root + path.sep)) throw new Error("outside preview");
    const body = await readFile(file);
    res.writeHead(200, {"Content-Type":types[path.extname(file)] || "application/octet-stream", "Cache-Control":"no-store", "X-Content-Type-Options":"nosniff"});
    res.end(req.method === "HEAD" ? undefined : body);
  } catch { res.writeHead(404); res.end("Not found"); }
});
server.listen(7347, "127.0.0.1", () => process.stdout.write("Local: http://127.0.0.1:7347/\n"));
process.on("SIGINT", () => server.close());
