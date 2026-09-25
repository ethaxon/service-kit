import { syncMetadata } from "../lib/metadata.ts";
import { select } from "@inquirer/prompts";
import { WorkflowTask, tasks, type WorkflowOptions } from "../lib/cli.ts";
import { checkLinks } from "../lib/docs.ts";
import { setup } from "./setup.ts";
import { check, test } from "./quality.ts";
import { buildDocs, syncDocs } from "./docs.ts";
import { build } from "./build.ts";
import { packageArtifacts, prepareDistribution } from "./release.ts";

export async function runWorkflow(options: WorkflowOptions): Promise<void> {
  if (options.task === WorkflowTask.Interactive) {
    if (!process.stdin.isTTY) throw new Error("Interactive selection requires a terminal; supply an explicit task in CI");
    const task = await select({ message: "Contributor workflow", choices: tasks.filter(t => t !== WorkflowTask.Interactive).map(value => ({ name: value, value })) });
    return runWorkflow({ ...options, task });
  }
  const workflows = {
    [WorkflowTask.SyncMetadata]: () => syncMetadata(), [WorkflowTask.Setup]: setup, [WorkflowTask.Check]: check, [WorkflowTask.Test]: test, [WorkflowTask.BuildDocs]: buildDocs, [WorkflowTask.CheckDocs]: checkLinks, [WorkflowTask.LinkDocs]: syncDocs,
    [WorkflowTask.Build]: () => build(options), [WorkflowTask.Package]: () => packageArtifacts(options),
    [WorkflowTask.Dist]: () => prepareDistribution(options),
  };
  const execute = workflows[options.task];
  await execute();
}
