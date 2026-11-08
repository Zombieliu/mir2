import { createHash } from "node:crypto";
import { createServer } from "node:net";

/** Same local archive always uses the same lock, regardless of runner/path. */
export async function acquireArchiveGuard(archiveId) {
  if (typeof archiveId !== "string" || !/^[A-Za-z0-9._:/@+-]{1,160}$/.test(archiveId)) throw new Error("A stable archive identity is required.");
  const digest = createHash("sha256").update(`numeron-companion-archive-lock/1:${archiveId}`).digest();
  const port = 20_000 + digest.readUInt32BE(0) % 20_000;
  const server = createServer(socket => socket.destroy());
  try {
    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen({ host: "127.0.0.1", port, exclusive: true }, resolve);
    });
  } catch {
    throw new Error("This archive is already running or its local archive lock is unavailable.");
  }
  return () => new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
}
