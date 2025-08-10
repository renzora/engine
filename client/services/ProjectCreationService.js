class ProjectCreationService {
  constructor() {
    // Removed Electron support
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
    return this.createProjectServer(sanitizedName);
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
    try {
      const response = await fetch('/api/projects');
      if (response.ok) {
        const data = await response.json();
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
    return 'server';
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