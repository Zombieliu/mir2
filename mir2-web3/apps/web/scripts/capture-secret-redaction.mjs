const SECRET_KEY_PATTERN = /(?:password|passkey|secret|token)/i;

export function redactCaptureSecrets(value) {
  if (Array.isArray(value)) {
    return value.map(redactCaptureSecrets);
  }
  if (typeof value === "string") {
    return redactCaptureSecretString(value);
  }
  if (!value || typeof value !== "object") {
    return value;
  }

  return Object.fromEntries(
    Object.entries(value).map(([key, nested]) => [
      key,
      SECRET_KEY_PATTERN.test(key) ? "[redacted]" : redactCaptureSecrets(nested),
    ]),
  );
}

export function redactCommandArgs(commandArgs) {
  return commandArgs.map((arg, index) => {
    const text = String(arg);
    const previousArg = index > 0 ? String(commandArgs[index - 1]) : "";
    if (
      previousArg.startsWith("--") &&
      !previousArg.includes("=") &&
      SECRET_KEY_PATTERN.test(previousArg)
    ) {
      return "[redacted]";
    }
    const inlineFlag = /^(--([^=]+)=)(.*)$/.exec(text);
    if (inlineFlag && SECRET_KEY_PATTERN.test(inlineFlag[2])) {
      return `${inlineFlag[1]}[redacted]`;
    }
    return redactCaptureSecrets(text);
  });
}

function redactCaptureSecretString(value) {
  const trimmed = value.trim();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    try {
      return JSON.stringify(redactCaptureSecrets(JSON.parse(trimmed)));
    } catch {
      // Fall through for truncated CDP payloads.
    }
  }

  return value
    .replace(
      /("(?:password|passkey|secret|token)[^"]*"\s*:\s*)"(?:\\.|[^"\\])*"/gi,
      '$1"[redacted]"',
    )
    .replace(/((?:password|passkey|secret|token)[^=\s]*=)[^&\s]+/gi, "$1[redacted]");
}
