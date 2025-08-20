/**
 * Script API - Main exports for the script system
 */

export { ScriptManager } from './ScriptManager.js';
export { ScriptAPI } from './ScriptAPI.js';
export { ScriptLoader, getScriptLoader } from './ScriptLoader.js';
export { ScriptRuntime, getScriptRuntime, initializeScriptRuntime } from './ScriptRuntime.js';

// Convenience re-exports
export {
  getScriptRuntime as getRuntime,
  initializeScriptRuntime as initRuntime
} from './ScriptRuntime.js';