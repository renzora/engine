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
      console.log('Showing project selection splash screen');
      setShowSplash(true);
      setIsCheckingProject(false);
    } catch (error) {
      console.warn('Error during startup:', error);
      setShowSplash(true);
      setIsCheckingProject(false);
    }
  };

  const handleCreateProject = async (projectName) => {
    try {
      console.log(`Creating new project: ${projectName}`);
      
      const result = await projectCreationService.createProject(projectName);
      
      if (result.success) {
        console.log(`Project created successfully: ${result.projectPath}`);
        
        await projectManager.loadProject(result.projectPath);
        
        projectManager.initialized = true;
        
        setShowSplash(false);
        
        if (onProjectReady) {
          onProjectReady(result);
        }
      } else {
        throw new Error('Project creation failed');
      }
    } catch (error) {
      console.error('Failed to create project:', error);
      throw error;
    }
  };

  const handleSelectProject = async (project) => {
    try {
      console.log(`Loading selected project: ${project.name || project.path}`);
      
      const projectPath = project.path || project.name;
      await projectManager.loadProject(projectPath);
      
      projectManager.initialized = true;
      
      setShowSplash(false);
      
      if (onProjectReady) {
        onProjectReady({ success: true, projectPath, projectName: project.name });
      }
    } catch (error) {
      console.error('Failed to load selected project:', error);
      throw error;
    }
  };

  const handleCloseSplash = () => {
    console.log('Splash closed, creating default project');
    handleCreateProject('UntitledProject').catch(error => {
      console.error('Failed to create default project:', error);
      setShowSplash(false);
    });
  };

  if (isCheckingProject) {
    return (
      <div className="fixed inset-0 bg-gray-900 flex items-center justify-center">
        <div className="text-white text-lg">Checking for existing projects...</div>
      </div>
    );
  }

  if (showSplash) {
    return (
      <ProjectSplashScreen
        onCreateProject={handleCreateProject}
        onSelectProject={handleSelectProject}
        onClose={handleCloseSplash}
      />
    );
  }

  return children;
};

export default ProjectSplashManager;