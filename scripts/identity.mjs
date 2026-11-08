#!/usr/bin/env node
// Keeps every copy of the app identity in step with app-identity.json.
// See docs/architecture.md §2.7.
//
//   node scripts/identity.mjs sync    write the values into each target file
//   node scripts/identity.mjs check   exit 1 if any target file is out of date (used by CI)
//
// To add a place that needs the product name or bundle id, add a target below.
// Prefer reading the value at runtime or build time instead, so the list stays short.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const identityPath = join(root, "app-identity.json");

// Same rule as AppIdentity in crates/sunset-domain/src/identity.rs.
const BUNDLE_ID = /^[A-Za-z][A-Za-z0-9-]*(\.[A-Za-z][A-Za-z0-9-]*)+$/;

/** Each target maps the identity onto one JSON file. */
const targets = [
  {
    file: "apps/desktop/src-tauri/tauri.conf.json",
    apply(conf, id) {
      conf.productName = id.productName;
      conf.identifier = id.bundleId;
      const main = conf.app?.windows?.find((w) => w.label === "main");
      if (!main) throw new Error("tauri.conf.json has no window with label \"main\"");
      main.title = id.productName;
    },
  },
];

function readIdentity() {
  const id = JSON.parse(readFileSync(identityPath, "utf8"));
  const errors = [];
  if (typeof id.productName !== "string" || id.productName.trim() === "") {
    errors.push("productName must be a non-empty string");
  }
  if (typeof id.bundleId !== "string" || !BUNDLE_ID.test(id.bundleId)) {
    errors.push(`bundleId ${JSON.stringify(id.bundleId)} must be reverse-DNS, e.g. com.example.app`);
  }
  if (errors.length) {
    throw new Error(`app-identity.json is invalid:\n  ${errors.join("\n  ")}`);
  }
  return id;
}

function main(mode) {
  if (mode !== "sync" && mode !== "check") {
    console.error("usage: node scripts/identity.mjs <sync|check>");
    process.exit(2);
  }
  const id = readIdentity();
  const stale = [];
  for (const target of targets) {
    const path = join(root, target.file);
    const before = readFileSync(path, "utf8");
    const json = JSON.parse(before);
    target.apply(json, id);
    const after = JSON.stringify(json, null, 2) + "\n";
    if (after === before) continue;
    if (mode === "sync") {
      writeFileSync(path, after);
      console.log(`updated ${target.file}`);
    } else {
      stale.push(target.file);
    }
  }
  if (stale.length) {
    console.error(
      `Out of date with app-identity.json:\n  ${stale.join("\n  ")}\nRun: pnpm identity:sync`,
    );
    process.exit(1);
  }
  console.log(`identity ${mode === "sync" ? "synced" : "ok"}: ${id.productName} (${id.bundleId})`);
}

try {
  main(process.argv[2]);
} catch (err) {
  console.error(err instanceof Error ? err.message : err);
  process.exit(1);
}
