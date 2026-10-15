export type BevyRuntimePackageId = "webgpu" | "webgl2" | "webgl2-shared";
export type BevyRuntimeBackend = "webgpu" | "webgl2";
export type NormalizedBevyRuntimePackage = Readonly<{
  id: BevyRuntimePackageId;
  backend: BevyRuntimeBackend;
  packageDir: "pkg-webgpu" | "pkg-webgl2" | "pkg-webgl2-shared";
  questUiAbiVersion: 0 | 1 | null;
  bagUiAbiVersion: 0 | 1 | null;
  primarySharedUiCompiled: boolean | null;
}>;
export type NormalizedBevyRuntimeManifest = Readonly<{
  schemaVersion: 1 | 2;
  version: string;
  packages: readonly NormalizedBevyRuntimePackage[];
  files: readonly Readonly<{ path: string; sha256: string }>[];
}>;
export function validateBevyRuntimeManifest(manifest: unknown, label?: string): NormalizedBevyRuntimeManifest;
