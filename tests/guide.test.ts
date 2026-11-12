import { test } from "node:test";
import assert from "node:assert/strict";
import { shouldResync } from "../src/guide.ts";

const many = (x: number, n = 30) => Array<number>(n).fill(x);

test("jitter around zero never resyncs", () => {
  const jitter = Array.from({ length: 30 }, (_, i) => (i % 2 ? 0.04 : -0.04));
  assert.equal(shouldResync(jitter, 10_000, 0), false);
});

test("a few spikes don't count, the median does", () => {
  const spiky = [...many(0.01, 25), 0.5, 0.6, -0.7, 0.9, 1];
  assert.equal(shouldResync(spiky, 10_000, 0), false);
});

test("clearly off in either direction resyncs", () => {
  assert.equal(shouldResync(many(0.2), 10_000, 0), true);
  assert.equal(shouldResync(many(-0.2), 10_000, 0), true);
});

test("needs enough samples and respects the cooldown", () => {
  assert.equal(shouldResync(many(0.5, 10), 10_000, 0), false);
  assert.equal(shouldResync(many(0.5), 10_000, 8_000), false);
  assert.equal(shouldResync(many(0.5), 10_000, 6_000), true);
});
