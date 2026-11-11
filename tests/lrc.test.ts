import { test } from "node:test";
import assert from "node:assert/strict";
import { parseLrc, lineAt } from "../src/lrc.ts";

test("parses lrc with multiple tags, metadata and blank lines", () => {
  const lines = parseLrc("[ar:Queen]\n[00:10.50]Is this the real life?\r\n\n[00:05.00][01:02]Mama\n[00:20.00]");
  assert.deepEqual(lines, [
    { t: 5, text: "Mama" },
    { t: 10.5, text: "Is this the real life?" },
    { t: 20, text: "" },
    { t: 62, text: "Mama" },
  ]);
});

test("lineAt finds the current line", () => {
  const lines = parseLrc("[00:05.00]a\n[00:10.00]b");
  assert.equal(lineAt(lines, 1), -1);
  assert.equal(lineAt(lines, 5), 0);
  assert.equal(lineAt(lines, 9.9), 0);
  assert.equal(lineAt(lines, 30), 1);
});
