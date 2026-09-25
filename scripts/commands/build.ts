import { Builder, type WorkflowOptions } from "../lib/cli.ts";
import { hostTarget, run } from "../lib/process.ts";

export async function build(options: WorkflowOptions): Promise<void> {
  const target = options.target ?? await hostTarget();
  await run("cargo", [options.builder === Builder.Zigbuild ? "zigbuild" : "build", "--workspace", "--release", "--locked", "--target", target]);
}
