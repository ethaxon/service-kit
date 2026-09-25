import { argument, choice, object, option, optional, parseSync, string, withDefault, formatMessage } from "@optique/core";

export const WorkflowTask = {
  Setup: "setup",
  Check: "check",
  Test: "test",
  BuildDocs: "build-docs",
  CheckDocs: "check-docs",
  LinkDocs: "link-docs",
  SyncMetadata: "sync-metadata",
  Build: "build",
  Package: "package",
  Dist: "dist",
  Interactive: "interactive",
} as const;
export type WorkflowTask = (typeof WorkflowTask)[keyof typeof WorkflowTask];

export const tasks = Object.values(WorkflowTask);

export const Builder = {
  Cargo: "cargo",
  Zigbuild: "zigbuild",
} as const;
export type Builder = (typeof Builder)[keyof typeof Builder];

export function parseArgs(args: string[]) {
  const parser = object({
    task: argument(choice(tasks)),
    target: optional(option("--target", string())),
    builder: withDefault(option("--builder", choice(Object.values(Builder))), Builder.Cargo),
    releaseTag: optional(option("--release-tag", string())),
  });
  const parsed = parseSync(parser, args);
  if (!parsed.success) throw new Error(formatMessage(parsed.error));
  return parsed.value;
}

export type WorkflowOptions = ReturnType<typeof parseArgs>;
