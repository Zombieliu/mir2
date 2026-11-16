import { EnvHttpProxyAgent, setGlobalDispatcher } from "undici";

// Node 22's built-in fetch does not read proxy variables automatically.
// Keep all proxy configuration within this service process, and keep local
// Gateway requests direct even when the workstation has an outbound proxy.
if (["http_proxy", "https_proxy", "HTTP_PROXY", "HTTPS_PROXY"].some((name) => process.env[name]?.trim())) {
  const inheritedNoProxy = process.env.no_proxy ?? process.env.NO_PROXY ?? "";
  setGlobalDispatcher(new EnvHttpProxyAgent({
    noProxy: [inheritedNoProxy, "localhost", "127.0.0.1", "::1"].filter(Boolean).join(","),
  }));
}

// Only the Next CLI parent reads its private redirected stdin. Its existing
// SIGTERM handler owns worker cleanup; no unrelated process is addressed.
if (process.env.MIR2_NATIVE_WALLET_TEST_STDIN_SHUTDOWN === "1" &&
    process.env.NEXT_PRIVATE_WORKER !== "1" &&
    process.argv[1]?.replaceAll("\\", "/").endsWith("/next/dist/bin/next")) {
  let pending = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => {
    pending = (pending + chunk).slice(-64);
    if (pending.split(/\r?\n/).some((line) => line === "shutdown")) {
      process.stdin.pause();
      process.emit("SIGTERM");
    }
  });
}
