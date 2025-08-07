import React, { useState, useEffect } from 'react';
import ProjectSplashScreen from './ProjectSplashScreen';
import { projectCreationService } from '@/services/ProjectCreationService';
import { projectManager } from '@/plugins/projects/projectManager';

const ProjectSplashManager = ({ children, onProjectReady }) => {
  const [showSplash, setShowSplash] = useState(false);
  const [isCheckingProject, setIsCheckingProject] = useState(true);

  useEffect(() => {
    checkForExistingProject();
  }, []);

  const checkForExistingProject = async () => {
    try {
      // Always show splash screen on startup to allow project selection
      console.log('Showing project selection splash screen');
      setShowSplash(true);
      setIsCheckingProject(false);
    } catch (error) {
      console.warn('Error during startup:', error);
      // On error, show splash screen to be safe
      setShowSplash(true);
      setIsCheckingProject(false);
    }
  };

  const handleCreateProject = async (projectName) => {
    try {
      console.log(`Creating new project: ${projectName}`);
      
      // Create project using the unified service
      const result = await projectCreationService.createProject(projectName);
      
      if (result.success) {
        console.log(`Project created successfully: ${result.projectPath}`);
        
        // Load the new project using project manager
        await projectManager.loadProject(result.projectPath);
        
        // Mark project manager as initialized to prevent duplicate loading
        projectManager.initialized = true;
        
        // Hide splash screen
        setShowSplash(false);
        
        // Notify parent component that project is ready
        if (onProjectReady) {
          onProjectReady(result);
        }
      } else {
        throw new Error('Project creation failed');
      }
    } catch (error) {
      console.error('Failed to create project:', error);
      throw error; // Let the splash screen handle the error display
    }
  };

  const handleSelectProject = async (project) => {
    try {
      console.log(`Loading selected project: ${project.name || project.path}`);
      
      // Load the selected project using project manager
      const projectPath = project.path || project.name;
      await projectManager.loadProject(projectPath);
      
      // Mark project manager as initialized to prevent duplicate loading
      projectManager.initialized = true;
      
      // Hide splash screen
      setShowSplash(false);
      
      // Notify parent component that project is ready
      if (onProjectReady) {
        onProjectReady({ success: true, projectPath, projectName: project.name });
      }
    } catch (error) {
      console.error('Failed to load selected project:', error);
      throw error; // Let the splash screen handle the error display
    }
  };

  const handleCloseSplash = () => {
    // If user closes splash without creating project, create default project
    console.log('Splash closed, creating default project');
    handleCreateProject('UntitledProject').catch(error => {
      console.error('Failed to create default project:', error);
      // As a last resort, continue without project
      setShowSplash(false);
    });
  };

  // Show loading while checking for projects
  if (isCheckingProject) {
    return (
      <div className="fixed inset-0 bg-gray-900 flex items-center justify-center">
        <div className="text-white text-lg">Checking for existing projects...</div>
      </div>
    );
  }

  // Show splash screen if no projects exist
  if (showSplash) {
    return (
      <ProjectSplashScreen
        onCreateProject={handleCreateProject}
        onSelectProject={handleSelectProject}
        onClose={handleCloseSplash}
      />
    );
  }

  // Render normal app
  return children;
};

export default ProjectSplashManager;