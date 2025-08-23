import { onMount } from 'solid-js'
import './base.css'
import './themes'
import { Engine } from '@/api/plugin'
import Layout from './layout'
import DevNotice from './components/DevNotice'
import EditorPage from './pages/editor'
import { Project } from './plugins/splash/ProjectStore'
import KeyboardShortcuts from './components/KeyboardShortcuts'
export default function App() {
    onMount(() => {
    console.log('🎮 Renzora Engine loaded successfully!')
    console.log('🔌 Plugins loaded with fixed UI layout structure')
  })

  return (
    <Engine>
      <Project>
        <KeyboardShortcuts />
        <div class="w-full h-full">
          <Layout />
          <DevNotice />
          <EditorPage />
        </div>
      </Project>
    </Engine>
  );
}