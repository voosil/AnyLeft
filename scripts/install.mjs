#!/usr/bin/env node

/**
 * Build (when requested or no current bundle exists), install, and on Windows
 * launch AnyLeft.
 *
 * Usage:
 *   pnpm app:install
 *   pnpm app:install --latest
 */

import {
  cpSync,
  existsSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
} from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const packageJson = JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8"));

const APP_NAME = "AnyLeft";
const MAIN_BINARY_NAME = "anyleft.exe";
const VERSION = packageJson.version;
const TARGET_ROOT = join(repoRoot, "src-tauri", "target");

function log(message) {
  process.stdout.write(`${message}\n`);
}

function fail(message) {
  throw new Error(message);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    stdio: options.stdio ?? "inherit",
    encoding: options.encoding,
    windowsHide: options.windowsHide ?? false,
  });

  if (result.error) {
    throw result.error;
  }

  const accepted = options.acceptedExitCodes ?? [0];
  if (!accepted.includes(result.status)) {
    fail(`${command} exited with code ${result.status ?? "unknown"}`);
  }
  return result;
}

function runPnpm(args) {
  // npm_execpath points at pnpm's JS entry when this script was started by
  // `pnpm app:install`; invoking it through Node avoids `.cmd`/shell quoting
  // differences between Windows and macOS.
  const pnpmEntry = process.env.npm_execpath;
  if (pnpmEntry && existsSync(pnpmEntry)) {
    return run(process.execPath, [pnpmEntry, ...args]);
  }
  return run(process.platform === "win32" ? "pnpm.cmd" : "pnpm", args);
}

function build() {
  log(">> pnpm app:build");
  runPnpm(["app:build"]);
}

function installMac({ latest }) {
  const bundle = join(TARGET_ROOT, "release", "bundle", "macos", `${APP_NAME}.app`);
  const destination = join("/Applications", `${APP_NAME}.app`);

  if (latest || !existsSync(bundle)) {
    build();
  }
  if (!existsSync(bundle)) {
    fail(`build succeeded but ${bundle} is missing`);
  }

  // A running process can keep files inside the bundle busy.
  spawnSync("pkill", ["-x", "anyleft"], { stdio: "ignore" });

  if (existsSync(destination)) {
    log(`>> removing existing ${destination}`);
    rmSync(destination, { recursive: true, force: true });
  }

  log(`>> installing to ${destination}`);
  cpSync(bundle, destination, { recursive: true, preserveTimestamps: true });

  log(">> stripping quarantine xattr");
  spawnSync("xattr", ["-dr", "com.apple.quarantine", destination], {
    stdio: "ignore",
  });

  log(`ok installed ${APP_NAME}`);
  log(`   launch: open ${destination}`);
}

function candidateBundleRoots(targetRoot) {
  const roots = [join(targetRoot, "release", "bundle")];
  if (!existsSync(targetRoot)) return roots;

  // Cross-target builds add a target-triple directory between `target` and
  // `release`; include those without recursively walking the full Cargo tree.
  for (const entry of readdirSync(targetRoot, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      roots.push(join(targetRoot, entry.name, "release", "bundle"));
    }
  }
  return roots;
}

function filesIn(directory, predicate) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isFile() && predicate(entry.name))
    .map((entry) => join(directory, entry.name));
}

function newest(paths) {
  return paths.sort((left, right) => statSync(right).mtimeMs - statSync(left).mtimeMs)[0];
}

/** Find the current-version Windows installer, preferring NSIS over MSI. */
export function findWindowsInstaller(targetRoot, version) {
  const versionToken = `_${version}_`;
  const roots = candidateBundleRoots(targetRoot);
  const nsis = roots.flatMap((root) =>
    filesIn(join(root, "nsis"), (name) =>
      name.toLowerCase().endsWith("-setup.exe") && name.includes(versionToken),
    ),
  );
  if (nsis.length > 0) return newest(nsis);

  const msi = roots.flatMap((root) =>
    filesIn(join(root, "msi"), (name) =>
      name.toLowerCase().endsWith(".msi") && name.includes(versionToken),
    ),
  );
  return msi.length > 0 ? newest(msi) : undefined;
}

/** Return the silent installer command used for a Windows bundle. */
export function windowsInstallPlan(installer) {
  if (installer.toLowerCase().endsWith(".msi")) {
    return {
      command: "msiexec.exe",
      args: [
        "/i",
        installer,
        "/quiet",
        "/norestart",
        "REINSTALL=ALL",
        "REINSTALLMODE=vomus",
      ],
      acceptedExitCodes: [0, 3010],
    };
  }
  return {
    command: installer,
    args: ["/S"],
    acceptedExitCodes: [0],
  };
}

function powershellInstalledExecutable() {
  const script = [
    "$roots = @(",
    "  'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',",
    "  'HKLM:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*',",
    "  'HKLM:\\Software\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*'",
    ")",
    `$item = Get-ItemProperty $roots -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -eq '${APP_NAME}' } | Select-Object -First 1`,
    "if ($null -ne $item) {",
    "  if ($item.DisplayIcon) { $item.DisplayIcon.Trim('\"') -replace ',0$', '' }",
    `  elseif ($item.InstallLocation) { Join-Path $item.InstallLocation '${MAIN_BINARY_NAME}' }`,
    "}",
  ].join("\n");

  const result = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-Command", script],
    { encoding: "utf8", windowsHide: true },
  );
  if (result.status !== 0) return undefined;

  return result.stdout
    ?.split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean);
}

function findInstalledWindowsExecutable() {
  const fromRegistry = powershellInstalledExecutable();
  if (fromRegistry && existsSync(fromRegistry)) return fromRegistry;

  const candidates = [
    process.env.LOCALAPPDATA && join(process.env.LOCALAPPDATA, APP_NAME, MAIN_BINARY_NAME),
    process.env.ProgramFiles && join(process.env.ProgramFiles, APP_NAME, MAIN_BINARY_NAME),
    process.env["ProgramFiles(x86)"] &&
      join(process.env["ProgramFiles(x86)"], APP_NAME, MAIN_BINARY_NAME),
  ].filter(Boolean);
  return candidates.find((path) => existsSync(path));
}

function launchWindows(executable) {
  log(`>> launching ${executable}`);
  const child = spawn(executable, [], {
    cwd: dirname(executable),
    detached: true,
    stdio: "ignore",
    windowsHide: false,
  });
  child.unref();
}

function installWindows({ latest }) {
  let installer = findWindowsInstaller(TARGET_ROOT, VERSION);
  if (latest || !installer) {
    build();
    installer = findWindowsInstaller(TARGET_ROOT, VERSION);
  }
  if (!installer) {
    fail(`build succeeded but no ${APP_NAME} ${VERSION} NSIS/MSI installer was found`);
  }

  // Tauri's installer refuses to replace a running executable. No match is a
  // normal first-install case, so taskkill's exit code is intentionally ignored.
  spawnSync("taskkill.exe", ["/IM", MAIN_BINARY_NAME, "/T", "/F"], {
    stdio: "ignore",
    windowsHide: true,
  });

  const plan = windowsInstallPlan(installer);
  log(`>> installing ${installer}`);
  run(plan.command, plan.args, {
    acceptedExitCodes: plan.acceptedExitCodes,
    windowsHide: true,
  });

  const executable = findInstalledWindowsExecutable();
  if (!executable) {
    fail("installation finished but the installed AnyLeft executable could not be located");
  }
  launchWindows(executable);
  log(`ok installed and launched ${APP_NAME}`);
}

function printHelp() {
  log("Usage: pnpm app:install [--latest]");
  log("  --latest  always rebuild before installing");
}

export function main(args = process.argv.slice(2), platform = process.platform) {
  const unknown = args.filter((arg) => !["--latest", "--help", "-h"].includes(arg));
  if (unknown.length > 0) fail(`unknown option: ${unknown[0]}`);
  if (args.includes("--help") || args.includes("-h")) {
    printHelp();
    return;
  }

  const options = { latest: args.includes("--latest") };
  if (platform === "darwin") return installMac(options);
  if (platform === "win32") return installWindows(options);
  fail("app:install supports macOS and Windows; run it in PowerShell/cmd, not WSL");
}

const isEntryPoint =
  process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href;
if (isEntryPoint) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`error: ${error.message}\n`);
    process.exitCode = 1;
  }
}
