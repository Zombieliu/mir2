import net from "node:net";

const LOOPBACK_HOST = "127.0.0.1";

export async function selectChromeDebugPort(requestedPort = null) {
  if (requestedPort !== null && requestedPort !== undefined && requestedPort !== "") {
    const port = Number(requestedPort);
    if (!Number.isInteger(port) || port < 1 || port > 65_535) {
      throw new Error(`Invalid Chrome debug port: ${requestedPort}`);
    }
    await probeAvailablePort(port);
    return port;
  }

  // Port 0 lets Chrome choose an ephemeral port and record it in its own
  // DevToolsActivePort file, avoiding the probe-to-spawn race entirely.
  return 0;
}

function probeAvailablePort(port) {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.once("error", (error) => {
      if (error?.code === "EADDRINUSE") {
        reject(new Error(`Chrome debug port ${port} is already in use.`));
        return;
      }
      reject(error);
    });
    server.listen({ host: LOOPBACK_HOST, port, exclusive: true }, () => {
      const address = server.address();
      const selectedPort = typeof address === "object" && address ? address.port : null;
      server.close((error) => {
        if (error) {
          reject(error);
        } else if (!Number.isInteger(selectedPort)) {
          reject(new Error("Failed to allocate a Chrome debug port."));
        } else {
          resolve(selectedPort);
        }
      });
    });
  });
}
