#!/usr/bin/env node

import inquirer from 'inquirer';
import chalk from 'chalk';
import ora from 'ora';
import boxen from 'boxen';
import { execSync, exec, spawn } from 'child_process';
import { existsSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { createInterface } from 'readline';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.join(__dirname, '..');

// Global state for running processes
let runningProcesses = {
  web: null,
  app: null,
  bridge: null,
  serve: null
};

let serverLogs = [];
const MAX_LOGS = 50;

// Helper functions
function clearScreen() {
  console.clear();
}

function createPromptWithEscapeHandling(promptConfig, onEscape = null) {
  return inquirer.prompt([{
    ...promptConfig,
    theme: {
      prefix: '',
      helpMode: 'never',
      ...promptConfig.theme
    }
  }]).catch((error) => {
    if (error.isTtyError || error.name === 'ExitPromptError') {
      if (onEscape) {
        onEscape();
      } else {
        // Default: return to previous menu
        return { [promptConfig.name]: 'back' };
      }
    }
    throw error;
  });
}

function showHeader() {
  const logo = chalk.bold.magenta(`
  ██████╗ ███████╗███╗   ██╗███████╗ ██████╗ ██████╗  █████╗ 
  ██╔══██╗██╔════╝████╗  ██║╚══███╔╝██╔═══██╗██╔══██╗██╔══██╗
  ██████╔╝█████╗  ██╔██╗ ██║  ███╔╝ ██║   ██║██████╔╝███████║
  ██╔══██╗██╔══╝  ██║╚██╗██║ ███╔╝  ██║   ██║██╔══██╗██╔══██║
  ██║  ██║███████╗██║ ╚████║███████╗╚██████╔╝██║  ██║██║  ██║
  ╚═╝  ╚═╝╚══════╝╚═╝  ╚═══╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝
  `);
  
  const subtitle = chalk.dim.cyan('        █ Select an action (Arrow keys • ESC to go back) █        ');
  
  console.log('\n' + logo);
  console.log(subtitle + '\n');
}

function runCommand(command, options = {}) {
  try {
    const result = execSync(command, { 
      cwd: projectRoot, 
      stdio: 'inherit',
      ...options 
    });
    return { success: true, result };
  } catch (error) {
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

function startServerBackground(target) {
  if (runningProcesses[target]) {
    console.log(chalk.yellow(`${target} server is already running`));
    return runningProcesses[target];
  }

  const commands = {
    web: 'bun run web',
    app: 'bun run app',
    bridge: 'bun run bridge',
    serve: 'bun run serve'
  };

  const command = commands[target];
  if (!command) {
    console.log(chalk.red(`Unknown target: ${target}`));
    return null;
  }

  console.log(chalk.cyan(`🚀 Starting ${target} server in background...`));
  
  const child = spawn('bun', ['run', target === 'web' ? 'web' : target === 'app' ? 'app' : target === 'bridge' ? 'bridge' : 'serve'], {
    cwd: projectRoot,
    stdio: ['ignore', 'pipe', 'pipe'],
    detached: false
  });

  runningProcesses[target] = {
    process: child,
    pid: child.pid,
    target,
    startTime: new Date()
  };

  // Capture logs but don't spam console
  child.stdout?.on('data', (data) => {
    const logEntry = {
      target,
      type: 'stdout',
      message: data.toString().trim(),
      timestamp: new Date()
    };
    serverLogs.push(logEntry);
    if (serverLogs.length > MAX_LOGS) {
      serverLogs.shift();
    }
  });

  child.stderr?.on('data', (data) => {
    const logEntry = {
      target,
      type: 'stderr', 
      message: data.toString().trim(),
      timestamp: new Date()
    };
    serverLogs.push(logEntry);
    if (serverLogs.length > MAX_LOGS) {
      serverLogs.shift();
    }
  });

  child.on('exit', (code) => {
    console.log(chalk.gray(`${target} server exited with code ${code}`));
    runningProcesses[target] = null;
  });

  child.on('error', (error) => {
    console.log(chalk.red(`${target} server error: ${error.message}`));
    runningProcesses[target] = null;
  });

  return runningProcesses[target];
}

function stopServer(target) {
  if (!runningProcesses[target]) {
    console.log(chalk.yellow(`${target} server is not running`));
    return false;
  }

  try {
    console.log(chalk.cyan(`🛑 Stopping ${target} server...`));
    runningProcesses[target].process.kill();
    runningProcesses[target] = null;
    return true;
  } catch (error) {
    console.log(chalk.red(`Failed to stop ${target}: ${error.message}`));
    return false;
  }
}

function getRunningServers() {
  return Object.entries(runningProcesses)
    .filter(([_, proc]) => proc !== null)
    .map(([target, proc]) => ({ target, ...proc }));
}

function killPorts() {
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

function getProjectStatus() {
  const bridgePaths = [
    path.join(projectRoot, 'bridge', 'target', 'release', 'bridge-server.exe'),
    path.join(projectRoot, 'bridge', 'target', 'release', 'bridge-server'),
    path.join(projectRoot, 'bridge', 'target', 'debug', 'bridge-server.exe'),
    path.join(projectRoot, 'bridge', 'target', 'debug', 'bridge-server')
  ];
  
  const bridgeExists = bridgePaths.some(p => existsSync(p));
  const distExists = existsSync(path.join(projectRoot, 'dist'));
  const processesRunning = checkProcesses();
  const hasPackageJson = existsSync(path.join(projectRoot, 'package.json'));
  const hasBridgeConfig = existsSync(path.join(projectRoot, 'bridge', 'Cargo.toml'));
  const hasSrcTauri = existsSync(path.join(projectRoot, 'src-tauri', 'Cargo.toml'));
  const runningServers = getRunningServers();

  return {
    bridgeExists,
    distExists,
    processesRunning,
    hasPackageJson,
    hasBridgeConfig,
    hasSrcTauri,
    runningServers
  };
}

function showStatus() {
  const status = getProjectStatus();
  
  // Simplified status display
  const buildStatus = status.bridgeExists && status.distExists ? 
    chalk.green('■ Ready') : chalk.yellow('■ Building Required');
  
  const serverStatus = status.runningServers.length > 0 ? 
    chalk.green(`■ ${status.runningServers.length} Active`) : chalk.gray('■ Stopped');
  
  let runningInfo = '';
  if (status.runningServers.length > 0) {
    const servers = status.runningServers.map(s => {
      const uptime = Math.floor((new Date() - s.startTime) / 1000);
      return `${chalk.cyan(s.target)} (${uptime}s)`;
    }).join(' • ');
    runningInfo = `\n   ${chalk.dim('Running:')} ${servers}`;
  }
  
  console.log(chalk.dim('┌────────────────────────────────────────────────────────┐'));
  
  const buildText = ` ${chalk.bold('Build:')} ${buildStatus}     ${chalk.bold('Servers:')} ${serverStatus}`;
  const buildTextClean = buildText.replace(/\u001b\[[0-9;]*m/g, '');
  const buildPadding = ' '.repeat(Math.max(0, 56 - buildTextClean.length));
  console.log(chalk.dim('│') + buildText + buildPadding + chalk.dim('│'));
  
  if (runningInfo) {
    const runningTextClean = runningInfo.replace(/\u001b\[[0-9;]*m/g, '');
    const runningPadding = ' '.repeat(Math.max(0, 56 - runningTextClean.length));
    console.log(chalk.dim('│') + runningInfo + runningPadding + chalk.dim('│'));
  }
  
  console.log(chalk.dim('└────────────────────────────────────────────────────────┘\n'));
}

async function buildMenu() {
  clearScreen();
  showHeader();
  
  const { target } = await createPromptWithEscapeHandling({
    type: 'list',
    name: 'target',
    message: '',
    choices: [
      { name: chalk.cyan(' Web App          ') + chalk.dim('Optimized frontend build'), value: 'web' },
      { name: chalk.green(' Desktop App      ') + chalk.dim('Tauri executable'), value: 'app' },
      { name: chalk.yellow(' Bridge           ') + chalk.dim('Rust backend server'), value: 'bridge' },
      { name: chalk.gray(' ← Back           ') + chalk.dim('Return to main menu'), value: 'back' }
    ],
    pageSize: 10
  });

  if (target === 'back') return;

  const spinner = ora(chalk.yellow(`Building ${target}...`)).start();
  
  const commands = {
    web: 'bun run build:web',
    app: 'bun run build:app', 
    bridge: 'bun run build:bridge'
  };
  
  try {
    const result = runCommand(commands[target]);
    if (result.success) {
      spinner.succeed(chalk.green(`${target} build completed`));
    } else {
      spinner.fail(chalk.red(`${target} build failed`));
    }
  } catch (error) {
    spinner.fail(`Build failed: ${error.message}`);
  }
  
  await inquirer.prompt([{ type: 'input', name: 'continue', message: 'Press Enter to continue...' }]);
}

async function startMenu() {
  clearScreen();
  showHeader();
  
  const { target } = await createPromptWithEscapeHandling({
    type: 'list',
    name: 'target',
    message: '',
    choices: [
      { name: chalk.cyan(' Web App          ') + chalk.dim('Frontend + Bridge (Rspack)'), value: 'web' },
      { name: chalk.green(' Desktop App      ') + chalk.dim('Tauri application'), value: 'app' },
      { name: chalk.blue(' Static Server    ') + chalk.dim('Serve built files'), value: 'serve' },
      { name: chalk.gray(' ← Back           ') + chalk.dim('Return to main menu'), value: 'back' }
    ],
    pageSize: 10
  });

  if (target === 'back') return;

  // Start server in background
  const spinner = ora(chalk.cyan(`Starting ${target} server...`)).start();
  const server = startServerBackground(target);
  
  if (!server) {
    spinner.fail('Failed to start server');
    return;
  }

  // Wait a moment for server to start
  await new Promise(resolve => setTimeout(resolve, 3000));
  
  spinner.succeed(chalk.green(`${target} server started`));
  console.log(chalk.dim('  ■ Server running in background\n'));
  
  await inquirer.prompt([{ type: 'input', name: 'continue', message: 'Press Enter to continue...' }]);
}

async function processMenu() {
  clearScreen();
  showHeader();
  showStatus();
  
  const runningServers = getRunningServers();
  const choices = [
    { name: chalk.red(' Stop All Processes ') + chalk.dim('Terminate all running servers'), value: 'stop_all' },
    { name: chalk.yellow(' Clean Build        ') + chalk.dim('Remove build artifacts'), value: 'clean' },
    { name: chalk.blue(' Clear Ports        ') + chalk.dim('Kill processes on ports 3000/3001'), value: 'kill_ports' },
    { name: chalk.green(' View Server Logs   ') + chalk.dim('Monitor process output'), value: 'logs' }
  ];

  // Add individual server controls
  if (runningServers.length > 0) {
    runningServers.forEach(server => {
      choices.push({ name: chalk.red(` Stop ${server.target} server  `) + chalk.dim('Terminate individual process'), value: `stop_${server.target}` });
      choices.push({ name: chalk.cyan(` Restart ${server.target}     `) + chalk.dim('Restart individual process'), value: `restart_${server.target}` });
    });
  }

  choices.push({ name: chalk.gray(' ← Back           ') + chalk.dim('Return to main menu'), value: 'back' });

  const { action } = await createPromptWithEscapeHandling({
    type: 'list',
    name: 'action',
    message: '',
    choices
  });

  if (action === 'back') return;

  const spinner = ora('Processing...').start();

  try {
    if (action === 'stop_all') {
      Object.keys(runningProcesses).forEach(target => {
        if (runningProcesses[target]) {
          stopServer(target);
        }
      });
      killPorts();
      spinner.succeed('All processes stopped and ports cleared!');
    } else if (action.startsWith('stop_')) {
      const target = action.replace('stop_', '');
      stopServer(target);
      spinner.succeed(`${target} server stopped!`);
    } else if (action.startsWith('restart_')) {
      const target = action.replace('restart_', '');
      stopServer(target);
      await new Promise(resolve => setTimeout(resolve, 2000));
      startServerBackground(target);
      spinner.succeed(`${target} server restarted!`);
    } else if (action === 'clean') {
      runCommand('bun run clean');
      spinner.succeed('Project cleaned successfully!');
    } else if (action === 'kill_ports') {
      killPorts();
      spinner.succeed('Ports 3000 and 3001 cleared!');
    } else if (action === 'logs') {
      spinner.stop();
      await showLogs();
      return;
    }
  } catch (error) {
    spinner.fail(`Failed: ${error.message}`);
  }
  
  await inquirer.prompt([{ type: 'input', name: 'continue', message: 'Press Enter to continue...' }]);
}

async function showLogs() {
  clearScreen();
  showHeader();
  
  console.log(chalk.dim('┌────────────────────────────────────────────────────────┐'));
  console.log(chalk.dim('│') + chalk.bold.white('  Server Logs') + ' '.repeat(43) + chalk.dim('│'));
  console.log(chalk.dim('└────────────────────────────────────────────────────────┘\n'));

  if (serverLogs.length === 0) {
    console.log(chalk.dim('  No logs available yet. Start a server to see logs here.\n'));
  } else {
    serverLogs.slice(-15).forEach(log => {
      const time = log.timestamp.toLocaleTimeString();
      const color = log.type === 'stderr' ? chalk.red : chalk.white;
      const targetColor = log.target === 'web' ? chalk.cyan : 
                         log.target === 'app' ? chalk.green :
                         log.target === 'bridge' ? chalk.yellow : chalk.blue;
      
      console.log(`  ${chalk.dim(time)} ${targetColor(log.target.padEnd(6))} ${color(log.message.slice(0, 60))}`);
    });
    console.log('');
  }

  await inquirer.prompt([{ type: 'input', name: 'continue', message: 'Press Enter to continue...' }]);
}

async function mainMenu() {
  while (true) {
    clearScreen();
    showHeader();
    showStatus();
    
    const runningServers = getRunningServers();
    
    // Dynamic choices based on server state
    const choices = [];
    
    if (runningServers.length === 0) {
      choices.push({ name: chalk.cyan(' Start Server    ') + chalk.dim('Launch development environment'), value: 'start' });
    } else {
      choices.push({ name: chalk.green(' Servers Running ') + chalk.dim('Manage active processes'), value: 'process' });
      choices.push({ name: chalk.blue(' View Logs       ') + chalk.dim('Monitor server output'), value: 'logs' });
    }
    
    choices.push({ name: chalk.yellow(' Build Project   ') + chalk.dim('Compile and package application'), value: 'build' });
    
    if (runningServers.length > 0) {
      choices.push({ name: chalk.cyan(' Start More      ') + chalk.dim('Launch additional servers'), value: 'start' });
    }

    choices.push(
      { name: chalk.red(' Exit            ') + chalk.dim('Shutdown and cleanup'), value: 'exit' }
    );

    const { action } = await inquirer.prompt([
      {
        type: 'list',
        name: 'action',
        message: '',
        choices,
        pageSize: 10,
        loop: false,
        theme: {
          prefix: '',
          helpMode: 'never'
        },
        validate: (input, answers) => {
          // Handle ESC key
          return true;
        }
      }
    ]).catch((error) => {
      if (error.isTtyError || error.name === 'ExitPromptError') {
        // ESC was pressed, exit gracefully
        console.log(chalk.magenta('\n  ■ Goodbye!\n'));
        process.exit(0);
      }
      throw error;
    });

    switch (action) {
      case 'start':
        await startMenu();
        break;
      case 'build':
        await buildMenu();
        break;
      case 'process':
        await processMenu();
        break;
      case 'logs':
        await showLogs();
        break;
      case 'exit':
        // Clean shutdown - stop all running servers
        const running = getRunningServers();
        if (running.length > 0) {
          console.log(chalk.yellow('\n  ■ Stopping running servers...'));
          running.forEach(server => stopServer(server.target));
          await new Promise(resolve => setTimeout(resolve, 1000));
        }
        console.log(chalk.magenta('\n  ■ Goodbye!\n'));
        process.exit(0);
        break;
    }
  }
}

// Handle Ctrl+C gracefully
process.on('SIGINT', () => {
  console.log(chalk.yellow('\n\n🛑 Interrupted. Returning to main menu...'));
  setTimeout(() => {
    mainMenu();
  }, 1000);
});

// Start the TUI
mainMenu().catch(error => {
  console.error(chalk.red('Error:', error.message));
  process.exit(1);
});