/**
 * Asset Operations API
 * Asset-specific convenience methods that wrap file operations
 */

import { getProjectAssetPath } from './projects.js';
import { 
  readFile, 
  readBinaryFile, 
  writeFile, 
  writeBinaryFile, 
  deleteFile, 
  listDirectory, 
  getFileUrl 
} from './files.js';

/**
 * List assets in the current project
 */
export async function listAssets(assetPath = '') {
  const fullPath = getProjectAssetPath(assetPath);
  return listDirectory(fullPath);
}

/**
 * Read an asset text file
 */
export async function readAssetFile(assetPath) {
  const fullPath = getProjectAssetPath(assetPath);
  return readFile(fullPath);
}

/**
 * Read an asset binary file as base64
 */
export async function readAssetBinaryFile(assetPath) {
  const fullPath = getProjectAssetPath(assetPath);
  return readBinaryFile(fullPath);
}

/**
 * Write an asset text file
 */
export async function writeAssetFile(assetPath, content) {
  const fullPath = getProjectAssetPath(assetPath);
  return writeFile(fullPath, content);
}

/**
 * Write an asset binary file from base64
 */
export async function writeAssetBinaryFile(assetPath, base64Content) {
  const fullPath = getProjectAssetPath(assetPath);
  return writeBinaryFile(fullPath, base64Content);
}

/**
 * Delete an asset file or directory
 */
export async function deleteAsset(assetPath) {
  const fullPath = getProjectAssetPath(assetPath);
  return deleteFile(fullPath);
}

/**
 * Get direct URL for asset file
 */
export function getAssetFileUrl(assetPath) {
  const fullPath = getProjectAssetPath(assetPath);
  return getFileUrl(fullPath);
}