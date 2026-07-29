const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { test } = require("node:test");

const servePath = path.join(__dirname, "..", "src", "commands", "serve.js");

test("DMG server startup repairs all supported runtime integrations", () => {
  const source = fs.readFileSync(servePath, "utf8");
  assert.match(source, /repairRuntimeIntegrations/);
  assert.doesNotMatch(source, /repairCodexNotifyIntegration/);
});

test("embedded safe mode skips automatic init, runtime replacement, and integration repair", () => {
  const source = fs.readFileSync(servePath, "utf8");

  assert.match(
    source,
    /if \(opts\.embeddedSafe\) \{[\s\S]*?embedded safe mode will not change CLI integrations[\s\S]*?\} else \{/,
  );
  assert.match(
    source,
    /if \(!opts\.embeddedSafe\) \{[\s\S]*?installLocalTrackerApp[\s\S]*?repairRuntimeIntegrations/,
  );
  assert.match(
    source,
    /ensurePortFreeFn: opts\.portExplicit && !opts\.embeddedSafe \? ensurePortFree : null/,
  );
});
