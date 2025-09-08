import { onMount, For } from 'solid-js'
import './base.css'
import './themes'
import { Engine, layoutComponents } from '@/api/plugin'
import Layout from './layout'
import DevNotice from './components/DevNotice'
import EditorPage from './pages/editor'
import { Project } from './plugins/splash/ProjectStore'
import KeyboardShortcuts from './components/KeyboardShortcuts'
export default function App() {
    onMount(() => {
    // Engine loaded successfully
  })

  return (
    <Engine>
      <Project>
        <KeyboardShortcuts />
        <div class="w-full h-full">
          <Layout />
          <DevNotice />
          <EditorPage />
          
          {/* Render layout components from plugins */}
          <For each={Array.from(layoutComponents().values())}>
            {(Component) => <Component />}
          </For>
        </div>
      </Project>
    </Engine>
  );
}