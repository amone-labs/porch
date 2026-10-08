import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const check = (env: Record<string, string> = {}) => {
  try {
    execFileSync("node", ["video/manifest.mjs", "--check"], { env: { ...process.env, ...env }, encoding: "utf8", stdio: "pipe" });
    return { code: 0, out: "" };
  } catch (e) {
    const err = e as { status: number; stderr: string };
    return { code: err.status, out: err.stderr };
  }
};

describe("video manifest", () => {
  it("the committed hero video and stills match their inputs", () => {
    expect(check()).toEqual({ code: 0, out: "" });
  });
  it("a changed input fails with the command that fixes it", () => {
    const path = join(mkdtempSync(join(tmpdir(), "porch-manifest-")), "manifest.json");
    writeFileSync(path, JSON.stringify({ inputs: "0".repeat(64) }));
    const r = check({ MANIFEST: path });
    expect(r.code).toBe(1);
    expect(r.out).toContain("pnpm video");
  });
});
