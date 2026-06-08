import { beforeEach, describe, expect, it } from "vitest";

import type { Module, WorkspaceModel } from "$lib/core/types.js";
import { wsStore } from "./workspace.svelte.js";

beforeEach(() => {
  wsStore.workspace = null;
  wsStore.moduleSources = {};
  wsStore.dependencyModules = [];
});

describe("allModules", () => {
  it("is empty with no workspace", () => {
    expect(wsStore.allModules).toEqual([]);
  });

  it("maps each file to its FQN and live source", () => {
    wsStore.workspace = { files: [{ fqn: "a" }, { fqn: "b" }] } as unknown as WorkspaceModel;
    wsStore.moduleSources = { a: "system A {}" };
    expect(wsStore.allModules).toEqual([
      { fqn: "a", source: "system A {}" },
      { fqn: "b", source: "" }, // no buffer yet → empty
    ]);
  });

  it("tolerates a file with no fqn", () => {
    wsStore.workspace = { files: [{}] } as unknown as WorkspaceModel;
    expect(wsStore.allModules).toEqual([{ fqn: "", source: "" }]);
  });
});

describe("externalModules", () => {
  it("exposes the resolved dependency modules", () => {
    const deps: Module[] = [{ fqn: "auth::core", source: "" }];
    wsStore.dependencyModules = deps;
    expect(wsStore.externalModules).toEqual(deps);
  });
});
