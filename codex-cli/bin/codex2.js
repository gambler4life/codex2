#!/usr/bin/env node
// Codex2 entry point: run the upstream Codex CLI with a separate default home.

import os from "node:os";
import path from "node:path";

if (!process.env.CODEX_HOME) {
  process.env.CODEX_HOME = path.join(os.homedir(), ".codex2");
}

process.env.CODEX2 = "1";

function hasUpdateCheckConfigArg(args) {
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === "-c" || arg === "--config") {
      if (args[i + 1]?.includes("check_for_update_on_startup")) {
        return true;
      }
      i += 1;
      continue;
    }
    if (
      arg.startsWith("-c") &&
      arg.slice(2).includes("check_for_update_on_startup")
    ) {
      return true;
    }
    if (
      arg.startsWith("--config=") &&
      arg.includes("check_for_update_on_startup")
    ) {
      return true;
    }
  }
  return false;
}

const updateCheckExplicitlyConfigured = hasUpdateCheckConfigArg(process.argv.slice(2));

if (!process.env.CODEX2_ENABLE_UPDATE_CHECK && !updateCheckExplicitlyConfigured) {
  process.argv.splice(2, 0, "-c", "check_for_update_on_startup=false");
}

await import("./codex.js");
