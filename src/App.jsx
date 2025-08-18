import { onMount } from 'solid-js'
import './base.css'
import { Engine } from '@/api/plugin'
import Layout from './layout'
import DevNotice from './components/DevNotice'
import { Project } from './plugins/splash/ProjectStore'
import { Theme } from '../themes/Theme.jsx'
import EditorPage from './pages/editor'
import NodeEditorPage from './pages/nodeEditor'

function AppContent() {

  onMount(() => {
    console.log('🎮 Renzora Engine loaded successfully!')
    console.log('🔌 Plugins loaded with fixed UI layout structure')
  })

  return (
    <Theme>
      <Project>
        <div class="w-full h-full">
          <Layout />
          <DevNotice />
          <EditorPage />
          <NodeEditorPage />
        </div>
      </Project>
    </Theme>
  );
}

export default function App() {
  return (
    <Engine>
      <AppContent />
    </Engine>
  )
}