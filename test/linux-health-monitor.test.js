const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const root = path.resolve(__dirname, '..');

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), 'utf8');
}

test('Linux health monitor releases the server mutex before readiness polling', () => {
  const main = read('TokenTrackerLinux/src-tauri/src/main.rs');
  const restart = main.indexOf('server.restart_process()');
  const release = main.indexOf('drop(guard)', restart);
  const wait = main.indexOf('server::wait_for_server_ready(', restart);

  assert.notEqual(restart, -1, 'health monitor should restart the child process');
  assert.notEqual(release, -1, 'health monitor should explicitly release the SERVER mutex');
  assert.notEqual(wait, -1, 'health monitor should poll readiness after restarting');
  assert.ok(restart < release, 'restart should begin while the server state is protected');
  assert.ok(release < wait, 'readiness polling must happen after releasing the SERVER mutex');
});

test('GitHub CI validates synchronized platform versions', () => {
  const workflow = read('.github/workflows/ci.yml');
  assert.match(workflow, /npm run validate:versions/);
});

test('GitHub CI compiles and tests the Linux desktop client', () => {
  const workflow = read('.github/workflows/ci.yml');
  assert.match(workflow, /linux-desktop:/);
  assert.match(workflow, /cargo test --locked --manifest-path TokenTrackerLinux\/src-tauri\/Cargo\.toml/);
  assert.match(workflow, /npm --prefix TokenTrackerLinux run bundle:node/);
  assert.match(workflow, /npm --prefix TokenTrackerLinux run build/);
});

test('Linux server startup uses embedded safe mode and Linux shell identity', () => {
  const server = read('TokenTrackerLinux/src-tauri/src/server.rs');
  assert.match(server, /\("TOKENTRACKER_APP_SHELL", "linux"\)/);
  assert.match(server, /\("TOKENTRACKER_EMBEDDED_SAFE", "1"\)/);
});

test('Linux desktop entry does not advertise an unimplemented deep-link protocol', () => {
  const desktop = read(
    'TokenTrackerLinux/packaging/arch/tokentracker-linux/tokentracker-linux.desktop',
  );
  assert.match(desktop, /^Exec=tokentracker-linux$/m);
  assert.doesNotMatch(desktop, /x-scheme-handler\/tokentracker|%u/);
});

test('Linux configures the WebKitGTK DMA-BUF compatibility fallback before Tauri starts', () => {
  const main = read('TokenTrackerLinux/src-tauri/src/main.rs');
  const configure = main.indexOf('configure_webkit_runtime();');
  const tauri = main.indexOf('tauri::Builder::default()', configure);

  assert.match(main, /WEBKIT_DISABLE_DMABUF_RENDERER/);
  assert.match(main, /var_os\(WEBKIT_DMABUF_ENV\)\.is_none\(\)/);
  assert.ok(configure !== -1 && configure < tauri);
});
