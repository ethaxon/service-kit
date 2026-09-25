import type { WorkflowOptions } from "../lib/cli.ts";
import { hostTarget } from "../lib/process.ts";
import { packageRelease } from "../lib/release.ts";
import { build } from "./build.ts";
import { check, test } from "./quality.ts";

export async function packageArtifacts(options: WorkflowOptions, metadataChecked = false): Promise<void> {
  console.log(await packageRelease(options.target ?? await hostTarget(), options.releaseTag, undefined, metadataChecked));
}

export async function prepareDistribution(options: WorkflowOptions): Promise<void> {
  await check();
  await test();
  await build(options);
  await packageArtifacts(options, true);
}
