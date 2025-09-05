import { render } from 'solid-js/web';
import RuntimeApp from './RuntimeApp.jsx';
import './runtime.css';

// Runtime entry point for exported projects
console.log('🚀 Renzora Runtime: Starting...');

// Check if we're running in Tauri
if (window.__TAURI__) {
  console.log('🚀 Runtime: Tauri environment detected');
  
  // Load project data from Tauri backend
  import('@tauri-apps/api/core').then(async ({ invoke }) => {
    try {
      const projectDataJson = await invoke('get_project_data');
      window.__RENZORA_PROJECT_DATA__ = JSON.parse(projectDataJson);
      console.log('📦 Runtime: Project data loaded from Tauri');
    } catch (error) {
      console.error('❌ Runtime: Failed to load project data from Tauri:', error);
    }
  });
} else {
  console.log('🚀 Runtime: Web environment detected');
  
  // For web runtime, project data should be embedded in the HTML
  if (!window.__RENZORA_PROJECT_DATA__) {
    console.error('❌ Runtime: No project data found in web environment');
  }
}

// Render the runtime app
const root = document.getElementById('root');

if (import.meta.env.DEV && !(root instanceof HTMLElement)) {
  throw new Error(
    'Root element not found. Did you forget to add it to your index.html? Or maybe the id attribute got mispelled?'
  );
}

render(() => <RuntimeApp />, root);