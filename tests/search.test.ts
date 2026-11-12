import { test } from "node:test";
import assert from "node:assert/strict";
import { matches, isOffline } from "../src/search.ts";

test("matches ignores case, accents and word order", () => {
  assert.ok(matches("ceu AZUL", ["Céu Azul", "Charlie Brown Jr."]));
  assert.ok(matches("charlie azul", ["Céu Azul", "Charlie Brown Jr."]));
  assert.ok(matches("céu", ["Ceu Azul (Clipe Oficial)", null]));
});

test("matches needs every word and a non-empty query", () => {
  assert.equal(matches("céu cinza", ["Céu Azul", null]), false);
  assert.equal(matches("   ", ["Céu Azul"]), false);
});

test("isOffline spots DNS failures on Linux and Windows", () => {
  assert.ok(isOffline(`ERROR: Failed to resolve 'www.youtube.com' ([Errno -3] Temporary failure in name resolution)`));
  assert.ok(isOffline("ERROR: Unable to download API page: [Errno 11001] getaddrinfo failed"));
  assert.equal(isOffline("ERROR: Sign in to confirm you're not a bot"), false);
});
