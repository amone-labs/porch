import { request as httpRequest } from "node:http";
import { networkInterfaces } from "node:os";
import { expect, test } from "@playwright/test";

function rawGet(host: string, path: string): Promise<number> {
  return new Promise((resolve, reject) => {
    const req = httpRequest({ host, port: 4329, path, timeout: 2000 }, (res) => {
      res.resume();
      resolve(res.statusCode ?? 0);
    });
    req.on("error", reject);
    req.on("timeout", () => req.destroy(new Error("timeout")));
    req.end();
  });
}

test("a malformed URL gets 400 and the server keeps serving", async () => {
  expect(await rawGet("127.0.0.1", "/%")).toBe(400);
  expect(await rawGet("127.0.0.1", "/")).toBe(200);
});

test("the server is not reachable from other machines", async () => {
  const lan = Object.values(networkInterfaces())
    .flat()
    .find((a) => a && a.family === "IPv4" && !a.internal);
  test.skip(!lan, "no non-loopback IPv4 address on this machine");
  await expect(rawGet(lan!.address, "/")).rejects.toThrow();
});
