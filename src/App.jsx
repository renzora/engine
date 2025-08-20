import { onMount } from 'solid-js'
import './base.css'
import { Engine } from '@/api/plugin'
import Layout from './layout'
import DevNotice from './components/DevNotice'
import EditorPage from './pages/editor'
import NodeEditorPage from './pages/nodeEditor'
import { Project } from './plugins/splash/ProjectStore'
import { RenderProvider, RendererType } from '@/api'
export default function App() {
    onMount(() => {
    console.log('🎮 Renzora Engine loaded successfully!')
    console.log('🔌 Plugins loaded with fixed UI layout structure')
  })

  return (
    <Engine>
      <Project>
        <RenderProvider defaultRenderer={RendererType.TORUS}>
          <div class="w-full h-full">
            <Layout />
            <DevNotice />
            <EditorPage />
            <NodeEditorPage />
          </div>
        </RenderProvider>
      </Project>
    </Engine>
  );
}