/**
 * SBAI-5841: the frontend mirror of `src-tauri/src/path_policy.rs`. The table
 * below is the same table the Rust unit tests assert, plus the deliberate
 * union rule: because the UI cannot know the target OS, a value passes when it
 * is absolute on *either* family, and the backend stays the authority.
 */
import { describe, expect, it } from "vitest";
import {
  classify,
  explainPathProblem,
  isAcceptableAbsolutePath,
  isNativeAbsolutePath,
  isWindowsPlatform,
} from "./pathPolicy";

const ACCEPTED = [
  // Windows drive-absolute, both separators, spaces, and the bare drive root.
  "C:\\LoreData",
  "c:\\loredata",
  "C:/x",
  "C:\\Lore Data\\with spaces",
  "C:\\",
  "D:\\",
  // UNC with both server and share.
  "\\\\server\\share",
  "\\\\server\\share\\lore",
  // Verbatim disk / verbatim UNC.
  "\\\\?\\C:\\x",
  "\\\\?\\UNC\\server\\share",
  "\\\\?\\UNC\\server\\share\\lore",
  // Unix-rooted.
  "/srv/lore",
  "/with spaces/x",
  "/srv/lore/../store",
  "//host/share",
];

const REJECTED = [
  "",
  "   ",
  ".",
  "..",
  "lore",
  "./x",
  ".\\lore",
  "..\\lore",
  "C:foo",
  "C:",
  "\\\\server",
  "\\\\server\\",
  "\\\\",
  "\\\\.\\COM1",
  "\\\\?\\lore",
  "\\foo",
  "~/lore",
];

describe("explainPathProblem / isAcceptableAbsolutePath", () => {
  it.each(ACCEPTED)("accepts %j", (value) => {
    expect(explainPathProblem(value)).toBeNull();
    expect(isAcceptableAbsolutePath(value)).toBe(true);
  });

  it.each(REJECTED)("rejects %j", (value) => {
    const message = explainPathProblem(value);
    expect(message).not.toBeNull();
    expect(isAcceptableAbsolutePath(value)).toBe(false);
    // Every rejection must say what to do, not just that it failed.
    expect(message).toMatch(
      /absolute path|drive letter|drive path|server and share|folder/,
    );
  });

  it("trims before judging, so padded absolute paths pass", () => {
    expect(explainPathProblem("  /srv/lore  ")).toBeNull();
    expect(explainPathProblem("  C:\\LoreData  ")).toBeNull();
    expect(isAcceptableAbsolutePath("\t/srv/lore\n")).toBe(true);
  });

  it("accepts /foo — the union edge: unix-absolute wins, backend decides", () => {
    // The Rust Windows classifier calls this driveless-rootful and rejects it;
    // the union accepts it because it is a perfectly good POSIX path and the
    // UI cannot know the target OS. Documented, intended.
    expect(classify("/foo", true)).toBe("rootRelative");
    expect(classify("/foo", false)).toBeNull();
    expect(explainPathProblem("/foo")).toBeNull();
  });
});

describe("rejection messages are actionable and name the value", () => {
  it("relative paths name the startup-folder hazard and a concrete example", () => {
    const message = explainPathProblem("lore") ?? "";
    expect(message).toContain('"lore"');
    expect(message).toContain("relative path");
    expect(message).toContain("startup folder");
    expect(message).toContain("C:\\LoreData");
    expect(message).toContain("/home/you/lore");
  });

  it("drive-relative paths say to add the backslash after the drive letter", () => {
    const message = explainPathProblem("C:foo") ?? "";
    expect(message).toContain('"C:foo"');
    expect(message).toContain("backslash after the drive letter");
    expect(message).toContain("C:\\LoreData");
    expect(explainPathProblem("C:") ?? "").toContain(
      "backslash after the drive letter",
    );
  });

  it("driveless rootful paths ask for the drive letter", () => {
    const message = explainPathProblem("\\foo") ?? "";
    expect(message).toContain("does not name a drive");
    expect(message).toContain("C:\\LoreData");
  });

  it("incomplete UNC paths ask for both server and share", () => {
    const message = explainPathProblem("\\\\server") ?? "";
    expect(message).toContain("incomplete network path");
    expect(message).toContain("server and share");
    expect(message).toContain("\\\\server\\share\\lore");
  });

  it("device paths say it is not a folder", () => {
    expect(explainPathProblem("\\\\.\\COM1") ?? "").toContain(
      "Windows device path, not a folder",
    );
  });

  it("unsupported verbatim forms name the supported shape", () => {
    const message = explainPathProblem("\\\\?\\lore") ?? "";
    expect(message).toContain("unsupported \\\\?\\ form");
    expect(message).toContain("\\\\?\\C:\\LoreData");
  });

  it("empty input asks for a folder without quoting an empty string", () => {
    const message = explainPathProblem("   ") ?? "";
    expect(message).toContain("A folder is required");
    expect(message).toContain("absolute path");
    expect(message).not.toContain('""');
  });
});

/**
 * The authorizing half. The union above may accept a path the *running*
 * platform cannot use; anything handed to the OS must clear this instead.
 */
describe("isNativeAbsolutePath judges the current platform only", () => {
  const WINDOWS_ACCEPTS = [
    "C:\\x",
    "C:/x",
    "C:\\LoreData",
    "C:\\",
    "\\\\server\\share",
    "\\\\server\\share\\lore",
    "\\\\?\\C:\\x",
    "\\\\?\\UNC\\server\\share",
  ];
  const WINDOWS_REJECTS = [
    "/foo",
    "/srv/lore",
    "\\foo",
    "C:foo",
    "C:",
    "lore",
    "./x",
    "",
    "   ",
    "\\\\server",
    "\\\\.\\COM1",
    "\\\\?\\lore",
  ];

  it.each(WINDOWS_ACCEPTS)("windows accepts %j", (value) => {
    expect(isNativeAbsolutePath(value, true)).toBe(true);
  });

  it.each(WINDOWS_REJECTS)("windows rejects %j", (value) => {
    expect(isNativeAbsolutePath(value, true)).toBe(false);
  });

  it.each(["/srv/x", "/with spaces/x", "/srv/lore/../store", "//host/share"])(
    "non-windows accepts %j",
    (value) => {
      expect(isNativeAbsolutePath(value, false)).toBe(true);
    },
  );

  it.each(["C:\\x", "C:/x", "C:\\LoreData", "\\\\server\\share", "lore", ""])(
    "non-windows rejects %j",
    (value) => {
      expect(isNativeAbsolutePath(value, false)).toBe(false);
    },
  );

  it("trims like the union check does", () => {
    expect(isNativeAbsolutePath("  /srv/lore  ", false)).toBe(true);
    expect(isNativeAbsolutePath("  C:\\LoreData  ", true)).toBe(true);
  });

  it("is strictly stricter than the union — the point of having both", () => {
    // Union-acceptable but NOT usable on Windows: `/foo` and `\foo` resolve
    // against the current drive there, so they must never start a picker.
    for (const value of ["/foo", "/srv/lore"]) {
      expect(isAcceptableAbsolutePath(value)).toBe(true);
      expect(isNativeAbsolutePath(value, true)).toBe(false);
    }
    // …and the mirror image on POSIX.
    expect(isAcceptableAbsolutePath("C:\\LoreData")).toBe(true);
    expect(isNativeAbsolutePath("C:\\LoreData", false)).toBe(false);
  });

  it("defaults to the detected platform", () => {
    const windows = isWindowsPlatform();
    expect(isNativeAbsolutePath("/srv/lore")).toBe(
      isNativeAbsolutePath("/srv/lore", windows),
    );
    expect(isNativeAbsolutePath("C:\\LoreData")).toBe(
      isNativeAbsolutePath("C:\\LoreData", windows),
    );
  });
});

describe("isWindowsPlatform", () => {
  it("reads the running platform from the user agent", () => {
    expect(isWindowsPlatform()).toBe(/\bWindows\b/.test(navigator.userAgent));
  });

  it("is false under jsdom, so the specs above pin both tables explicitly", () => {
    // jsdom's UA is built from process.platform ("win32", "linux", "darwin"),
    // none of which contain the word "Windows" — so this holds on every host
    // and the platform-specific expectations stay deterministic.
    expect(isWindowsPlatform()).toBe(false);
  });
});

describe("classify mirrors the backend's per-family table", () => {
  it("windows accepts exactly the documented absolute forms", () => {
    expect(classify("C:\\LoreData", true)).toBeNull();
    expect(classify("C:/x", true)).toBeNull();
    expect(classify("C:\\", true)).toBeNull();
    expect(classify("\\\\server\\share", true)).toBeNull();
    expect(classify("\\\\?\\C:\\x", true)).toBeNull();
    expect(classify("\\\\?\\UNC\\server\\share", true)).toBeNull();
  });

  it("windows names the specific defect for every rejected form", () => {
    expect(classify("", true)).toBe("empty");
    expect(classify(".", true)).toBe("relative");
    expect(classify("..", true)).toBe("relative");
    expect(classify("lore", true)).toBe("relative");
    expect(classify(".\\lore", true)).toBe("relative");
    expect(classify("C:foo", true)).toBe("driveRelative");
    expect(classify("C:", true)).toBe("driveRelative");
    expect(classify("\\foo", true)).toBe("rootRelative");
    expect(classify("\\\\", true)).toBe("incompleteUnc");
    expect(classify("\\\\server", true)).toBe("incompleteUnc");
    expect(classify("\\\\server\\", true)).toBe("incompleteUnc");
    expect(classify("\\\\.\\COM1", true)).toBe("deviceNamespace");
    expect(classify("\\\\?\\lore", true)).toBe("unsupportedVerbatim");
    expect(classify("\\\\?\\UNC\\server", true)).toBe("incompleteUnc");
  });

  it("unix accepts only rooted paths", () => {
    expect(classify("/srv/lore", false)).toBeNull();
    expect(classify("//host/share", false)).toBeNull();
    expect(classify("", false)).toBe("empty");
    expect(classify(".", false)).toBe("relative");
    expect(classify("lore", false)).toBe("relative");
    expect(classify("C:\\LoreData", false)).toBe("relative");
    expect(classify("C:foo", false)).toBe("relative");
  });
});
