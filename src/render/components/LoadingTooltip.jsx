import { Show } from 'solid-js'

export function LoadingTooltip({ loadingTooltip }) {
  return (
    <Show when={loadingTooltip().isVisible}>
      <div
        class="fixed bg-gray-800 text-white px-3 py-2 rounded-md shadow-lg z-50 pointer-events-none"
        style={{
          left: `${loadingTooltip().position.x + 10}px`,
          top: `${loadingTooltip().position.y - 40}px`
        }}
      >
        <div class="flex items-center space-x-2">
          <div class="animate-spin w-4 h-4 border-2 border-white border-t-transparent rounded-full"></div>
          <span class="text-sm">{loadingTooltip().message}</span>
        </div>
        <Show when={loadingTooltip().progress !== null}>
          <div class="mt-1 w-32 bg-gray-600 rounded-full h-1">
            <div
              class="bg-blue-500 h-1 rounded-full transition-all duration-200"
              style={{
                width: `${(loadingTooltip().progress || 0) * 100}%`
              }}
            />
          </div>
        </Show>
      </div>
    </Show>
  )
}