const PRODUCTION_GAME_URL = "https://mir2.obelisk.build";

function resolveAllowedNavigationHost(environment = process.env) {
  const value = environment.MIR2_MOBILE_GAME_URL ?? PRODUCTION_GAME_URL;
  let parsed;
  try {
    parsed = new URL(value);
  } catch (error) {
    throw new Error(`MIR2_MOBILE_GAME_URL is not a valid URL: ${error.message}`);
  }

  if (parsed.protocol !== "https:" || !parsed.hostname || parsed.username || parsed.password) {
    throw new Error("MIR2_MOBILE_GAME_URL must use https: without credentials");
  }

  return parsed.hostname;
}

function createCapacitorConfig(environment = process.env) {
  return {
    appId: "com.obelisklabs.mir2",
    appName: "Mir2",
    webDir: "www",
    server: {
      androidScheme: "https",
      iosScheme: "capacitor",
      // Capacitor otherwise hands the loader's top-level remote navigation to
      // the system browser. Keep the in-app WebView limited to the configured
      // first-party HTTPS game host.
      allowNavigation: [resolveAllowedNavigationHost(environment)],
    },
    android: {},
    ios: {
      contentInset: "automatic",
    },
  };
}

module.exports = {
  PRODUCTION_GAME_URL,
  createCapacitorConfig,
  resolveAllowedNavigationHost,
};
