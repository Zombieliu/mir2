export function isVercelHostedWeb() {
  return (
    process.env.MIR2_WEB_HOSTING?.toLowerCase() === "vercel" ||
    process.env.VERCEL === "1" ||
    Boolean(process.env.VERCEL_ENV)
  );
}

export function requestFileWritesDisabled() {
  return isVercelHostedWeb() || process.env.MIR2_DISABLE_REQUEST_FILE_WRITES === "1";
}
