/**
 * Bridge API - Unified Exports
 * Central entry point for all bridge API functionality
 */

// Configuration
export * from './config.js';

// Project Management
export * from './projects.js';

// File Operations
export * from './files.js';

// Asset Operations
export * from './assets.js';

// Thumbnail Generation
export * from './thumbnails.js';

// Server Management
export * from './server.js';

// Backwards compatibility - export a service object similar to the old BridgeService
// This can be removed once all imports are updated
import * as projects from './projects.js';
import * as files from './files.js';
import * as assets from './assets.js';
import * as thumbnails from './thumbnails.js';
import * as server from './server.js';

export const bridgeService = {
  // Project methods
  getCurrentProject: projects.getCurrentProject,
  setCurrentProject: projects.setCurrentProject,
  getProjects: projects.getProjects,
  createProject: projects.createProject,
  getProjectPath: projects.getProjectPath,
  getProjectAssetPath: projects.getProjectAssetPath,
  
  // File methods
  readFile: files.readFile,
  readBinaryFile: files.readBinaryFile,
  writeFile: files.writeFile,
  writeBinaryFile: files.writeBinaryFile,
  deleteFile: files.deleteFile,
  listDirectory: files.listDirectory,
  getFileUrl: files.getFileUrl,
  
  // Asset methods
  listAssets: assets.listAssets,
  readAssetFile: assets.readAssetFile,
  readAssetBinaryFile: assets.readAssetBinaryFile,
  writeAssetFile: assets.writeAssetFile,
  writeAssetBinaryFile: assets.writeAssetBinaryFile,
  deleteAsset: assets.deleteAsset,
  getAssetFileUrl: assets.getAssetFileUrl,
  
  // Thumbnail methods
  generateThumbnail: thumbnails.generateThumbnail,
  
  // Server methods
  getHealth: server.getHealth,
  getStartupTime: server.getStartupTime,
  restartServer: server.restartServer,
  clearCache: server.clearCache,
  isServerConnected: server.isServerConnected
};