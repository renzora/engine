class ProjectManager {
  constructor() {
    this.currentProject = null;
    this.initialized = false;
    this.fileChangeListeners = new Set();
    this.loadingListeners = new Set();
  }

  async initializeDefaultProject(projectName = null) {
    console.log('Initializing project:', projectName || 'default');
    this.currentProject = {
      name: projectName || 'Default Project',
      path: projectName ? `/projects/${projectName}` : '/projects/default',
      created: new Date()
    };
    this.initialized = true;
    return this.currentProject;
  }

  async loadProject(projectPath) {
    console.log(`Loading project from: ${projectPath}`);
    this.currentProject = {
      name: projectPath, // Use the project path directly as the name
      path: projectPath,
      loaded: new Date()
    };
    this.initialized = true;
    
    // Notify loading listeners
    this.loadingListeners.forEach(listener => {
      try {
        listener(this.currentProject);
      } catch (error) {
        console.warn('Error in loading listener:', error);
      }
    });
    
    return this.currentProject;
  }

  getCurrentProject() {
    return this.currentProject;
  }

  getCurrentProjectFromStorage() {
    // Mock implementation - in a real app this would read from localStorage
    return this.currentProject;
  }

  async autoSaveCurrentProject() {
    if (!this.currentProject) {
      console.warn('No current project to auto-save');
      return;
    }
    console.log(`Auto-saving project: ${this.currentProject.name}`);
    // Mock auto-save functionality
    return true;
  }

  addFileChangeListener(callback) {
    this.fileChangeListeners.add(callback);
  }

  removeFileChangeListener(callback) {
    this.fileChangeListeners.delete(callback);
  }

  addLoadingListener(callback) {
    this.loadingListeners.add(callback);
  }

  removeLoadingListener(callback) {
    this.loadingListeners.delete(callback);
  }

  // Mock method to simulate file changes
  notifyFileChange(fileData) {
    this.fileChangeListeners.forEach(listener => {
      try {
        listener(fileData);
      } catch (error) {
        console.warn('Error in file change listener:', error);
      }
    });
  }

  extractProjectName(projectPath) {
    if (typeof projectPath === 'string') {
      return projectPath.split('/').pop() || 'Untitled Project';
    }
    return 'Untitled Project';
  }

  isInitialized() {
    return this.initialized;
  }
}

export const projectManager = new ProjectManager();