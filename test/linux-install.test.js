const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const repoRoot = path.resolve(__dirname, "..");
const installScript = path.join(repoRoot, "TokenTrackerLinux", "scripts", "install-local.sh");

test("Linux local installer creates and removes a complete user installation", () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "tokentracker-linux-install-"));
  const prefix = path.join(tempDir, "prefix");
  const fixtures = path.join(tempDir, "fixtures");
  const runtime = path.join(fixtures, "EmbeddedServer");
  const binary = path.join(fixtures, "tokentracker-linux");
  const icon = path.join(fixtures, "icon.png");

  try {
    fs.mkdirSync(path.join(runtime, "tokentracker", "bin"), { recursive: true });
    fs.writeFileSync(path.join(runtime, "node"), "node fixture", { mode: 0o755 });
    fs.writeFileSync(path.join(runtime, "tokentracker", "bin", "tracker.js"), "tracker fixture");
    fs.writeFileSync(binary, "binary fixture", { mode: 0o755 });
    fs.writeFileSync(icon, "icon fixture");

    const env = {
      ...process.env,
      TOKENTRACKER_LINUX_PREFIX: prefix,
      TOKENTRACKER_LINUX_BINARY: binary,
      TOKENTRACKER_LINUX_RUNTIME: runtime,
      TOKENTRACKER_LINUX_ICON: icon,
    };
    const installed = spawnSync("bash", [installScript], {
      cwd: repoRoot,
      env,
      encoding: "utf8",
    });

    assert.equal(installed.status, 0, installed.stderr);
    assert.equal(fs.existsSync(path.join(prefix, "bin", "tokentracker-linux")), true);
    assert.equal(fs.existsSync(path.join(prefix, "lib", "tokentracker-linux", "node")), true);
    assert.equal(
      fs.readFileSync(path.join(prefix, "lib", "tokentracker-linux", "tokentracker", "bin", "tracker.js"), "utf8"),
      "tracker fixture",
    );
    assert.match(
      fs.readFileSync(path.join(prefix, "share", "applications", "tokentracker-linux.desktop"), "utf8"),
      new RegExp(`^Exec="${prefix.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\/bin\/tokentracker-linux"$`, "m"),
    );
    assert.equal(
      fs.existsSync(path.join(prefix, "share", "icons", "hicolor", "512x512", "apps", "tokentracker-linux.png")),
      true,
    );

    const uninstalled = spawnSync("bash", [installScript, "--uninstall"], {
      cwd: repoRoot,
      env,
      encoding: "utf8",
    });

    assert.equal(uninstalled.status, 0, uninstalled.stderr);
    assert.equal(fs.existsSync(path.join(prefix, "bin", "tokentracker-linux")), false);
    assert.equal(fs.existsSync(path.join(prefix, "lib", "tokentracker-linux")), false);
    assert.equal(
      fs.existsSync(path.join(prefix, "share", "applications", "tokentracker-linux.desktop")),
      false,
    );
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test("Linux local installer keeps the previous installation when staging fails", () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "tokentracker-linux-upgrade-"));
  const prefix = path.join(tempDir, "prefix");
  const fixtures = path.join(tempDir, "fixtures");
  const runtime = path.join(fixtures, "EmbeddedServer");
  const binary = path.join(fixtures, "tokentracker-linux");
  const icon = path.join(fixtures, "icon.png");

  try {
    fs.mkdirSync(path.join(runtime, "tokentracker", "bin"), { recursive: true });
    fs.writeFileSync(path.join(runtime, "node"), "node fixture", { mode: 0o755 });
    fs.writeFileSync(path.join(runtime, "tokentracker", "bin", "tracker.js"), "v1 tracker");
    fs.writeFileSync(binary, "v1 binary", { mode: 0o755 });
    fs.writeFileSync(icon, "icon fixture");

    const env = {
      ...process.env,
      TOKENTRACKER_LINUX_PREFIX: prefix,
      TOKENTRACKER_LINUX_BINARY: binary,
      TOKENTRACKER_LINUX_RUNTIME: runtime,
      TOKENTRACKER_LINUX_ICON: icon,
    };
    assert.equal(spawnSync("bash", [installScript], { cwd: repoRoot, env }).status, 0);

    // A v2 runtime that stages cleanly, paired with a binary that cannot be
    // staged: the runtime swap must not happen before the binary is ready.
    fs.writeFileSync(path.join(runtime, "tokentracker", "bin", "tracker.js"), "v2 tracker");
    fs.rmSync(binary);
    fs.mkdirSync(binary);
    const upgraded = spawnSync("bash", [installScript], { cwd: repoRoot, env, encoding: "utf8" });

    assert.notEqual(upgraded.status, 0);
    assert.equal(
      fs.readFileSync(path.join(prefix, "lib", "tokentracker-linux", "tokentracker", "bin", "tracker.js"), "utf8"),
      "v1 tracker",
    );
    assert.equal(fs.readFileSync(path.join(prefix, "bin", "tokentracker-linux"), "utf8"), "v1 binary");
    assert.deepEqual(
      fs.readdirSync(path.join(prefix, "lib")).sort(),
      ["tokentracker-linux"],
    );
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test("Linux local installer rejects a root installation prefix", () => {
  // Noncanonical spellings normalize to root and must be refused too.
  for (const prefix of ["/", "//", "/.", "/tmp/..", "/usr/local/../.."]) {
    const result = spawnSync("bash", [installScript, "--uninstall"], {
      cwd: repoRoot,
      env: { ...process.env, TOKENTRACKER_LINUX_PREFIX: prefix },
      encoding: "utf8",
    });

    assert.notEqual(result.status, 0, `accepted root-equivalent prefix ${prefix}`);
    assert.match(result.stderr, /unsafe TokenTracker installation prefix/);
  }
});

test("Linux local installer rejects a relative installation prefix", () => {
  const result = spawnSync("bash", [installScript, "--uninstall"], {
    cwd: repoRoot,
    env: { ...process.env, TOKENTRACKER_LINUX_PREFIX: "relative/prefix" },
    encoding: "utf8",
  });

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /unsafe TokenTracker installation prefix/);
});
