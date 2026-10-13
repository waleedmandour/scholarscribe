// ScholarScribe. tauri JS/Rust version parity gate.
// The tauri CLI aborts `tauri build` when the installed @tauri-apps/*
// npm packages are not on the same major/minor as their Rust crate
// counterparts (see release run 38378180763, tag v2.2.1-r5 first push).
// This gate catches the drift in CI (npm test) before a tag can be cut.
//
// Pairs checked (major.minor must match):
//   tauri (crate)              <-> @tauri-apps/api
//   tauri-plugin-shell (crate) <-> @tauri-apps/plugin-shell
//   tauri-plugin-dialog (crate)<-> @tauri-apps/plugin-dialog

import { readFileSync } from "node:fs";
import { join } from "node:path";

const root = process.cwd();

function crateVersion(cargoLockText, crateName) {
  const re = new RegExp(`name = "${crateName}"\\nversion = "([^"]+)"`);
  const m = cargoLockText.match(re);
  return m ? m[1] : null;
}

function npmVersion(pkgLock, pkgName) {
  const entry = (pkgLock.packages || {})[`node_modules/${pkgName}`];
  return entry && entry.version ? entry.version : null;
}

function short(v) {
  if (!v) return "absent";
  return v;
}

const cargoLock = readFileSync(join(root, "src-tauri", "Cargo.lock"), "utf8");
const pkgLock = JSON.parse(readFileSync(join(root, "package-lock.json"), "utf8"));

const pairs = [
  { crate: "tauri", npm: "@tauri-apps/api" },
  { crate: "tauri-plugin-shell", npm: "@tauri-apps/plugin-shell" },
  { crate: "tauri-plugin-dialog", npm: "@tauri-apps/plugin-dialog" },
];

let failed = false;

for (const { crate, npm } of pairs) {
  const cv = crateVersion(cargoLock, crate);
  const nv = npmVersion(pkgLock, npm);

  if (!cv || !nv) {
    console.error(
      `PARITY: MISSING  ${crate}=${short(cv)} in src-tauri/Cargo.lock, ${npm}=${short(nv)} in package-lock.json`
    );
    failed = true;
    continue;
  }

  const cm = cv.split(".").slice(0, 2).join(".");
  const nm = nv.split(".").slice(0, 2).join(".");

  if (cm !== nm) {
    console.error(
      `PARITY: MISMATCH ${crate} v${cv} (major.minor ${cm}) vs ${npm} v${nv} (major.minor ${nm})`
    );
    console.error(
      `  Fix: npm install ${npm}@${cm}.x  (or update the crate) so both sides share major.minor ${cm}.`
    );
    failed = true;
  } else {
    console.log(`PARITY: ok  ${crate} v${cv} <-> ${npm} v${nv}`);
  }
}

if (failed) {
  console.error("PARITY: tauri JS/Rust version drift detected; `tauri build` would fail. See the pairs above.");
  process.exit(1);
}

console.log("PARITY: all tauri JS/Rust pairs share the same major.minor.");
