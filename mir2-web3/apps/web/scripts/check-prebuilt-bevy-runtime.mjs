#!/usr/bin/env node
import { localPrebuiltRuntimeReady } from "./fetch-prebuilt-bevy-runtime.mjs";

try {
  if (!(await localPrebuiltRuntimeReady())) {
    console.error("Pinned Bevy runtime is incomplete or has different bytes");
    process.exitCode = 1;
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
