import { Capacitor, registerPlugin, WebPlugin } from "@capacitor/core";

export type VpnState =
  | "unsupported"
  | "disconnected"
  | "connecting"
  | "connected"
  | "disconnecting"
  | "error";
export interface VpnCapabilities {
  available: boolean;
  platform: string;
  killSwitch: boolean;
  splitTunnel: boolean;
  reason?: string;
}
export interface VpnStatus {
  state: VpnState;
  /** Must be true only after the native OS tunnel is active. */
  tunnelActive: boolean;
  serverId?: string;
  receivedBytes: number | null;
  sentBytes: number | null;
  errorCode?: string;
}
export interface ConnectOptions {
  /** Opaque ID of a configuration in the native secure credential store. */
  profileId: string;
  serverId: string;
}
export interface AegisVpnPlugin {
  getCapabilities(): Promise<VpnCapabilities>;
  getStatus(): Promise<VpnStatus>;
  connect(options: ConnectOptions): Promise<VpnStatus>;
  disconnect(): Promise<VpnStatus>;
}

export const unavailableStatus = (): VpnStatus => ({
  state: "unsupported",
  tunnelActive: false,
  receivedBytes: null,
  sentBytes: null,
});

export class VpnUnavailableError extends Error {
  readonly code = "VPN_MODULE_UNAVAILABLE";
  constructor() {
    super("VPN-модуль ещё не подключён");
    this.name = "VpnUnavailableError";
  }
}

export class AegisVpnWeb extends WebPlugin implements AegisVpnPlugin {
  async getCapabilities(): Promise<VpnCapabilities> {
    return {
      available: false,
      platform: "web",
      killSwitch: false,
      splitTunnel: false,
      reason: "VPN-модуль ещё не подключён",
    };
  }
  async getStatus(): Promise<VpnStatus> {
    return unavailableStatus();
  }
  async connect(_options: ConnectOptions): Promise<VpnStatus> {
    throw new VpnUnavailableError();
  }
  async disconnect(): Promise<VpnStatus> {
    return unavailableStatus();
  }
}

const native = registerPlugin<AegisVpnPlugin>("AegisVpn", {
  web: () => Promise.resolve(new AegisVpnWeb()),
});

/** Capacitor shells have no native VPN implementation yet; never assume one. */
export function hasNativeVpn(): boolean {
  return (
    Capacitor.isNativePlatform() && Capacitor.isPluginAvailable("AegisVpn")
  );
}

export function createVpnBridge(): AegisVpnPlugin {
  return hasNativeVpn() ? native : new AegisVpnWeb();
}

/** UI may claim protection only from an affirmative, active native status. */
export function isProtected(status: VpnStatus): boolean {
  return status.state === "connected" && status.tunnelActive === true;
}

export function validateStatus(value: unknown): VpnStatus {
  if (!value || typeof value !== "object")
    throw new Error("Invalid VPN status");
  const status = value as Record<string, unknown>;
  const states: VpnState[] = [
    "unsupported",
    "disconnected",
    "connecting",
    "connected",
    "disconnecting",
    "error",
  ];
  if (
    !states.includes(status.state as VpnState) ||
    typeof status.tunnelActive !== "boolean"
  ) {
    throw new Error("Invalid VPN status");
  }
  for (const field of ["receivedBytes", "sentBytes"]) {
    const counter = status[field];
    if (
      counter !== null &&
      (typeof counter !== "number" ||
        !Number.isSafeInteger(counter) ||
        counter < 0)
    ) {
      throw new Error("Invalid VPN counter");
    }
  }
  if (status.state === "connected" && status.tunnelActive !== true) {
    throw new Error("Connected status requires an active tunnel");
  }
  if (status.state !== "connected" && status.tunnelActive === true) {
    throw new Error("Active tunnel requires connected status");
  }
  return value as VpnStatus;
}
