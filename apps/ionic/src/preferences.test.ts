import { describe, expect, it } from "vitest";
import { loadPreferences, savePreferences } from "./preferences";
describe("non-secret preferences", () => {
  it("survives malformed or unavailable storage", () => {
    expect(loadPreferences({ getItem: () => "{bad" }).serverId).toBe("auto");
    expect(
      loadPreferences({ getItem: () => '{"serverId":"attacker"}' }).serverId,
    ).toBe("auto");
    expect(() =>
      savePreferences(
        {
          setItem: () => {
            throw new Error("disabled");
          },
        },
        { serverId: "nl", reduceMotion: true },
      ),
    ).not.toThrow();
  });
  it("persists only the approved display fields", () => {
    let stored = "";
    savePreferences(
      {
        setItem: (_key, value) => {
          stored = value;
        },
      },
      { serverId: "de", reduceMotion: true },
    );
    expect(JSON.parse(stored)).toEqual({ serverId: "de", reduceMotion: true });
  });
});
