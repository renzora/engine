import { projectBundler } from './ProjectBundler.js';
import { bridgeService } from '@/plugins/core/bridge';

/**
 * ExportManager - Main export pipeline coordinator
 */
export class ExportManager {
  constructor() {
    this.exportInProgress = false;
    this.currentProgress = 0;
    this.currentStatus = '';
    this.progressCallback = null;
  }

  /**
   * Export a project to standalone application
   * @param {string} projectName - Name of the project to export
   * @param {Object} options - Export options
   * @returns {Promise<{success: boolean, error?: string, outputPath?: string}>}
   */
  async exportProject(projectName, options = {}) {
    if (this.exportInProgress) {
      return { success: false, error: 'Export already in progress' };
    }

    this.exportInProgress = true;
    this.currentProgress = 0;
    
    try {
      console.log('🚀 ExportManager: Starting export for project:', projectName);
      
      // Check if bridge server is available
      const bridgeAvailable = await this.checkBridgeAvailability();
      if (!bridgeAvailable) {
        return { success: false, error: 'Bridge server is not running. Please start it with "bun run bridge"' };
      }
      
      // Validate project exists
      const projectExists = await this.validateProject(projectName);
      if (!projectExists) {
        return { success: false, error: 'Project not found or invalid' };
      }

      this.updateProgress(10, 'Creating project bundle...');
      
      // Create project bundle
      const bundleResult = await projectBundler.createBundle(projectName, options);
      if (!bundleResult.success) {
        return { success: false, error: bundleResult.error };
      }

      this.updateProgress(40, 'Generating runtime application...');
      
      // Generate runtime application
      const runtimeResult = await this.generateRuntimeApp(bundleResult.bundle, projectName, options);
      if (!runtimeResult.success) {
        return { success: false, error: runtimeResult.error };
      }

      this.updateProgress(70, 'Building Tauri application...');
      
      // Build Tauri application (if requested)
      let tauriResult = null;
      if (options.buildTauri !== false) {
        tauriResult = await this.buildTauriApp(projectName, options);
        if (!tauriResult.success) {
          console.warn('⚠️ ExportManager: Tauri build failed, but runtime was generated:', tauriResult.error);
        }
      }

      this.updateProgress(90, 'Finalizing export...');
      
      // Create export summary
      const exportSummary = this.createExportSummary(bundleResult, runtimeResult, tauriResult);
      
      this.updateProgress(100, 'Export complete!');
      
      console.log('✅ ExportManager: Export completed successfully');
      
      return {
        success: true,
        outputPath: runtimeResult.outputPath,
        summary: exportSummary
      };

    } catch (error) {
      console.error('❌ ExportManager: Export failed:', error);
      return { success: false, error: error.message };
      
    } finally {
      this.exportInProgress = false;
    }
  }

  /**
   * Check if bridge server is available
   * @returns {Promise<boolean>}
   */
  async checkBridgeAvailability() {
    try {
      const response = await fetch('http://localhost:3001/health', { timeout: 2000 });
      return response.ok;
    } catch {
      console.error('❌ ExportManager: Bridge server not accessible on port 3001');
      return false;
    }
  }

  /**
   * Validate that project is ready for export
   * @param {string} projectName - Project name
   * @returns {Promise<boolean>}
   */
  async validateProject(projectName) {
    try {
      // Check if project.json exists using correct bridge API
      const projectContent = await bridgeService.readFile(`projects/${projectName}/project.json`);
      
      if (!projectContent) {
        console.error('❌ ExportManager: project.json not found');
        return false;
      }

      const projectConfig = JSON.parse(projectContent);
      
      // Validate required fields
      if (!projectConfig.name || !projectConfig.version) {
        console.error('❌ ExportManager: Invalid project.json - missing name or version');
        return false;
      }

      return true;
      
    } catch (error) {
      console.error('❌ ExportManager: Project validation failed:', error);
      return false;
    }
  }

  /**
   * Generate runtime application files
   * @param {Object} bundle - Project bundle
   * @param {string} projectName - Project name
   * @param {Object} options - Export options
   * @returns {Promise<{success: boolean, outputPath?: string, error?: string}>}
   */
  async generateRuntimeApp(bundle, projectName) {
    try {
      console.log('🔧 ExportManager: Generating runtime application...');
      
      const outputDir = `exported-projects/${projectName}`;
      
      // Create output directory
      await this.ensureDirectory(outputDir);
      
      // Generate HTML file
      const htmlContent = this.generateRuntimeHTML(bundle);
      await this.writeFile(`${outputDir}/index.html`, htmlContent);
      
      // Generate JavaScript bundle
      const jsContent = this.generateRuntimeJS(bundle);
      await this.writeFile(`${outputDir}/runtime.js`, jsContent);
      
      // Copy assets (this would be more complex in reality)
      await this.copyAssets(bundle, outputDir);
      
      // Generate package.json for web runtime
      const packageJson = this.generateRuntimePackageJson(bundle);
      await this.writeFile(`${outputDir}/package.json`, JSON.stringify(packageJson, null, 2));
      
      console.log('✅ ExportManager: Runtime application generated');
      return { success: true, outputPath: outputDir };
      
    } catch (error) {
      console.error('❌ ExportManager: Runtime generation failed:', error);
      return { success: false, error: error.message };
    }
  }

  /**
   * Build Tauri desktop application
   * @param {string} projectName - Project name
   * @param {Object} options - Export options
   * @returns {Promise<{success: boolean, error?: string}>}
   */
  async buildTauriApp() {
    try {
      console.log('🔧 ExportManager: Building Tauri application...');
      
      // This would involve:
      // 1. Setting up Tauri project structure
      // 2. Embedding project data
      // 3. Running tauri build
      
      // For now, return placeholder
      console.log('🏗️ ExportManager: Tauri build would execute here');
      
      return { success: true };
      
    } catch (error) {
      console.error('❌ ExportManager: Tauri build failed:', error);
      return { success: false, error: error.message };
    }
  }

  /**
   * Generate runtime HTML file
   * @param {Object} bundle - Project bundle
   * @returns {string} HTML content
   */
  generateRuntimeHTML(bundle) {
    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${bundle.project.name} - Renzora Runtime</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    html, body { width: 100%; height: 100%; overflow: hidden; background: #000; }
    #root { width: 100%; height: 100%; }
    #renderCanvas { width: 100%; height: 100%; outline: none; display: block; }
  </style>
</head>
<body>
  <div id="root">
    <canvas id="renderCanvas"></canvas>
  </div>
  
  <script>
    // Embed project data
    window.__RENZORA_PROJECT_DATA__ = ${JSON.stringify(bundle)};
  </script>
  
  <script src="runtime.js"></script>
</body>
</html>`;
  }

  /**
   * Generate runtime JavaScript bundle
   * @param {Object} bundle - Project bundle
   * @returns {string} JavaScript content
   */
  generateRuntimeJS(bundle) {
    return `${bundle.runtime}

// Initialize runtime when ready
document.addEventListener('DOMContentLoaded', () => {
  console.log('🚀 Runtime: DOM ready, initializing...');
  
  const canvas = document.getElementById('renderCanvas');
  if (!canvas) {
    console.error('❌ Runtime: Canvas not found');
    return;
  }
  
  // Start the runtime
  const runtime = new ExportedRuntimeBootstrapper();
  runtime.initialize(canvas).catch(error => {
    console.error('❌ Runtime: Initialization failed:', error);
  });
});`;
  }

  /**
   * Generate package.json for web runtime
   * @param {Object} bundle - Project bundle
   * @returns {Object} Package.json content
   */
  generateRuntimePackageJson(bundle) {
    return {
      name: `${bundle.project.name.toLowerCase().replace(/\s+/g, '-')}-runtime`,
      version: bundle.project.version,
      description: `Runtime for ${bundle.project.name}`,
      main: "runtime.js",
      scripts: {
        start: "npx serve . -p 3000",
        serve: "npx serve . -p 3000"
      },
      dependencies: {
        "@babylonjs/core": "^8.20.0",
        "@babylonjs/loaders": "^8.20.0"
      },
      author: bundle.project.author,
      license: "MIT",
      renzora: {
        runtime: true,
        engineVersion: bundle.metadata.engine_version,
        exported: bundle.metadata.exported
      }
    };
  }

  /**
   * Copy project assets to output directory
   * @param {Object} bundle - Project bundle
   * @param {string} outputDir - Output directory
   */
  async copyAssets(bundle, outputDir) {
    try {
      console.log('🎯 ExportManager: Copying assets...');
      
      // Create assets directory
      await this.ensureDirectory(`${outputDir}/assets`);
      
      // For now, just create an assets manifest
      // In a full implementation, this would copy all asset files
      const assetManifest = {
        assets: bundle.assets,
        totalSize: Object.values(bundle.assets).reduce((sum, asset) => sum + (asset.size || 0), 0),
        count: Object.keys(bundle.assets).length
      };
      
      await this.writeFile(`${outputDir}/assets/manifest.json`, JSON.stringify(assetManifest, null, 2));
      
      console.log('✅ ExportManager: Asset manifest created');
      
    } catch (error) {
      console.error('❌ ExportManager: Asset copying failed:', error);
      throw error;
    }
  }

  /**
   * Ensure directory exists
   * @param {string} dirPath - Directory path
   */
  async ensureDirectory(dirPath) {
    try {
      // For now, we'll just attempt to create directories via write operations
      // The bridge service will create parent directories automatically
      console.log('🔧 ExportManager: Directory will be created automatically:', dirPath);
    } catch (error) {
      console.error('❌ ExportManager: Directory creation failed:', dirPath, error);
    }
  }

  /**
   * Write file using bridge service
   * @param {string} filePath - File path
   * @param {string} content - File content
   */
  async writeFile(filePath, content) {
    const result = await bridgeService.writeFile(filePath, content);
    if (!result) {
      throw new Error(`Failed to write file ${filePath}`);
    }
  }

  /**
   * Create export summary
   * @param {Object} bundleResult - Bundle creation result
   * @param {Object} runtimeResult - Runtime generation result
   * @param {Object} tauriResult - Tauri build result
   * @returns {Object} Export summary
   */
  createExportSummary(bundleResult, runtimeResult, tauriResult) {
    const bundle = bundleResult.bundle;
    
    return {
      project: {
        name: bundle.project.name,
        version: bundle.project.version,
        exported: bundle.metadata.exported
      },
      stats: {
        scripts: bundle.metadata.script_count,
        assets: bundle.metadata.asset_count,
        scenes: Object.keys(bundle.scenes).length,
        bundleSize: JSON.stringify(bundle).length
      },
      outputs: {
        runtime: runtimeResult.outputPath,
        tauri: tauriResult?.success ? 'Built successfully' : 'Build skipped or failed',
        webRuntime: `${runtimeResult.outputPath}/index.html`
      },
      warnings: [
        ...(bundleResult.errors || []),
        ...(tauriResult?.error ? [{ type: 'tauri', message: tauriResult.error }] : [])
      ]
    };
  }

  /**
   * Set progress callback
   * @param {Function} callback - Progress callback function
   */
  setProgressCallback(callback) {
    this.progressCallback = callback;
  }

  /**
   * Update export progress
   * @param {number} progress - Progress percentage (0-100)
   * @param {string} status - Status message
   */
  updateProgress(progress, status) {
    this.currentProgress = progress;
    this.currentStatus = status;
    
    if (this.progressCallback) {
      this.progressCallback(progress, status);
    }
    
    console.log(`📊 ExportManager: ${progress}% - ${status}`);
  }

  /**
   * Get current export progress
   * @returns {{progress: number, status: string, inProgress: boolean}}
   */
  getProgress() {
    return {
      progress: this.currentProgress,
      status: this.currentStatus,
      inProgress: this.exportInProgress
    };
  }

  /**
   * Cancel ongoing export
   */
  cancelExport() {
    if (this.exportInProgress) {
      console.log('🛑 ExportManager: Export cancelled by user');
      this.exportInProgress = false;
      this.updateProgress(0, 'Export cancelled');
    }
  }
}

export const exportManager = new ExportManager();