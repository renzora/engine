import { createSignal, createContext, useContext } from 'solid-js';

const ProjectContext = createContext();

export function Project(props) {
  const [currentProject, setCurrentProject] = createSignal(null);
  const [isProjectLoaded, setIsProjectLoaded] = createSignal(false);

  const projectStore = {
    currentProject,
    isProjectLoaded,
    
    setCurrentProject: async (project) => {
      setCurrentProject(project);
      setIsProjectLoaded(true);
      
      // Tell the bridge server to focus file watching on this project
      try {
        const response = await fetch('http://localhost:3001/set-current-project', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ 
            project_name: project?.name || null 
          })
        });
        
        if (response.ok) {
          const result = await response.json();
          // File watcher updated for project
        }
      } catch (error) {
        console.error('Failed to set current project for file watcher:', error);
      }
    },
    
    clearProject: () => {
      setCurrentProject(null);
      setIsProjectLoaded(false);
    }
  };

  return (
    <ProjectContext.Provider value={projectStore}>
      {props.children}
    </ProjectContext.Provider>
  );
}

export function useProject() {
  const context = useContext(ProjectContext);
  if (!context) {
    throw new Error('useProject must be used within a ProjectProvider');
  }
  return context;
}