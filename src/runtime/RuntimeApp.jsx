import { createSignal, onMount, onCleanup } from 'solid-js';
import { RuntimeRenderer } from './RuntimeRenderer.jsx';
import { RuntimeScriptManager } from './RuntimeScriptManager.jsx';
import { RuntimeAssetLoader } from './RuntimeAssetLoader.jsx';

/**
 * RuntimeApp - Minimal application for exported projects
 * Contains only what's needed to run the game/experience
 */
export default function RuntimeApp() {
  let canvasRef = null;
  const [isLoading, setIsLoading] = createSignal(true);
  const [loadingProgress, setLoadingProgress] = createSignal(0);
  const [loadingStatus, setLoadingStatus] = createSignal('Initializing...');
  const [error, setError] = createSignal(null);
  
  let renderer = null;
  let scriptManager = null;
  let assetLoader = null;

  onMount(async () => {
    try {
      // Initialize runtime components
      
      // Check for embedded project data
      if (!window.__RENZORA_PROJECT_DATA__) {
        throw new Error('No project data found. This appears to be an invalid export.');
      }
      
      const projectData = window.__RENZORA_PROJECT_DATA__;
      // Project data loaded successfully
      
      setLoadingStatus('Initializing renderer...');
      setLoadingProgress(10);
      
      // Initialize renderer
      renderer = new RuntimeRenderer(canvasRef);
      await renderer.initialize();
      
      setLoadingStatus('Loading assets...');
      setLoadingProgress(30);
      
      // Initialize asset loader
      assetLoader = new RuntimeAssetLoader(renderer.scene);
      await assetLoader.loadProjectAssets(projectData.assets);
      
      setLoadingStatus('Loading scenes...');
      setLoadingProgress(60);
      
      // Load main scene
      if (projectData.manifest.runtime.entry_scene) {
        const entrySceneData = projectData.scenes[projectData.manifest.runtime.entry_scene];
        if (entrySceneData) {
          await assetLoader.loadScene(entrySceneData.data);
        }
      }
      
      setLoadingStatus('Initializing scripts...');
      setLoadingProgress(80);
      
      // Initialize script manager
      scriptManager = new RuntimeScriptManager(renderer.scene);
      await scriptManager.loadProjectScripts(projectData.scripts);
      
      setLoadingStatus('Starting runtime...');
      setLoadingProgress(90);
      
      // Start the runtime
      renderer.start();
      scriptManager.start();
      
      setLoadingProgress(100);
      setIsLoading(false);
      
      // Runtime initialization complete
      
    } catch (err) {
      console.error('❌ RuntimeApp: Initialization failed:', err);
      setError(err.message);
      setIsLoading(false);
    }
  });

  onCleanup(() => {
    // Clean up runtime resources
    
    if (scriptManager) {
      scriptManager.dispose();
    }
    
    if (assetLoader) {
      assetLoader.dispose();
    }
    
    if (renderer) {
      renderer.dispose();
    }
  });

  return (
    <div class="w-full h-screen bg-black flex flex-col">
      {/* Loading Screen */}
      {isLoading() && (
        <div class="absolute inset-0 bg-black flex items-center justify-center z-50">
          <div class="text-center text-white">
            <div class="mb-4">
              <div class="w-64 bg-gray-800 rounded-full h-2 mx-auto">
                <div 
                  class="bg-blue-600 h-2 rounded-full transition-all duration-300"
                  style={`width: ${loadingProgress()}%`}
                />
              </div>
            </div>
            <div class="text-lg font-semibold mb-2">
              {window.__RENZORA_PROJECT_DATA__?.project?.name || 'Loading...'}
            </div>
            <div class="text-sm text-gray-400">
              {loadingStatus()}
            </div>
            <div class="text-xs text-gray-600 mt-2">
              {Math.round(loadingProgress())}%
            </div>
          </div>
        </div>
      )}
      
      {/* Error Screen */}
      {error() && (
        <div class="absolute inset-0 bg-red-900 flex items-center justify-center z-50">
          <div class="text-center text-white p-8">
            <div class="text-xl font-bold mb-4">Runtime Error</div>
            <div class="text-sm text-red-200 max-w-md">
              {error()}
            </div>
            <button 
              class="mt-4 px-4 py-2 bg-red-700 hover:bg-red-600 rounded text-sm"
              onClick={() => window.location.reload()}
            >
              Reload
            </button>
          </div>
        </div>
      )}
      
      {/* Main Render Canvas */}
      <canvas 
        ref={canvasRef}
        id="renderCanvas"
        class="w-full h-full"
        style="outline: none; -webkit-tap-highlight-color: rgba(255, 255, 255, 0);"
      />
      
      {/* Runtime Controls (minimal) */}
      {!isLoading() && !error() && (
        <div class="absolute bottom-4 left-4 z-40">
          <div class="flex gap-2">
            <button 
              class="px-3 py-1 bg-black bg-opacity-50 text-white text-xs rounded hover:bg-opacity-70 transition-all"
              onClick={() => {
                if (renderer?.engine) {
                  if (renderer.engine.renderEvenInBackground) {
                    renderer.engine.renderEvenInBackground = false;
                    scriptManager?.pause();
                  } else {
                    renderer.engine.renderEvenInBackground = true;
                    scriptManager?.start();
                  }
                }
              }}
            >
              ⏯️
            </button>
            
            {window.__TAURI__ && (
              <button 
                class="px-3 py-1 bg-black bg-opacity-50 text-white text-xs rounded hover:bg-opacity-70 transition-all"
                onClick={async () => {
                  const { exit } = await import('@tauri-apps/api/process');
                  exit();
                }}
              >
                ✕
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}