#!/usr/bin/env node

import { spawn } from 'child_process';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const projectRoot = join(__dirname, '..');
const port = process.env.PORT || 3000;

console.log('🚀 Starting Electron development mode...');

// Start the Vite dev server
console.log('📦 Starting Vite dev server...');
const viteProcess = spawn('npm', ['run', 'dev'], {
  cwd: projectRoot,
  stdio: 'inherit',
  shell: true
});

// Wait for server to be ready, then start Electron
const waitForServer = async () => {
  console.log('⏳ Waiting 5 seconds for server to start...');
  await new Promise(resolve => setTimeout(resolve, 5000));
  
  console.log('✅ Starting Electron...');
  
  // Start Electron
  const electronProcess = spawn('electron', ['.'], {
    cwd: projectRoot,
    stdio: 'inherit',
    env: {
      ...process.env,
      NODE_ENV: 'development'
    }
  });
  
  electronProcess.on('close', () => {
    console.log('🔴 Electron closed, stopping dev server...');
    viteProcess.kill();
    process.exit(0);
  });
};

// Handle cleanup
process.on('SIGINT', () => {
  console.log('🔴 Stopping development servers...');
  viteProcess.kill();
  process.exit(0);
});

process.on('SIGTERM', () => {
  viteProcess.kill();
  process.exit(0);
});

// Start waiting for server
waitForServer();