import { describe, expect, it } from "vitest";
import {
  AegisVpnWeb,
  isProtected,
  unavailableStatus,
  validateStatus,
  VpnUnavailableError,
} from "./aegis";

describe("honest VPN bridge", () => {
  it("never reports protection in a browser or an unimplemented shell", async () => {
    const bridge = new AegisVpnWeb();
    expect((await bridge.getCapabilities()).available).toBe(false);
    expect(isProtected(await bridge.getStatus())).toBe(false);
    await expect(
      bridge.connect({ profileId: "test", serverId: "test" }),
    ).rejects.toBeInstanceOf(VpnUnavailableError);
    expect(isProtected(await bridge.disconnect())).toBe(false);
  });
  it("requires both connected and an active native tunnel", () => {
    expect(isProtected(unavailableStatus())).toBe(false);
    expect(isProtected({ ...unavailableStatus(), state: "connected" })).toBe(
      false,
    );
    expect(
      isProtected({
        ...unavailableStatus(),
        state: "connected",
        tunnelActive: true,
      }),
    ).toBe(true);
  });
  it("rejects inconsistent states and forged counters at the native boundary", () => {
    expect(() =>
      validateStatus({ ...unavailableStatus(), state: "connected" }),
    ).toThrow();
    expect(() =>
      validateStatus({ ...unavailableStatus(), tunnelActive: true }),
    ).toThrow();
    expect(() =>
      validateStatus({ ...unavailableStatus(), receivedBytes: -1 }),
    ).toThrow();
    expect(() =>
      validateStatus({ ...unavailableStatus(), sentBytes: Number.NaN }),
    ).toThrow();
    expect(() =>
      validateStatus({ ...unavailableStatus(), state: "mystery" }),
    ).toThrow();
    expect(validateStatus(unavailableStatus())).toEqual(unavailableStatus());
  });
});
