import { createPlugin } from '@/api/plugin';
import SplashScreen from './SplashScreen.jsx';
import { ProjectProvider, useProject } from './ProjectStore.jsx';
import { IconHome, IconFolder, IconSettings } from '@tabler/icons-solidjs';
import { createSignal, Show, createEffect } from 'solid-js';
import { usePluginAPI } from '@/api/plugin';
import { viewportStore } from '@/layout/stores/ViewportStore.jsx';
import { getProjectCurrentScene } from '@/api/bridge/projects.js';
import { sceneManager } from '@/api/scene/SceneManager.js';

function SplashViewport({ tab }) {
  const { currentProject, isProjectLoaded, setCurrentProject } = useProject();
  const pluginAPI = usePluginAPI();
  const [isLoading, setIsLoading] = createSignal(false);
  const [loadingStage, setLoadingStage] = createSignal('');
  const [currentFile, setCurrentFile] = createSignal('');
  const [processedCount, setProcessedCount] = createSignal(0);
  const [totalCount, setTotalCount] = createSignal(0);

  // Hide UI elements when this splash viewport is active
  createEffect(() => {
    const activeTabId = viewportStore.activeTabId;
    const isThisTabActive = activeTabId === tab?.id;
    
    if (isThisTabActive) {
      pluginAPI.hidePanel();
      pluginAPI.hideProps();
      pluginAPI.hideMenu();
      pluginAPI.hideTabs();
      pluginAPI.hideToolbar();
      pluginAPI.hideHelper();
    }
  });

  // Listen for detailed scene loading progress
  createEffect(() => {
    const handleProgress = (event) => {
      const { stage, currentFile, processedCount, totalCount } = event.detail;
      setLoadingStage(stage || '');
      setCurrentFile(currentFile || '');
      setProcessedCount(processedCount || 0);
      setTotalCount(totalCount || 0);
    };

    document.addEventListener('scene-loading-progress', handleProgress);
    return () => document.removeEventListener('scene-loading-progress', handleProgress);
  });

  // Check for pending project load from menu refresh on startup
  createEffect(() => {
    const checkPendingProjectLoad = () => {
      try {
        const pendingLoad = localStorage.getItem('pendingProjectLoad');
        if (pendingLoad) {
          const { project, timestamp } = JSON.parse(pendingLoad);
          
          // Check if the pending load is recent (within 10 seconds)
          const age = Date.now() - timestamp;
          if (age < 10000) {
            console.log('🔄 Detected pending project load from menu refresh:', project.name);
            localStorage.removeItem('pendingProjectLoad');
            
            // Set loading state immediately to skip splash screen
            setIsLoading(true);
            setLoadingStage(`Loading ${project.name}...`);
            
            // Auto-load the project after a brief delay
            setTimeout(() => {
              handleProjectSelect(project);
            }, 100);
            return;
          } else {
            // Clean up old pending loads
            localStorage.removeItem('pendingProjectLoad');
          }
        }
      } catch (error) {
        console.warn('Failed to check pending project load:', error);
        localStorage.removeItem('pendingProjectLoad');
      }
    };
    
    // Check on mount
    checkPendingProjectLoad();
  });

  const handleProjectSelect = async (project) => {
    // Project selected from splash viewport or auto-loaded from menu refresh
    
    setIsLoading(true);
    setLoadingStage('Initializing project...');
    
    try {
      // Set project first
      const { setCurrentProject: setApiProject } = await import('@/api/bridge/projects.js');
      setApiProject(project);
      setCurrentProject(project);

      // Create Babylon scene
      if (window._createBabylonScene) {
        // Creating Babylon scene for project
        setLoadingStage('Creating 3D scene...');
        const scene = await window._createBabylonScene();
        
        if (scene) {
          // Load the current scene - this will emit detailed progress events
          try {
            const currentSceneName = await getProjectCurrentScene(project.name);
            // Loading current scene
            
            const result = await sceneManager.loadScene(currentSceneName);
            
            if (result.success) {
              // Current scene loaded successfully
            } else {
              console.warn('⚠️ Failed to load current scene:', result.error);
            }
          } catch (error) {
            console.warn('⚠️ Failed to get/load current scene:', error);
          }
        }
      }

      setLoadingStage('Setting up interface...');

      // Show UI elements
      pluginAPI.showProps();
      pluginAPI.showPanel();
      pluginAPI.showMenu();
      pluginAPI.showFooter();
      pluginAPI.showToolbar();
      pluginAPI.showHelper();
      
      // Create scene viewport
      pluginAPI.createSceneViewport({
        name: 'Scene 1',
        setActive: true
      });
      
      setLoadingStage('Complete!');
      
      // Clear unsaved changes after project loading is complete
      // (project loading processes can trigger markAsModified calls during initialization)
      setTimeout(() => {
        import('@/stores/UnsavedChangesStore.jsx').then(({ unsavedChangesActions }) => {
          unsavedChangesActions.clearChanges();
          console.log('🧹 Cleared unsaved changes after project load');
        }).catch(err => {
          console.warn('Failed to clear unsaved changes after project load:', err);
        });
      }, 500);
      
      // Close splash screen after a brief delay
      setTimeout(() => {
        import('@/layout/stores/ViewportStore.jsx').then(({ viewportActions, viewportStore }) => {
          const splashTab = viewportStore.tabs.find(tab => tab.type === 'splash-viewport');
          if (splashTab) {
            // Closing splash viewport tab
            viewportActions.removeViewportTab(splashTab.id);
          }
        }).catch(err => {
          console.error('Failed to close splash viewport:', err);
        });
      }, 1000);
      
      document.dispatchEvent(new CustomEvent('engine:project-selected', { 
        detail: { project } 
      }));
      
    } catch (error) {
      console.error('❌ Failed to load project:', error);
      setIsLoading(false);
      setLoadingStage('Failed to load project');
    }
  };

  return (
    <Show when={isProjectLoaded() && currentProject() && !isLoading()} fallback={
      <Show when={isLoading()} fallback={<SplashScreen onProjectSelect={handleProjectSelect} />}>
        <div class="w-full h-full bg-slate-900 text-white flex items-center justify-center">
          <div class="text-center max-w-md mx-auto p-6">
            <div class="mb-6">
              <div class="p-3 bg-blue-600/20 rounded-full w-16 h-16 flex items-center justify-center mx-auto mb-4">
                <IconFolder class="w-8 h-8 text-blue-400 animate-pulse" />
              </div>
              <h1 class="text-xl font-bold mb-2">Loading Project</h1>
              <p class="text-lg text-blue-400 font-semibold">{currentProject()?.name}</p>
            </div>
            
            {/* Progress details */}
            <div class="mb-4">
              <div class="text-sm text-gray-300 mb-2">{loadingStage()}</div>
              
              {/* Show progress count if available */}
              <Show when={totalCount() > 0}>
                <div class="w-full bg-gray-700 rounded-full h-2.5 mb-2">
                  <div 
                    class="bg-blue-500 h-2.5 rounded-full transition-all duration-300 ease-out" 
                    style={{ width: `${totalCount() > 0 ? (processedCount() / totalCount()) * 100 : 0}%` }}
                  ></div>
                </div>
                <div class="text-xs text-gray-400 mb-2">
                  {processedCount()} / {totalCount()} items processed
                </div>
              </Show>
              
              {/* Show current file being processed */}
              <Show when={currentFile()}>
                <div class="text-xs text-gray-500 font-mono bg-gray-800 p-2 rounded truncate">
                  {currentFile()}
                </div>
              </Show>
            </div>
            
            <p class="text-gray-400 text-sm">
              Setting up your project and loading scene data...
            </p>
          </div>
        </div>
      </Show>
    }>
      {/* This fallback will only show if there's an error or unexpected state */}
      <div class="w-full h-full bg-slate-900 text-white flex items-center justify-center">
        <div class="text-center max-w-md mx-auto p-6">
          <div class="mb-4">
            <div class="p-3 bg-red-600/20 rounded-full w-16 h-16 flex items-center justify-center mx-auto mb-4">
              <IconFolder class="w-8 h-8 text-red-400" />
            </div>
            <h1 class="text-xl font-bold mb-2">Loading Error</h1>
            <p class="text-lg text-red-400 font-semibold">{currentProject()?.name}</p>
          </div>
          <p class="text-gray-400 text-sm">
            There was an issue loading your project. Please try again.
          </p>
        </div>
      </div>
    </Show>
  );
}

export default createPlugin({
  id: 'splash-plugin',
  name: 'Splash Screen Plugin',
  version: '1.0.0',
  description: 'Project selection and startup screen for Renzora Engine',
  author: 'Renzora Engine Team',

  async onInit(api) {
    // Splash plugin initialized
  },

  async onStart(api) {
    // Starting splash screen plugin
    
    api.viewport('splash-viewport', {
      label: 'Project Home',
      component: SplashViewport,
      icon: IconHome,
      description: 'Project selection and management screen'
    });
    // Splash viewport type registered

    api.hideProps();
    api.hidePanel();
    api.hideMenu();
    api.hideFooter();
    api.hideToolbar();
    api.hideHelper();
    
    setTimeout(() => {
      // Creating splash viewport tab
      api.open('splash-viewport', {
        label: 'Project Home',
        setActive: true
      });
    }, 500);

    api.on('project-created', (data) => {
      // Project created event received
    });

    api.on('project-opened', (data) => {
      // Project opened event received
    });

    api.on('project-selected', (data) => {
      // Project selected
    });

    // Splash screen plugin started
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    // Stopping splash screen plugin
  },

  async onDispose() {
    // Disposing splash screen plugin
  }
});