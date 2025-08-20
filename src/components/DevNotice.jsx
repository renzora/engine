import { createSignal, onMount, Show } from 'solid-js'
import { usePluginAPI } from '@/api/plugin'

export default function DevNotice() {
  const [isVisible, setIsVisible] = createSignal(false)
  const api = usePluginAPI()

  onMount(() => {
    const dismissed = localStorage.getItem('dev-disclaimer-dismissed')
    if (!dismissed) {
      setTimeout(() => setIsVisible(true), 1000)
    }
  })

  const dismiss = () => {
    localStorage.setItem('dev-disclaimer-dismissed', 'true')
    setIsVisible(false)
  }

  return (
    <Show when={isVisible()}>
      <div class="fixed inset-0 backdrop-blur-sm flex items-center justify-center z-[100] bg-black/50">
        <div class="bg-gray-900 border border-yellow-500/50 rounded-lg p-6 max-w-lg mx-4">
          <div class="flex items-center gap-3 mb-4">
            <div class="w-8 h-8 bg-yellow-500/20 rounded-full flex items-center justify-center">
              <svg class="w-5 h-5 text-yellow-400" fill="currentColor" viewBox="0 0 20 20">
                <path fill-rule="evenodd" d="M8.485 3.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 3.495zM10 6a.75.75 0 01.75.75v3.5a.75.75 0 01-1.5 0v-3.5A.75.75 0 0110 6zm0 9a1 1 0 100-2 1 1 0 000 2z" clip-rule="evenodd" />
              </svg>
            </div>
            <h3 class="text-lg font-semibold text-yellow-400">Development Version</h3>
          </div>

          <p class="text-sm text-gray-300 leading-relaxed mb-4">
            This is an early development version of Renzora Engine. Expect breaking changes, experimental features, and potential bugs.
          </p>

          <div class="flex gap-2 flex-wrap">
            <button 
              onClick={dismiss}
              class="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors"
            >
              I Understand
            </button>
            
          </div>
        </div>
      </div>
    </Show>
  )
}