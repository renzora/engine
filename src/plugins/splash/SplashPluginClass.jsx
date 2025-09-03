import { createPlugin } from '@/api/plugin';
import SplashScreen from './SplashScreen.jsx';
import { ProjectProvider, useProject } from './ProjectStore.jsx';
import { IconHome, IconFolder, IconSettings } from '@tabler/icons-solidjs';
import { createSignal, Show, createEffect } from 'solid-js';
import { usePluginAPI } from '@/api/plugin';
import { viewportStore } from '@/layout/stores/ViewportStore.jsx';

function SplashViewport({ tab }) {
  const { currentProject, isProjectLoaded, setCurrentProject } = useProject();
  const pluginAPI = usePluginAPI();

  // Hide UI elements when this splash viewport is active
  createEffect(() => {
    const activeTabId = viewportStore.activeTabId;
    const isThisTabActive = activeTabId === tab?.id;
    
    if (isThisTabActive) {
      pluginAPI.hidePanel();
      pluginAPI.hideProps();
      pluginAPI.hideMenu();
      pluginAPI.hideTabs();
    }
  });

  const handleProjectSelect = (project) => {
    console.log('🎯 Project selected from splash viewport:', project.name);
    setCurrentProject(project);

    pluginAPI.showProps();
    pluginAPI.showPanel();
    pluginAPI.showMenu();
    
    pluginAPI.createSceneViewport({
      name: 'Scene 1',
      setActive: true
    });
    
    setTimeout(() => {
      import('@/layout/stores/ViewportStore.jsx').then(({ viewportActions, viewportStore }) => {
        const splashTab = viewportStore.tabs.find(tab => tab.type === 'splash-viewport');
        if (splashTab) {
          console.log('🎯 Closing splash viewport tab:', splashTab.id);
          viewportActions.removeViewportTab(splashTab.id);
        }
      }).catch(err => {
        console.error('Failed to close splash viewport:', err);
      });
    }, 100);
    
    document.dispatchEvent(new CustomEvent('engine:project-selected', { 
      detail: { project } 
    }));
  };

  return (
    <Show when={isProjectLoaded() && currentProject()} fallback={
      <SplashScreen onProjectSelect={handleProjectSelect} />
    }>
      <div class="w-full h-full bg-slate-900 text-white flex items-center justify-center">
        <div class="text-center max-w-md mx-auto p-6">
          <div class="mb-4">
            <div class="p-3 bg-green-600/20 rounded-full w-16 h-16 flex items-center justify-center mx-auto mb-4">
              <IconFolder class="w-8 h-8 text-green-400" />
            </div>
            <h1 class="text-xl font-bold mb-2">Project Ready</h1>
            <p class="text-lg text-green-400 font-semibold">{currentProject()?.name}</p>
          </div>
          <p class="text-gray-400 text-sm">
            Your project is loaded and ready to use. 
            Switch to other viewport tabs to start working with your 3D scenes.
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
    console.log('[SplashPlugin] Initializing splash screen plugin...');
    console.log('[SplashPlugin] Splash plugin initialized');
  },

  async onStart(api) {
    console.log('[SplashPlugin] Starting splash screen plugin...');
    console.log('[SplashPlugin] Registering splash viewport type...');
    
    api.viewport('splash-viewport', {
      label: 'Project Home',
      component: SplashViewport,
      icon: IconHome,
      description: 'Project selection and management screen'
    });
    console.log('[SplashPlugin] Splash viewport type registered');

    api.hideProps();
    api.hidePanel();
    api.hideMenu();
    
    setTimeout(() => {
      console.log('[SplashPlugin] Creating splash viewport tab...');
      api.open('splash-viewport', {
        label: 'Project Home',
        setActive: true
      });
    }, 500);

    api.on('project-created', (data) => {
      console.log('[SplashPlugin] Project created event received:', data);
    });

    api.on('project-opened', (data) => {
      console.log('[SplashPlugin] Project opened event received:', data);
    });

    api.on('project-selected', (data) => {
      console.log('[SplashPlugin] Project selected:', data.project.name);
    });

    console.log('[SplashPlugin] Splash screen plugin started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('[SplashPlugin] Stopping splash screen plugin...');
  },

  async onDispose() {
    console.log('[SplashPlugin] Disposing splash screen plugin...');
  }
});