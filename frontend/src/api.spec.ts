/**
 * Unit tests for the typed Tauri command wrappers in `api.ts`.
 *
 * `api.ts` is a thin, hand-maintained mapping from JS calls to Tauri command
 * names + arg shapes. A wrong command string or a renamed/dropped arg key is a
 * silent runtime break (the command just fails or gets `undefined`), so these
 * tests pin the exact `invoke(name, args)` contract for the wrappers that matter
 * most: the core repo loop, host-server option flattening, the
 * default-argument-bearing ops, and the wrappers that post-process the result.
 *
 * `@tauri-apps/api/core` is mocked so `invoke` records what was called without
 * any Tauri runtime.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock the Tauri core module BEFORE importing api.ts (hoisted by vitest).
const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import {
  api,
  repositoryDumpApi,
  fileStageApi,
  fileDiffApi,
  branchMergeIntoApi,
  branchCreateApi,
  revisionHistoryApi,
  lockMessagingApi,
  storageApi,
  workingFileApi,
  desktopSettingsApi,
  isNoAuthConfigured,
  OPERATION_NOT_SUPPORTED,
  NO_AUTH_CONFIGURED,
} from "./api";

/** The (name, args) pair passed to the most recent invoke call. */
function lastCall(): [string, Record<string, unknown> | undefined] {
  const calls = invokeMock.mock.calls;
  const call = calls[calls.length - 1];
  return [call?.[0] as string, call?.[1] as Record<string, unknown> | undefined];
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(undefined);
});

describe("core repo-loop wrappers", () => {
  it("status() invokes 'status' with no args", () => {
    api.status();
    const [name, args] = lastCall();
    expect(name).toBe("status");
    expect(args).toBeUndefined();
  });

  it("log(limit) passes the limit through", () => {
    api.log(25);
    expect(lastCall()).toEqual(["log", { limit: 25 }]);
  });

  it("openRepository(path) maps the path arg", () => {
    api.openRepository("/tmp/world-bible");
    expect(lastCall()).toEqual(["open_repository", { path: "/tmp/world-bible" }]);
  });

  it("stage / unstage forward the path list", () => {
    api.stage(["a.md", "b.md"]);
    expect(lastCall()).toEqual(["stage", { paths: ["a.md", "b.md"] }]);
    api.unstage(["a.md"]);
    expect(lastCall()).toEqual(["unstage", { paths: ["a.md"] }]);
  });

  it("commit(message) maps the message arg", () => {
    api.commit("initial lore");
    expect(lastCall()).toEqual(["commit", { message: "initial lore" }]);
  });

  it("createBranch / switchBranch / mergeBranch map the name arg", () => {
    api.createBranch("feature/x");
    expect(lastCall()).toEqual(["create_branch", { name: "feature/x" }]);
    api.switchBranch("main");
    expect(lastCall()).toEqual(["switch_branch", { name: "main" }]);
    api.mergeBranch("dev");
    expect(lastCall()).toEqual(["merge_branch", { name: "dev" }]);
  });

  it("push / sync take no args", () => {
    api.push();
    expect(lastCall()).toEqual(["push", undefined]);
    api.sync();
    expect(lastCall()).toEqual(["sync", undefined]);
  });
});

describe("auth + onboarding wrappers", () => {
  it("recognizes v0.8.5 auth-disabled error contract", () => {
    const message = "No authentication configured on server";
    expect(isNoAuthConfigured(message)).toBe(true);
    expect(isNoAuthConfigured(new Error(message))).toBe(true);
    expect(isNoAuthConfigured({ kind: "CommandFailed", message })).toBe(true);
    expect(isNoAuthConfigured({ message: `${message}.` })).toBe(false);
    expect(isNoAuthConfigured({ kind: "CommandFailed" })).toBe(false);
  });

  it("recognizes nightly (f20ef0d7d+) NotSupported code 18 authless signal", () => {
    const message =
      "Operation not supported: No authentication configured on server";
    expect(isNoAuthConfigured(message)).toBe(true);
    expect(isNoAuthConfigured(new Error(message))).toBe(true);
    expect(isNoAuthConfigured({ kind: "CommandFailed", message })).toBe(true);
  });

  it("does NOT broadly swallow other NotSupported messages", () => {
    expect(isNoAuthConfigured("Operation not supported")).toBe(false);
    // Near-miss: a legitimate NotSupported error with a qualifier should NOT match
    expect(isNoAuthConfigured("Operation not supported: disk full")).toBe(false);
    expect(isNoAuthConfigured("Operation not supported on this platform")).toBe(false);
    expect(isNoAuthConfigured(new Error("Operation not supported: read-only filesystem"))).toBe(false);
    // Suffix/prefix variants should NOT match
    expect(isNoAuthConfigured({ kind: "CommandFailed", message: "Operation not supported." })).toBe(false);
    expect(
      isNoAuthConfigured({
        kind: "NotSupported",
        message:
          "Operation not supported: No authentication configured on server",
      }),
    ).toBe(true); // exact match still accepted regardless of kind
  });

  it("exports the canonical constants for external use", () => {
    expect(NO_AUTH_CONFIGURED).toBe("No authentication configured on server");
    expect(OPERATION_NOT_SUPPORTED).toBe(
      "Operation not supported: No authentication configured on server",
    );
  });

  it("authLoginWithToken maps remoteUrl + token", () => {
    api.authLoginWithToken("lore://srv/repo", "tok123");
    expect(lastCall()).toEqual([
      "auth_login_with_token",
      { remoteUrl: "lore://srv/repo", token: "tok123" },
    ]);
  });

  it("repositoryCreate derives the lore:// url from the name and returns the id", async () => {
    invokeMock.mockResolvedValueOnce({ id: "repo-7", name: "n", path: "/p" });
    const id = await api.repositoryCreate("/disk/path", "my-world");
    const [name, args] = lastCall();
    expect(name).toBe("repository_create");
    expect(args).toMatchObject({
      path: "/disk/path",
      repositoryUrl: "lore://localhost/my-world",
      description: "",
      id: "",
      useSharedStore: false,
      sharedStorePath: "",
    });
    expect(id).toBe("repo-7");
  });

  it("serviceStop defaults `all` to false", () => {
    api.serviceStop();
    expect(lastCall()).toEqual(["service_stop", { all: false }]);
    api.serviceStop(true);
    expect(lastCall()).toEqual(["service_stop", { all: true }]);
  });
});

describe("hostServerStart option flattening", () => {
  it("nulls every optional + nested s3/advanced field when only storeDir is given", () => {
    api.hostServerStart({ storeDir: "/srv/store" });
    const [name, args] = lastCall();
    expect(name).toBe("host_server_start");
    expect(args).toEqual({
      storeDir: "/srv/store",
      port: null,
      repositoryName: null,
      auth: false,
      bindHost: null,
      s3Bucket: null,
      s3Endpoint: null,
      s3Region: null,
      s3AccessKeyId: null,
      s3SecretAccessKey: null,
      s3ForcePathStyle: null,
      s3DynamodbEndpoint: null,
      advanced: null,
    });
  });

  it("flattens nested s3 options into the s3* arg keys", () => {
    api.hostServerStart({
      storeDir: "/srv/store",
      port: 41000,
      s3: { bucket: "lore-bucket", region: "us-east-1", forcePathStyle: true },
    });
    const [, args] = lastCall();
    expect(args).toMatchObject({
      port: 41000,
      s3Bucket: "lore-bucket",
      s3Region: "us-east-1",
      s3ForcePathStyle: true,
      s3Endpoint: null,
    });
  });

  it("passes the advanced bag through verbatim", () => {
    const advanced = { quic: { port: 9000 } };
    api.hostServerStart({ storeDir: "/s", advanced });
    const [, args] = lastCall();
    expect(args?.advanced).toBe(advanced);
  });
});

describe("lockMessagingApi.requestCheckin", () => {
  it("maps the args object and defaults note to ''", () => {
    lockMessagingApi.requestCheckin({
      path: "scene.umap",
      branch: "main",
      from: "alice",
      toUserId: "u-9",
      holder: "bob",
    });
    expect(lastCall()).toEqual([
      "lock_request_checkin",
      {
        path: "scene.umap",
        branch: "main",
        from: "alice",
        toUserId: "u-9",
        holder: "bob",
        note: "",
      },
    ]);
  });

  it("forwards an explicit note", () => {
    lockMessagingApi.requestCheckin({
      path: "p",
      branch: "",
      from: "a",
      toUserId: "u",
      holder: "h",
      note: "please release",
    });
    const [, args] = lastCall();
    expect(args?.note).toBe("please release");
  });
});

describe("ops-layer wrappers with default arguments", () => {
  it("repositoryDumpApi.dump applies its defaults", () => {
    repositoryDumpApi.dump();
    expect(lastCall()).toEqual([
      "repository_dump",
      { revision: "", path: "", maxDepth: 0 },
    ]);
  });

  it("branchCreateApi.create applies category/id defaults", () => {
    branchCreateApi.create("topic/foo");
    expect(lastCall()).toEqual([
      "branch_create",
      { branch: "topic/foo", category: "", id: "" },
    ]);
  });

  it("revisionHistoryApi.history maps positional args to named keys", () => {
    revisionHistoryApi.history("rev1", "main", 1234, 50, true);
    expect(lastCall()).toEqual([
      "revision_history",
      {
        revision: "rev1",
        branch: "main",
        date: 1234,
        length: 50,
        onlyBranch: true,
      },
    ]);
  });

  it("fileStageApi.stage forwards caseChange + scan when provided", () => {
    fileStageApi.stage(["a"], "rename", true);
    expect(lastCall()).toEqual([
      "file_stage",
      { paths: ["a"], caseChange: "rename", scan: true },
    ]);
  });

  it("fileDiffApi.diff defaults contextLines to 3 and whitespace flags to false", () => {
    fileDiffApi.diff();
    const [name, args] = lastCall();
    expect(name).toBe("file_diff");
    expect(args).toMatchObject({
      paths: [],
      sourceRevision: "",
      targetRevision: "",
      diff3: false,
      contextLines: 3,
      ignoreWhitespaceEol: false,
      ignoreWhitespaceInline: false,
    });
  });

  it("branchMergeIntoApi.mergeInto reorders branchId ahead of message in the payload", () => {
    branchMergeIntoApi.mergeInto("target", "msg", "bid", "lnk", true);
    expect(lastCall()).toEqual([
      "branch_merge_into",
      {
        branch: "target",
        branchId: "bid",
        message: "msg",
        link: "lnk",
        ignoreLinks: true,
      },
    ]);
  });
});

describe("storage + working-file wrappers", () => {
  it("storageApi.open defaults all three args and returns the handle", async () => {
    invokeMock.mockResolvedValueOnce(42);
    const handle = await storageApi.open();
    expect(lastCall()).toEqual([
      "storage_open_handle",
      { repositoryPath: "", remoteUrl: "", inMemory: false },
    ]);
    expect(handle).toBe(42);
  });

  it("storageApi.putFile applies context/remoteWrite/localCache defaults", () => {
    storageApi.putFile(1, "frag", "/f");
    expect(lastCall()).toEqual([
      "storage_put_file",
      {
        handle: 1,
        partition: "frag",
        path: "/f",
        context: "",
        remoteWrite: false,
        localCache: false,
      },
    ]);
  });

  it("workingFileApi.writeText maps path + content", () => {
    workingFileApi.writeText("/wt/a.md", "hello");
    expect(lastCall()).toEqual([
      "write_text_file",
      { path: "/wt/a.md", content: "hello" },
    ]);
  });

  it("desktopSettingsApi setters map the boolean arg", () => {
    desktopSettingsApi.setAutostart(true);
    expect(lastCall()).toEqual(["set_autostart", { enabled: true }]);
    desktopSettingsApi.setCloseToTray(false);
    expect(lastCall()).toEqual(["set_close_to_tray", { enabled: false }]);
  });
});
