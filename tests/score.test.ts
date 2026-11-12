import { test } from "node:test";
import assert from "node:assert/strict";
import { randomScore, phrase } from "../src/score.ts";

/** rand() that returns the given values in order. */
const seq = (...xs: number[]) => () => xs.shift()!;

test("score is always 30–100 and usually high", () => {
  const scores = Array.from({ length: 10_000 }, () => randomScore());
  assert.ok(scores.every((s) => Number.isInteger(s) && s >= 30 && s <= 100));
  const high = scores.filter((s) => s >= 60).length / scores.length;
  assert.ok(high > 0.9, `only ${high} were >= 60`);
});

test("edges of the distribution", () => {
  assert.equal(randomScore(seq(0.5, 0)), 100); // best roll
  assert.equal(randomScore(seq(0.5, 0.9999)), 60); // worst normal roll
  assert.equal(randomScore(seq(0.01, 0)), 30); // flop
  assert.equal(randomScore(seq(0.01, 0.9999)), 59);
});

test("phrases by range", () => {
  assert.equal(phrase(100), "PERFEITO!");
  assert.equal(phrase(99), "Arrasou!");
  assert.equal(phrase(90), "Arrasou!");
  assert.equal(phrase(89), "Passou raspando.");
  assert.equal(phrase(69), "Passou raspando.");
  assert.equal(phrase(68), "Melhor não largar o emprego…");
  assert.equal(phrase(30), "Melhor não largar o emprego…");
});
