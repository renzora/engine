#!/usr/bin/env node

import { program } from 'commander';
import { execSync, exec } from 'child_process';
import { existsSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.join(__dirname, '..');

program
  .name('renzora')
  .description('Renzora Engine CLI - Manage building, running, and development tasks')
  .version('1.0.0');

// Interactive TUI command
program
  .command('tui')
  .alias('ui')
  .description('Launch interactive terminal interface')
  .action(async () => {
    const { spawn } = await import('child_process');
    const tuiPath = path.join(__dirname, 'renzora-tui.js');
    
    const child = spawn('node', [tuiPath], {
      stdio: 'inherit',
      cwd: projectRoot
    });
    
    child.on('exit', (code) => {
      process.exit(code);
    });
  });

// Helper functions
function runCommand(command, options = {}) {
  try {
    const result = execSync(command, { 
      cwd: projectRoot, 
      stdio: 'inherit',
      ...options 
    });
    return { success: true, result };
  } catch (error) {
    console.error(`Failed to execute: ${command}`);
    return { success: false, error };
  }
}

function runCommandAsync(command, options = {}) {
  return new Promise((resolve, reject) => {
    const child = exec(command, { 
      cwd: projectRoot, 
      ...options 
    }, (error, stdout, stderr) => {
      if (error) {
        reject(error);
      } else {
        resolve({ stdout, stderr });
      }
    });
    
    child.stdout?.pipe(process.stdout);
    child.stderr?.pipe(process.stderr);
  });
}

function killPorts() {
  console.log('🧹 Clearing ports and stopping processes...');
  const killCommand = process.platform === 'win32' 
    ? 'npx kill-port 3000 3001 || (taskkill /F /IM node.exe 2>nul || echo "Ports cleared")'
    : 'npx kill-port 3000 3001 || (pkill -f "rspack\\|tauri" 2>/dev/null || echo "Ports cleared")';
  
  return runCommand(killCommand);
}

function checkProcesses() {
  try {
    const command = process.platform === 'win32' 
      ? 'netstat -ano | findstr ":3000\\|:3001"'
      : 'lsof -i :3000,:3001';
    
    const result = execSync(command, { 
      cwd: projectRoot, 
      encoding: 'utf8',
      stdio: 'pipe'
    });
    
    return result.trim() !== '';
  } catch {
    return false;
  }
}

// Build commands
program
  .command('build')
  .description('Build the project')
  .option('-t, --target <target>', 'Build target: web, app, bridge', 'web')
  .action((options) => {
    console.log(`🔨 Building ${options.target}...`);
    
    const commands = {
      web: 'bun run build:web',
      app: 'bun run build:app', 
      bridge: 'bun run build:bridge'
    };
    
    const command = commands[options.target];
    if (!command) {
      console.error(`❌ Unknown build target: ${options.target}`);
      process.exit(1);
    }
    
    const result = runCommand(command);
    if (result.success) {
      console.log(`✅ ${options.target} build completed successfully!`);
    } else {
      console.error(`❌ ${options.target} build failed!`);
      process.exit(1);
    }
  });

// Run/Start commands
program
  .command('start')
  .alias('run')
  .description('Start the development server')
  .option('-t, --target <target>', 'Run target: web, app, bridge, serve', 'web')
  .action(async (options) => {
    console.log(`🚀 Starting ${options.target} development server...`);
    
    const commands = {
      web: 'bun run web',
      app: 'bun run app',
      bridge: 'bun run bridge',
      serve: 'bun run serve'
    };
    
    const command = commands[options.target];
    if (!command) {
      console.error(`❌ Unknown run target: ${options.target}`);
      process.exit(1);
    }
    
    try {
      await runCommandAsync(command);
    } catch (error) {
      console.error(`❌ Failed to start ${options.target}:`, error.message);
      process.exit(1);
    }
  });

// Stop command
program
  .command('stop')
  .description('Stop all running processes and clear ports')
  .action(() => {
    const result = killPorts();
    if (result.success) {
      console.log('✅ All processes stopped and ports cleared!');
    } else {
      console.error('❌ Failed to stop some processes');
      process.exit(1);
    }
  });

// Restart command
program
  .command('restart')
  .description('Restart the development server')
  .option('-t, --target <target>', 'Restart target: web, app, bridge, serve', 'web')
  .action(async (options) => {
    console.log(`🔄 Restarting ${options.target}...`);
    
    // Stop first
    killPorts();
    
    // Wait a moment
    await new Promise(resolve => setTimeout(resolve, 2000));
    
    // Start again
    const commands = {
      web: 'bun run web',
      app: 'bun run app',
      bridge: 'bun run bridge',
      serve: 'bun run serve'
    };
    
    const command = commands[options.target];
    if (!command) {
      console.error(`❌ Unknown restart target: ${options.target}`);
      process.exit(1);
    }
    
    console.log(`🚀 Starting ${options.target} again...`);
    try {
      await runCommandAsync(command);
    } catch (error) {
      console.error(`❌ Failed to restart ${options.target}:`, error.message);
      process.exit(1);
    }
  });

// Clean command
program
  .command('clean')
  .description('Clean build artifacts and stop processes')
  .action(() => {
    console.log('🧼 Cleaning project...');
    const result = runCommand('bun run clean');
    if (result.success) {
      console.log('✅ Project cleaned successfully!');
    } else {
      console.error('❌ Failed to clean project');
      process.exit(1);
    }
  });

// Status command
program
  .command('status')
  .description('Show current status of processes and ports')
  .action(() => {
    console.log('📊 Checking Renzora Engine status...\n');
    
    // Check if bridge binary exists
    const bridgePaths = [
      path.join(projectRoot, 'bridge', 'target', 'release', 'bridge-server.exe'),
      path.join(projectRoot, 'bridge', 'target', 'release', 'bridge-server'),
      path.join(projectRoot, 'bridge', 'target', 'debug', 'bridge-server.exe'),
      path.join(projectRoot, 'bridge', 'target', 'debug', 'bridge-server')
    ];
    
    const bridgeExists = bridgePaths.some(p => existsSync(p));
    console.log(`Bridge Binary: ${bridgeExists ? '✅ Built' : '❌ Not built'}`);
    
    // Check if dist exists
    const distExists = existsSync(path.join(projectRoot, 'dist'));
    console.log(`Web Build: ${distExists ? '✅ Built' : '❌ Not built'}`);
    
    // Check running processes
    const processesRunning = checkProcesses();
    console.log(`Processes: ${processesRunning ? '🟢 Running on ports 3000/3001' : '🔴 Not running'}`);
    
    // Check project structure
    const hasPackageJson = existsSync(path.join(projectRoot, 'package.json'));
    const hasBridgeConfig = existsSync(path.join(projectRoot, 'bridge', 'Cargo.toml'));
    const hasSrcTauri = existsSync(path.join(projectRoot, 'src-tauri', 'Cargo.toml'));
    
    console.log('\n📁 Project Structure:');
    console.log(`  Package.json: ${hasPackageJson ? '✅' : '❌'}`);
    console.log(`  Bridge Config: ${hasBridgeConfig ? '✅' : '❌'}`);
    console.log(`  Tauri Config: ${hasSrcTauri ? '✅' : '❌'}`);
    
    console.log('\n🎯 Available Commands:');
    console.log('  renzora start [web|app|bridge|serve] - Start development server');
    console.log('  renzora build [web|app|bridge] - Build project');
    console.log('  renzora stop - Stop all processes');
    console.log('  renzora restart [target] - Restart development server');
    console.log('  renzora clean - Clean build artifacts');
    console.log('  renzora status - Show this status');
  });

// Kill-port command (alias for stop)
program
  .command('kill-port')
  .description('Clear ports 3000 and 3001')
  .action(() => {
    const result = killPorts();
    if (result.success) {
      console.log('✅ Ports 3000 and 3001 cleared!');
    } else {
      console.error('❌ Failed to clear ports');
      process.exit(1);
    }
  });

// If no arguments provided, launch TUI by default
if (process.argv.length === 2) {
  const tuiPath = path.join(__dirname, 'renzora-tui.js');
  const { spawn } = await import('child_process');
  
  const child = spawn('node', [tuiPath], {
    stdio: 'inherit',
    cwd: projectRoot
  });
  
  child.on('exit', (code) => {
    process.exit(code);
  });
} else {
  program.parse();
}