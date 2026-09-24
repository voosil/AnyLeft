import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { mkdtempSync } from "node:fs";
import test from "node:test";

import { findWindowsInstaller, pnpmInvocation, windowsInstallPlan } from "./install.mjs";

function bundle(root, type, name) {
  const directory = join(root, "release", "bundle", type);
  mkdirSync(directory, { recursive: true });
  const path = join(directory, name);
  writeFileSync(path, "fixture");
  return path;
}

test("findWindowsInstaller prefers the current-version NSIS bundle", () => {
  const root = mkdtempSync(join(tmpdir(), "anyleft-install-"));
  bundle(root, "nsis", "AnyLeft_0.1.2_x64-setup.exe");
  const expected = bundle(root, "nsis", "AnyLeft_0.1.3_x64-setup.exe");
  bundle(root, "msi", "AnyLeft_0.1.3_x64_en-US.msi");

  assert.equal(findWindowsInstaller(root, "0.1.3"), expected);
});

test("findWindowsInstaller falls back to MSI", () => {
  const root = mkdtempSync(join(tmpdir(), "anyleft-install-"));
  const expected = bundle(root, "msi", "AnyLeft_0.1.3_x64_en-US.msi");

  assert.equal(findWindowsInstaller(root, "0.1.3"), expected);
});

test("windowsInstallPlan uses Tauri's silent NSIS flag", () => {
  assert.deepEqual(windowsInstallPlan("C:\\build\\AnyLeft_0.1.3_x64-setup.exe"), {
    command: "C:\\build\\AnyLeft_0.1.3_x64-setup.exe",
    args: ["/S"],
    acceptedExitCodes: [0],
  });
});

test("windowsInstallPlan configures a quiet MSI reinstall", () => {
  const plan = windowsInstallPlan("C:\\build\\AnyLeft_0.1.3_x64_en-US.msi");
  assert.equal(plan.command, "msiexec.exe");
  assert.deepEqual(plan.args.slice(0, 4), [
    "/i",
    "C:\\build\\AnyLeft_0.1.3_x64_en-US.msi",
    "/quiet",
    "/norestart",
  ]);
  assert.deepEqual(plan.acceptedExitCodes, [0, 3010]);
});

test("pnpmInvocation re-enters pnpm through a script entry", () => {
  const invocation = pnpmInvocation(
    { npm_execpath: "/opt/pnpm/pnpm.cjs" },
    "darwin",
    () => true,
  );
  assert.deepEqual(invocation, {
    command: process.execPath,
    prefix: ["/opt/pnpm/pnpm.cjs"],
  });
});

test("pnpmInvocation ignores a native launcher as npm_execpath", () => {
  // pnpm 12 points npm_execpath at a native binary in its tool directory; Node
  // cannot load it, so the pnpm on PATH has to be used instead.
  const invocation = pnpmInvocation(
    { npm_execpath: "/Users/x/Library/pnpm/.tools/pnpm/12.1.0/pnpm" },
    "darwin",
    () => true,
  );
  assert.deepEqual(invocation, { command: "pnpm", prefix: [] });
});

test("pnpmInvocation falls back to the pnpm on PATH", () => {
  assert.deepEqual(pnpmInvocation({}, "darwin"), { command: "pnpm", prefix: [] });
  assert.deepEqual(pnpmInvocation({}, "win32"), { command: "pnpm.cmd", prefix: [] });
});
