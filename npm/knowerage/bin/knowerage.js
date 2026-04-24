#!/usr/bin/env node
const { spawn } = require('child_process');
const { getBinaryPath } = require('../index.js');

const binaryPath = getBinaryPath();
const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: 'inherit',
  shell: false,
});

child.on('error', (err) => {
  console.error(`Failed to start knowerage: ${err.message}`);
  process.exit(1);
});

child.on('exit', (code) => {
  process.exit(code ?? 1);
});
