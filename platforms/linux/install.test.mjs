import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, writeFileSync, mkdirSync, rmSync, statSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { setTimeout } from "node:timers/promises";
import { install, uninstall, locations, desktopEntry } from "./install.mjs";

function fixture(callback) {
  const temp = mkdtempSync(join(tmpdir(), "aegis-install-"));
  try { return callback(temp); } finally { rmSync(temp, { recursive: true, force: true }); }
}

function sources(temp) {
  const binary = join(temp, "input");
  const icon = join(temp, "icon.svg");
  writeFileSync(binary, "first");
  writeFileSync(icon, "<svg/>");
  return { binary, icon };
}

test("install, update and repeated uninstall preserve unrelated data and permit reinstall", () => fixture((temp) => {
  const paths = locations(join(temp, "my home"), join(temp, "data space"));
  const { binary, icon } = sources(temp);
  install(binary, icon, paths);
  assert.equal(readFileSync(paths.binary, "utf8"), "first");
  assert.equal(statSync(paths.binary).mode & 0o777, 0o755);
  assert.equal(readlinkSync(paths.command), paths.binary);
  assert.match(readFileSync(paths.desktop, "utf8"), /^Exec=".*data space.*"$/m);
  assert.ok(paths.desktop.endsWith("/dev.aegisprotocol.client.desktop"));
  assert.doesNotMatch(readFileSync(paths.desktop, "utf8"), /^StartupWMClass=/m);
  writeFileSync(binary, "updated");
  install(binary, icon, paths);
  assert.equal(readFileSync(paths.binary, "utf8"), "updated");
  const other = join(paths.directory, "retain-me");
  writeFileSync(other, "data");
  uninstall(paths);
  uninstall(paths);
  assert.equal(readFileSync(other, "utf8"), "data");
  assert.throws(() => statSync(paths.binary), { code: "ENOENT" });
  assert.throws(() => statSync(paths.desktop), { code: "ENOENT" });
  install(binary, icon, paths);
  assert.equal(readFileSync(paths.binary, "utf8"), "updated");
  assert.equal(readFileSync(other, "utf8"), "data");
}));

test("uninstall removes an otherwise empty managed directory and is repeatable", () => fixture((temp) => {
  const paths = locations(temp, null);
  const { binary, icon } = sources(temp);
  install(binary, icon, paths);
  uninstall(paths);
  assert.equal(existsSync(paths.directory), false);
  uninstall(paths);
  install(binary, icon, paths);
  assert.equal(readFileSync(paths.binary, "utf8"), "first");
}));

test("refuses an unrelated command before changing installed files", () => fixture((temp) => {
  const paths = locations(temp, null);
  mkdirSync(join(temp, ".local/bin"), { recursive: true });
  writeFileSync(paths.command, "unrelated");
  assert.throws(() => install("missing", "missing", paths), /unrelated/);
  assert.equal(readFileSync(paths.command, "utf8"), "unrelated");
}));

test("a missing update icon leaves the previous binary, icon and launcher intact", () => fixture((temp) => {
  const paths = locations(temp, null);
  const { binary, icon } = sources(temp);
  install(binary, icon, paths);
  const launcher = readFileSync(paths.desktop, "utf8");
  writeFileSync(binary, "new version");
  assert.throws(() => install(binary, join(temp, "missing.svg"), paths), { code: "ENOENT" });
  assert.equal(readFileSync(paths.binary, "utf8"), "first");
  assert.equal(readFileSync(paths.icon, "utf8"), "<svg/>");
  assert.equal(readFileSync(paths.desktop, "utf8"), launcher);
  assert.deepEqual(readdirSync(paths.directory).sort(), [".aegis-managed", "aegis-client", "aegis-client.svg"]);
}));

test("refuses destination symlinks without overwriting their targets", () => {
  for (const target of ["binary", "icon", "ownership"]) fixture((temp) => {
    const paths = locations(temp, null);
    const { binary, icon } = sources(temp);
    install(binary, icon, paths);
    const outside = join(temp, "unrelated");
    writeFileSync(outside, "keep");
    const destination = target === "ownership" ? join(paths.directory, ".aegis-managed") : paths[target];
    rmSync(destination);
    symlinkSync(outside, destination);
    assert.throws(() => install(binary, icon, paths), /unrelated/);
    assert.throws(() => uninstall(paths), /unrelated/);
    assert.equal(readFileSync(outside, "utf8"), "keep");
    assert.equal(readlinkSync(destination), outside);
  });
});

test("PID-named temporary symlinks are never used for copies", () => fixture((temp) => {
  const paths = locations(temp, null);
  const { binary, icon } = sources(temp);
  install(binary, icon, paths);
  const outside = join(temp, "unrelated");
  writeFileSync(outside, "keep");
  const legacyTemp = join(paths.directory, `.aegis-client-${process.pid}.tmp`);
  symlinkSync(outside, legacyTemp);
  writeFileSync(binary, "updated");
  install(binary, icon, paths);
  assert.equal(readFileSync(outside, "utf8"), "keep");
  assert.equal(readlinkSync(legacyTemp), outside);
  assert.equal(readFileSync(paths.binary, "utf8"), "updated");
  assert.equal(readdirSync(paths.directory).some((name) => name.startsWith(".install-")), false);
}));

test("can reinstall using its existing binary and icon as source files", () => fixture((temp) => {
  const paths = locations(temp, null);
  const { binary, icon } = sources(temp);
  install(binary, icon, paths);
  install(paths.binary, paths.icon, paths);
  assert.equal(readFileSync(paths.binary, "utf8"), "first");
  assert.equal(readFileSync(paths.icon, "utf8"), "<svg/>");
}));

test("desktop quoting escapes both syntax layers and rejects invalid executable paths", () => {
  const paths = locations('/tmp/my "$home`quoted`\\dir', null);
  const entry = desktopEntry(paths);
  const exec = entry.split("\n").find((line) => line.startsWith("Exec="));
  assert.equal(exec, 'Exec="/tmp/my \\\\"\\\\$home\\\\`quoted\\\\`\\\\\\\\dir/.local/share/aegis-client/aegis-client"');
  assert.throws(() => locations("/tmp/%U", null), /Unsupported/);
  assert.throws(() => locations("/tmp/a=b", null), /Unsupported/);
  assert.throws(() => locations("/tmp/a\nName=Injected", null), /Unsupported/);
  assert.throws(() => locations("relative", null), /absolute/);
  assert.throws(() => desktopEntry({ ...paths, binary: "/tmp/%F" }), /Unsupported/);
  assert.equal(locations("/tmp/home", "relative").directory, "/tmp/home/.local/share/aegis-client");
});

test("the desktop launcher executes a quoted path using the real GLib parser", async (context) => {
  if (spawnSync("gio", ["help"], { stdio: "ignore" }).error) return context.skip("GLib gio is unavailable");
  const temp = mkdtempSync(join(tmpdir(), "aegis-desktop-launch-"));
  try {
    const home = join(temp, 'home "$HOME`echo harmless`\\dir\'s&;#()');
    const data = join(home, "data space");
    const paths = locations(home, data);
    const { binary, icon } = sources(temp);
    const output = join(temp, "launched");
    writeFileSync(binary, '#!/bin/sh\nprintf "%s\\n" "$0" > "$AEGIS_INSTALL_TEST_OUTPUT"\n');
    install(binary, icon, paths);
    const env = { ...process.env, HOME: home, XDG_DATA_HOME: data, XDG_CONFIG_HOME: join(home, "config"), XDG_CACHE_HOME: join(home, "cache"), AEGIS_INSTALL_TEST_OUTPUT: output };
    const validated = spawnSync("desktop-file-validate", [paths.desktop], { env, cwd: temp, encoding: "utf8", timeout: 5000 });
    if (validated.error?.code === "ENOENT") context.diagnostic("desktop-file-validate is unavailable; GLib launch still verifies Exec parsing");
    else assert.equal(validated.status, 0, validated.stderr || validated.error?.message);
    const result = spawnSync("gio", ["launch", paths.desktop], { env, cwd: temp, encoding: "utf8", timeout: 5000 });
    assert.equal(result.status, 0, result.stderr || result.error?.message);
    for (let attempt = 0; attempt < 100 && !existsSync(output); attempt++) await setTimeout(10);
    assert.equal(readFileSync(output, "utf8"), `${paths.binary}\n`);
  } finally { rmSync(temp, { recursive: true, force: true }); }
});
