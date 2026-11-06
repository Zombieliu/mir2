export function parseWindowsGatewayProcessSample(output) {
  const values = String(output).trim().split(",").map((value) => Number(value));
  if (values.length !== 6 || !Number.isSafeInteger(values[0]) || values[0] <= 0) return null;
  const [pid, workingSetBytes, privateBytes, handleCount, threadCount, cpuTimeMs] = values;
  if (![workingSetBytes, privateBytes, handleCount, threadCount, cpuTimeMs].every(Number.isFinite)) {
    return null;
  }
  return {
    atUnixMs: null,
    pid,
    workingSetBytes,
    privateBytes,
    handleCount,
    threadCount,
    cpuTimeMs,
    cpuPercent: null,
  };
}

export function parseListenBindings(output) {
  return String(output)
    .split(/\r?\n/)
    .map((value) => value.trim())
    .filter(Boolean)
    .map((value) => {
      const [localAddress, rawPid] = value.split("|");
      const pid = Number(rawPid);
      return { localAddress, pid };
    })
    .filter((value) => value.localAddress && Number.isSafeInteger(value.pid) && value.pid > 0);
}

export function parseWindowsGatewayIdentity(output) {
  try {
    const value = JSON.parse(String(output).trim());
    if (!Number.isSafeInteger(value.pid) || value.pid <= 0) return null;
    if (typeof value.path !== "string" || value.path.trim() === "") return null;
    if (!Number.isSafeInteger(value.bytes) || value.bytes <= 0) return null;
    if (!/^[a-fA-F0-9]{64}$/.test(value.sha256)) return null;
    return {
      pid: value.pid,
      path: value.path,
      bytes: value.bytes,
      sha256: value.sha256.toUpperCase(),
    };
  } catch {
    return null;
  }
}
