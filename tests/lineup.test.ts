import { test } from "node:test";
import assert from "node:assert/strict";
import { move } from "../src/lineup.ts";

test("move swaps with the neighbour up or down", () => {
  assert.deepEqual(move(["a", "b", "c"], 1, -1), ["b", "a", "c"]);
  assert.deepEqual(move(["a", "b", "c"], 1, 1), ["a", "c", "b"]);
});

test("move past either end changes nothing", () => {
  const xs = ["a", "b"];
  assert.equal(move(xs, 0, -1), xs);
  assert.equal(move(xs, 1, 1), xs);
});
