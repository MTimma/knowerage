#!/usr/bin/env node
const { spawn } = require('child_process');
const fs = require('fs');
const { getBinaryPath } = require('../index.js');

const binaryPath = getBinaryPath();

let didChmodRetry = false;

function spawnOnce() {
  const child = spawn(binaryPath, process.argv.slice(2), {
    stdio: 'inherit',
    shell: false,
  });

  child.on('error', (err) => {
    if (err && err.code === 'EACCES' && !didChmodRetry) {
      didChmodRetry = true;
      try {
        fs.chmodSync(binaryPath, 0o755);
        spawnOnce();
        return;
      } catch (chmodErr) {
        console.error(`Failed to fix permissions for knowerage: ${chmodErr.message}`);
        process.exit(1);
      }
    }

    console.error(`Failed to start knowerage: ${err?.message ?? String(err)}`);
    process.exit(1);
  });

  child.on('exit', (code) => {
    process.exit(code ?? 1);
  });

  return child;
}

spawnOnce();
