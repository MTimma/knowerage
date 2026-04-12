const path = require('path');

const PLATFORMS = {
  'darwin-arm64': 'knowerage-mcp-darwin-arm64',
  'darwin-x64': 'knowerage-mcp-darwin-x64',
  'linux-x64': 'knowerage-mcp-linux-x64',
  'linux-arm64': 'knowerage-mcp-linux-arm64-gnu',
  'win32-x64': 'knowerage-mcp-win32-x64',
  'win32-arm64': 'knowerage-mcp-win32-arm64',
};

function getBinaryPath() {
  const key = `${process.platform}-${process.arch}`;
  const pkg = PLATFORMS[key];
  if (!pkg) {
    console.error(`Unsupported platform: ${key}`);
    console.error(`Supported platforms: ${Object.keys(PLATFORMS).join(', ')}`);
    process.exit(1);
  }

  try {
    const binName = process.platform === 'win32' ? 'knowerage-mcp.exe' : 'knowerage-mcp';
    return path.join(path.dirname(require.resolve(`${pkg}/package.json`)), binName);
  } catch {
    console.error(`Platform package '${pkg}' not installed.`);
    console.error('Try reinstalling: npm install knowerage-mcp');
    process.exit(1);
  }
}

module.exports = { getBinaryPath, PLATFORMS };
