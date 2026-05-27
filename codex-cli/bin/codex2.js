#!/usr/bin/env node
// Codex2 entry point: run the upstream Codex CLI with a separate default home.

import os from "node:os";
import path from "node:path";

if (!process.env.CODEX_HOME) {
  process.env.CODEX_HOME = path.join(os.homedir(), ".codex2");
}

process.env.CODEX2 = "1";

await import("./codex.js");
