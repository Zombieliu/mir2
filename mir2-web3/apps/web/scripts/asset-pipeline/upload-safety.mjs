export function normalizeWorkerUploadPath(value) {
  const uploadPath = String(value || "/upload").trim();
  // `new URL("//host/path", base)` switches origins. Keep the configurable
  // uploader endpoint to one normalized, same-origin path so its bearer secret
  // can never be forwarded to a host embedded in configuration.
  if (
    !/^\/[A-Za-z0-9][A-Za-z0-9/_-]*$/.test(uploadPath) ||
    uploadPath.includes("//")
  ) {
    throw new Error(`Unsafe R2 upload Worker path: ${value}`);
  }
  return uploadPath;
}
