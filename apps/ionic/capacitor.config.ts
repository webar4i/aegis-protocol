import type { CapacitorConfig } from "@capacitor/cli";
const config: CapacitorConfig = {
  appId: "dev.aegisprotocol.client",
  appName: "Aegis",
  webDir: "dist",
  server: { androidScheme: "https" },
  android: { allowMixedContent: false },
};
export default config;
