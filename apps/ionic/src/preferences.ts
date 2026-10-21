export type ServerId = "auto" | "nl" | "de" | "fi";
const KEY = "aegis.preferences.v1";
export const serverIds: ServerId[] = ["auto", "nl", "de", "fi"];
export interface Preferences {
  serverId: ServerId;
  reduceMotion: boolean;
}
export const defaults: Preferences = { serverId: "auto", reduceMotion: false };

/** Only non-secret display preferences belong in browser storage. */
export function loadPreferences(
  storage: Pick<Storage, "getItem">,
): Preferences {
  try {
    const value = JSON.parse(
      storage.getItem(KEY) ?? "{}",
    ) as Partial<Preferences>;
    return {
      serverId: serverIds.includes(value.serverId as ServerId)
        ? (value.serverId as ServerId)
        : "auto",
      reduceMotion: value.reduceMotion === true,
    };
  } catch {
    return { ...defaults };
  }
}
export function savePreferences(
  storage: Pick<Storage, "setItem">,
  value: Preferences,
): void {
  try {
    storage.setItem(
      KEY,
      JSON.stringify({
        serverId: value.serverId,
        reduceMotion: value.reduceMotion,
      }),
    );
  } catch {
    /* Private browsing may disable storage; this must not break the app. */
  }
}
