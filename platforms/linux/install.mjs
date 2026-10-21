#!/usr/bin/env node
// Build and install the development UI for the current user. No elevated privileges.
import { copyFileSync, chmodSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, renameSync, rmSync, rmdirSync, symlinkSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const app = join(root, "apps/ionic");
const marker = "X-Aegis-Managed=true";
const ownershipFile = ".aegis-managed";
const ownershipContents = "Aegis development UI\n";

export function locations(home = homedir(), data = process.env.XDG_DATA_HOME) {
  if (!isAbsolute(home)) throw new Error("The home directory must be an absolute path");
  const dataHome = data && isAbsolute(data) ? data : join(home, ".local/share");
  // Desktop Entry cannot represent '=' in the executable name/path.
  if (/[=\x00-\x1f\x7f%]/u.test(dataHome)) throw new Error("Unsupported characters in XDG_DATA_HOME/home path");
  const directory = join(dataHome, "aegis-client");
  return {
    directory,
    binary: join(directory, "aegis-client"),
    icon: join(directory, "aegis-client.svg"),
    command: join(home, ".local/bin/aegis-client"),
    desktop: join(dataHome, "applications/dev.aegisprotocol.client.desktop"),
  };
}

function stringValue(value) {
  return value.replaceAll("\\", "\\\\");
}

export function desktopEntry(paths) {
  if (!isAbsolute(paths.binary) || /[=\x00-\x1f\x7f%]/u.test(paths.binary)) throw new Error("Unsupported desktop executable path");
  if (!isAbsolute(paths.icon) || /[\x00-\x1f\x7f]/u.test(paths.icon)) throw new Error("Unsupported desktop icon path");
  // Exec has two escaping layers: Desktop Entry strings, then argument quoting.
  // https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html
  const quoted = `"${paths.binary.replace(/[\\"$`]/gu, "\\$&")}"`;
  return `[Desktop Entry]\nType=Application\nName=Aegis (Development)\nComment=Aegis UI prototype; VPN module is not implemented\nExec=${stringValue(quoted)}\nIcon=${stringValue(paths.icon)}\nTerminal=false\nCategories=Network;Development;\n${marker}\n`;
}

function statOrNull(path) {
  try { return lstatSync(path); } catch (error) {
    if (error.code === "ENOENT") return null;
    throw error;
  }
}

function checkOwned(paths) {
  const command = statOrNull(paths.command);
  if (command && (!command.isSymbolicLink() || readlinkSync(paths.command) !== paths.binary)) {
    throw new Error(`Refusing to replace an unrelated file: ${paths.command}`);
  }
  const desktop = statOrNull(paths.desktop);
  if (desktop && (desktop.isSymbolicLink() || !desktop.isFile() || !readFileSync(paths.desktop, "utf8").split("\n").includes(marker))) {
    throw new Error(`Refusing to replace an unrelated launcher: ${paths.desktop}`);
  }
  const directory = statOrNull(paths.directory);
  if (directory) {
    if (directory.isSymbolicLink() || !directory.isDirectory()) throw new Error(`Refusing to use an unrelated directory: ${paths.directory}`);
    const ownership = join(paths.directory, ownershipFile);
    const owned = statOrNull(ownership);
    if (!owned || owned.isSymbolicLink() || !owned.isFile() || readFileSync(ownership, "utf8") !== ownershipContents) {
      throw new Error(`Refusing to use an unrelated directory: ${paths.directory}`);
    }
    for (const file of [paths.binary, paths.icon]) {
      const existing = statOrNull(file);
      if (existing && (existing.isSymbolicLink() || !existing.isFile())) throw new Error(`Refusing to replace an unrelated installed file: ${file}`);
    }
  }
}

export function install(binary, icon, paths = locations()) {
  checkOwned(paths);
  const launcher = desktopEntry(paths);
  for (const source of [binary, icon]) {
    if (!lstatSync(source).isFile()) throw new Error(`Not a regular installation source: ${source}`);
  }
  const directoryExisted = statOrNull(paths.directory);
  mkdirSync(paths.directory, { recursive: true });
  if (!directoryExisted) writeFileSync(join(paths.directory, ownershipFile), ownershipContents, { flag: "wx", mode: 0o644 });
  // A random private staging directory avoids following a pre-existing temp
  // symlink. Stage both inputs before replacing a functioning installation.
  const staging = mkdtempSync(join(paths.directory, ".install-"));
  try {
    const stagedBinary = join(staging, "binary");
    const stagedIcon = join(staging, "icon");
    copyFileSync(binary, stagedBinary);
    copyFileSync(icon, stagedIcon);
    chmodSync(stagedBinary, 0o755);
    chmodSync(stagedIcon, 0o644);
    mkdirSync(dirname(paths.command), { recursive: true });
    mkdirSync(dirname(paths.desktop), { recursive: true });
    writeFileSync(join(paths.directory, ownershipFile), ownershipContents, { mode: 0o644 });
    // Rename complete files so a failed copy never truncates a working binary.
    renameSync(stagedBinary, paths.binary);
    renameSync(stagedIcon, paths.icon);
    if (!statOrNull(paths.command)) symlinkSync(paths.binary, paths.command);
    writeFileSync(paths.desktop, launcher, { mode: 0o644 });
  } finally { rmSync(staging, { recursive: true, force: true }); }
  return paths;
}

export function uninstall(paths = locations()) {
  checkOwned(paths);
  rmSync(paths.command, { force: true });
  rmSync(paths.desktop, { force: true });
  if (!statOrNull(paths.directory)) return;
  // Keep ownership information if unrelated data remains, so uninstall is
  // repeatable and a subsequent installation can safely reuse this directory.
  for (const file of [paths.binary, paths.icon]) rmSync(file, { force: true });
  if (readdirSync(paths.directory).every((file) => file === ownershipFile)) {
    const ownership = join(paths.directory, ownershipFile);
    rmSync(ownership, { force: true });
    try { rmdirSync(paths.directory); } catch (error) {
      if (error.code !== "ENOTEMPTY" && error.code !== "EEXIST") throw error;
      // A user file may have appeared since the directory was inspected.
      try { writeFileSync(ownership, ownershipContents, { flag: "wx", mode: 0o644 }); } catch (restoreError) {
        if (restoreError.code !== "EEXIST") throw restoreError;
      }
    }
  }
}

function run(command, args, cwd = app) {
  const result = spawnSync(command, args, {
    cwd, stdio: "inherit",
    env: { ...process.env, CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || "2", CARGO_TARGET_DIR: join(app, "src-tauri/target") },
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} ${args.join(" ")} failed (${result.status ?? result.signal})`);
}

function main(args) {
  if (args.includes("--help")) {
    console.log("node platforms/linux/install.mjs [--no-build | --uninstall]\nDefault: npm ci, UI tests, locked desktop build, user-local installation.");
    return;
  }
  if (args.some((arg) => !["--no-build", "--uninstall"].includes(arg)) || args.length > 1) throw new Error("Use --help for supported options");
  if (process.platform !== "linux") throw new Error("This installer supports Linux only");
  if (process.getuid?.() === 0) throw new Error("Run as your desktop user, without sudo");
  const paths = locations();
  checkOwned(paths);
  if (args.includes("--uninstall")) {
    uninstall(paths);
    spawnSync("update-desktop-database", [dirname(paths.desktop)], { stdio: "ignore" });
    console.log("Aegis launcher and installed binary removed. UI preferences were retained.");
    return;
  }
  const [major, minor] = process.versions.node.split(".").map(Number);
  if (major < 22 || (major === 22 && minor < 12)) throw new Error("Node.js 22.12+ is required");
  if (!args.includes("--no-build")) {
    run("cargo", ["--version"]);
    run("pkg-config", ["--exists", "webkit2gtk-4.1", "gtk+-3.0"]);
    run("npm", ["ci"]);
    run("npm", ["test"]);
    run("npm", ["run", "desktop:build"]);
  }
  install(join(app, "src-tauri/target/release/aegis-client-desktop"), join(root, "platforms/linux/aegis-client.svg"), paths);
  spawnSync("update-desktop-database", [dirname(paths.desktop)], { stdio: "ignore" });
  console.log(`Installed: ${paths.binary}\nLaunch: ${paths.command}\nMenu: Aegis (Development)\nThis build has no VPN tunnel. Restart it after reinstalling an update.`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try { main(process.argv.slice(2)); } catch (error) {
    console.error(`Aegis installation failed: ${error.message}`);
    process.exitCode = 1;
  }
}
