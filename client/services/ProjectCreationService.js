class ProjectCreationService {
  constructor() {
    this.isElectron = (typeof window !== 'undefined' && window.electronAPI?.isElectron) || false;
  }

  async createProject(projectName) {
    if (!projectName || typeof projectName !== 'string') {
      throw new Error('Project name is required');
    }

    const validNamePattern = /^[a-zA-Z0-9_-]+$/;
    if (!validNamePattern.test(projectName.trim())) {
      throw new Error('Project name can only contain letters, numbers, underscores, and hyphens');
    }

    const sanitizedName = projectName.trim();

    if (this.isElectron) {
      return this.createProjectElectron(sanitizedName);
    } else {
      return this.createProjectServer(sanitizedName);
    }
  }

  async createProjectElectron(projectName) {
    try {
      console.log(`Creating project via Electron: ${projectName}`);
      
      if (typeof window === 'undefined' || !window.fileSystemAPI?.createProject) {
        throw new Error('Electron file system API not available');
      }

      const result = await window.fileSystemAPI.createProject(projectName);
      
      if (!result.success) {
        throw new Error('Failed to create project via Electron');
      }

      console.log(`✅ Electron project created: ${result.projectPath}`);
      
      return {
        success: true,
        projectPath: result.projectPath,
        projectName: projectName,
        fullPath: result.fullPath
      };

    } catch (error) {
      console.error('Electron project creation failed:', error);
      throw new Error(`Electron project creation failed: ${error.message}`);
    }
  }

  async createProjectServer(projectName) {
    try {
      console.log(`Creating project via server: ${projectName}`);
      
      const response = await fetch('/api/projects/create', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ projectName })
      });

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.error || 'Server project creation failed');
      }

      const result = await response.json();
      
      console.log(`✅ Server project created: ${result.projectPath}`);
      
      return {
        success: true,
        projectPath: result.projectPath,
        projectName: result.projectName || projectName
      };

    } catch (error) {
      console.error('Server project creation failed:', error);
      throw new Error(`Server project creation failed: ${error.message}`);
    }
  }

  async listProjects() {
    console.log(`Listing projects in ${this.getEnvironment()} environment`);
    
    if (this.isElectron && typeof window !== 'undefined' && window.fileSystemAPI?.listProjects) {
      try {
        console.log('Using Electron file system API to list projects');
        const result = await window.fileSystemAPI.listProjects();
        console.log('Electron projects result:', result);
        return result.projects || [];
      } catch (error) {
        console.warn('Failed to list projects via Electron, falling back to server:', error);
      }
    }

    try {
      console.log('Using server API to list projects');
      const response = await fetch('/api/projects');
      if (response.ok) {
        const data = await response.json();
        console.log('Server projects result:', data);
        return data.projects || [];
      }
    } catch (error) {
      console.warn('Failed to list projects via server:', error);
    }

    return [];
  }

  async projectExists(projectName) {
    try {
      const projects = await this.listProjects();
      return projects.some(project => project.name === projectName || project.path === projectName);
    } catch (error) {
      console.warn('Failed to check if project exists:', error);
      return false;
    }
  }

  getEnvironment() {
    return this.isElectron ? 'electron' : 'server';
  }
}

let _instance = null;
export const projectCreationService = {
  getInstance() {
    if (!_instance) {
      _instance = new ProjectCreationService();
    }
    return _instance;
  },
  
  async createProject(projectName) {
    return this.getInstance().createProject(projectName);
  },
  
  async listProjects() {
    return this.getInstance().listProjects();
  },
  
  async projectExists(projectName) {
    return this.getInstance().projectExists(projectName);
  },
  
  getEnvironment() {
    return this.getInstance().getEnvironment();
  }
};