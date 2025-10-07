/**
 * Server Management API
 * Health checks, server control, cache management
 */

import { bridgeFetch, parseJsonResponse } from './config.js';

/**
 * Check server health status
 */
export async function getHealth() {
  const response = await bridgeFetch('/health');
  return parseJsonResponse(response);
}

/**
 * Get server startup time
 */
export async function getStartupTime() {
  const response = await bridgeFetch('/startup-time');
  return parseJsonResponse(response);
}

/**
 * Restart the bridge server
 */
export async function restartServer() {
  const response = await bridgeFetch('/restart', {
    method: 'POST'
  });
  return parseJsonResponse(response);
}

/**
 * Clear server cache
 */
export async function clearCache() {
  const response = await bridgeFetch('/clear-cache', {
    method: 'POST'
  });
  return parseJsonResponse(response);
}

/**
 * Check if server is connected (simple health check)
 */
export async function isServerConnected() {
  try {
    await getHealth();
    return true;
  } catch {
    return false;
  }
}