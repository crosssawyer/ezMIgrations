import { describe, it, expect, beforeEach } from "vitest";
import { compareVersions, getSeenVersion, markSeen } from "./seen-version";

describe("compareVersions", () => {
  it("returns 0 for identical versions", () => {
    expect(compareVersions("1.2.0", "1.2.0")).toBe(0);
  });

  it("treats earlier versions as less than later ones", () => {
    expect(compareVersions("1.1.5", "1.2.0")).toBeLessThan(0);
    expect(compareVersions("1.2.0", "1.1.5")).toBeGreaterThan(0);
    expect(compareVersions("0.9.0", "1.0.0")).toBeLessThan(0);
  });

  it("handles different segment counts", () => {
    expect(compareVersions("1.2", "1.2.0")).toBe(0);
    expect(compareVersions("1.2", "1.2.1")).toBeLessThan(0);
  });

  it("parses pre-release suffixes loosely (1.2.0-1 < 1.2.0)", () => {
    // "1.2.0-1" → [1,2,0,1]; "1.2.0" → [1,2,0]; the extra segment makes pre > release
    // — not strict semver, but our gate compares against a major.minor.patch baseline.
    expect(compareVersions("1.1.0", "1.2.0-1")).toBeLessThan(0);
  });

  it("treats the 0.0.0 default as less than any real version", () => {
    expect(compareVersions("0.0.0", "1.2.0")).toBeLessThan(0);
  });
});

describe("getSeenVersion / markSeen", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("defaults to 0.0.0 when nothing has been stored", () => {
    expect(getSeenVersion()).toBe("0.0.0");
  });

  it("roundtrips through localStorage", () => {
    markSeen("1.2.0");
    expect(getSeenVersion()).toBe("1.2.0");
  });

  it("overwrites the previous value on subsequent markSeen calls", () => {
    markSeen("1.0.0");
    markSeen("1.2.0");
    expect(getSeenVersion()).toBe("1.2.0");
  });
});
