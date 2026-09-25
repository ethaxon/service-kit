#!/usr/bin/env node
import { parseArgs, tasks } from "./lib/cli.ts";
import { runWorkflow } from "./commands/index.ts";

try {
  if (process.argv.includes("--help")) {
    console.log(`Usage: node scripts/dev-cli.ts <${tasks.join("|")}> [--target TRIPLE] [--builder cargo|zigbuild] [--release-tag VERSION]`);
  } else {
    await runWorkflow(parseArgs(process.argv.slice(2)));
  }
} catch (error) {
  console.error(error);
  process.exitCode = 1;
}
