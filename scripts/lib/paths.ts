import { resolve } from "node:path";

export const root = resolve(import.meta.dirname, "../..");
export const tempRoot = resolve(root, "temp");
