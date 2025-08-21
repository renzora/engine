import { createSignal } from 'solid-js';

// Project state management
const [currentProject, setCurrentProject] = createSignal(null);

export class BridgeService {
  constructor() {
    // Standalone bridge server on port 3001
    this.apiPrefix = 'http://localhost:3001';
  }

  // Project management
  getCurrentProject() {
    return currentProject();
  }

  setCurrentProject(project) {
    setCurrentProject(project);
  }

  // Centralized path construction for assets
  getProjectAssetPath(assetPath = '', projectName = null) {
    const project = projectName || currentProject()?.name;
    if (!project) {
      throw new Error('No current project set');
    }
    
    if (assetPath) {
      return `projects/${project}/assets/${assetPath}`;
    }
    return `projects/${project}/assets`;
  }

  // Centralized path construction for project files (non-assets)
  getProjectPath(filePath = '', projectName = null) {
    const project = projectName || currentProject()?.name;
    if (!project) {
      throw new Error('No current project set');
    }
    
    if (filePath) {
      return `projects/${project}/${filePath}`;
    }
    return `projects/${project}`;
  }

  async getProjects() {
    const response = await fetch(`${this.apiPrefix}/projects`);
    if (!response.ok) throw new Error('Failed to fetch projects');
    return response.json();
  }

  async readFile(path) {
    const response = await fetch(`${this.apiPrefix}/read/${path}`);
    if (!response.ok) throw new Error('Failed to read file');
    const data = await response.json();
    return data.content;
  }

  async readBinaryFile(path) {
    const response = await fetch(`${this.apiPrefix}/file/${path}`);
    if (!response.ok) throw new Error('Failed to read binary file');
    const blob = await response.blob();
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => {
        const result = reader.result;
        if (typeof result === 'string' && result.includes(',')) {
          resolve(result.split(',')[1]); // Remove data:mime;base64, prefix
        } else {
          reject(new Error('Failed to convert blob to base64'));
        }
      };
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(blob);
    });
  }

  async writeFile(path, content) {
    const response = await fetch(`${this.apiPrefix}/write/${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ content })
    });
    if (!response.ok) throw new Error('Failed to write file');
    return response.json();
  }

  async writeBinaryFile(path, base64Content) {
    const response = await fetch(`${this.apiPrefix}/write-binary/${path}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ base64_content: base64Content })
    });
    if (!response.ok) throw new Error('Failed to write binary file');
    return response.json();
  }

  async deleteFile(path) {
    const response = await fetch(`${this.apiPrefix}/delete/${path}`, {
      method: 'DELETE'
    });
    if (!response.ok) throw new Error('Failed to delete file');
    return response.json();
  }

  async listDirectory(path = '') {
    const response = await fetch(`${this.apiPrefix}/list/${path}`);
    if (!response.ok) throw new Error('Failed to list directory');
    return response.json();
  }

  getFileUrl(path) {
    return `${this.apiPrefix}/file/${path}`;
  }

  // Asset-specific convenience methods
  async listAssets(assetPath = '') {
    const fullPath = this.getProjectAssetPath(assetPath);
    return this.listDirectory(fullPath);
  }

  async readAssetFile(assetPath) {
    const fullPath = this.getProjectAssetPath(assetPath);
    return this.readFile(fullPath);
  }

  async readAssetBinaryFile(assetPath) {
    const fullPath = this.getProjectAssetPath(assetPath);
    return this.readBinaryFile(fullPath);
  }

  async writeAssetFile(assetPath, content) {
    const fullPath = this.getProjectAssetPath(assetPath);
    return this.writeFile(fullPath, content);
  }

  async writeAssetBinaryFile(assetPath, base64Content) {
    const fullPath = this.getProjectAssetPath(assetPath);
    return this.writeBinaryFile(fullPath, base64Content);
  }

  async deleteAsset(assetPath) {
    const fullPath = this.getProjectAssetPath(assetPath);
    return this.deleteFile(fullPath);
  }

  getAssetFileUrl(assetPath) {
    const fullPath = this.getProjectAssetPath(assetPath);
    return this.getFileUrl(fullPath);
  }

  // Generate thumbnail for 3D models
  async generateThumbnail(assetPath, size = 512) {
    const project = currentProject();
    if (!project?.name) {
      throw new Error('No current project set');
    }

    const response = await fetch(`${this.apiPrefix}/thumbnail`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        project_name: project.name,
        asset_path: `assets/${assetPath}`,
        size: size
      })
    });

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    return response.json();
  }

}

export const bridgeService = new BridgeService();
export { default as BridgeStatus } from './BridgeStatus.jsx';
export { default as BridgeViewport } from './BridgeViewport.jsx';
export { default as BridgePlugin } from './BridgePlugin.jsx';
export { default } from './BridgePluginClass.jsx';