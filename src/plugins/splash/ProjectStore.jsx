import { createSignal, createContext, useContext } from 'solid-js';

// Project Store Context
const ProjectContext = createContext();

export function ProjectProvider(props) {
  const [currentProject, setCurrentProject] = createSignal(null);
  const [isProjectLoaded, setIsProjectLoaded] = createSignal(false);

  const projectStore = {
    // State
    currentProject,
    isProjectLoaded,
    
    // Actions
    setCurrentProject: (project) => {
      setCurrentProject(project);
      setIsProjectLoaded(true);
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