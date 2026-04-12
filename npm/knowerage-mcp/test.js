const { describe, it } = require('node:test');
const assert = require('node:assert');
const { PLATFORMS, getBinaryPath } = require('./index.js');

describe('knowerage-mcp wrapper', () => {
  it('resolves correct platform package name for darwin-arm64', () => {
    assert.strictEqual(PLATFORMS['darwin-arm64'], 'knowerage-mcp-darwin-arm64');
  });

  it('resolves correct platform package name for linux-x64', () => {
    assert.strictEqual(PLATFORMS['linux-x64'], 'knowerage-mcp-linux-x64');
  });

  it('has no shell in spawn (verify code has shell: false)', () => {
    const fs = require('fs');
    const path = require('path');
    const binContent = fs.readFileSync(path.join(__dirname, 'bin', 'knowerage-mcp.js'), 'utf8');
    assert.ok(binContent.includes('shell: false'), 'spawn must use shell: false');
  });

  it('returns undefined for unsupported platform', () => {
    assert.strictEqual(PLATFORMS['freebsd-x64'], undefined);
  });

  it('covers all required platforms', () => {
    const required = ['darwin-arm64', 'darwin-x64', 'linux-x64', 'linux-arm64', 'win32-x64', 'win32-arm64'];
    for (const p of required) {
      assert.ok(PLATFORMS[p], `Missing platform: ${p}`);
    }
  });
});
