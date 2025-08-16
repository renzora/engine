import { onMount } from 'solid-js'
import './base.css'
import Engine from './plugins/core/engine/Engine.jsx'
import { useEngineAPI } from './plugins/core/engine/EngineAPI.jsx'
import EditorPlugin from './plugins/editor'
import DevNotice from './components/DevNotice'
import { ProjectProvider } from './plugins/splash/ProjectStore'

function AppContent() {
  const engineAPI = useEngineAPI();

  onMount(() => {
    console.log('🎮 Renzora Engine loaded successfully!')
    console.log('🔌 Plugins loaded with fixed UI layout structure')
  })

  // Fixed UI Layout Structure - Non-Changeable  
  // Splash screen is now available as a viewport type
  // ProjectProvider is maintained for components that need project context
  return (
    <ProjectProvider>
      <div class="w-full h-full">
        <EditorPlugin />
        <DevNotice />
      </div>
    </ProjectProvider>
  );
}

export default function App() {
  return (
    <Engine>
      <AppContent />
    </Engine>
  )
}