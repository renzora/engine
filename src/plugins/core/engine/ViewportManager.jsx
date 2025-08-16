import { Show, createEffect } from 'solid-js';
import { useEngineAPI, activeViewport } from './EngineAPI.jsx';
import { IconX } from '@tabler/icons-solidjs';

export default function ViewportManager() {
  const api = useEngineAPI();

  return (
    <Show when={activeViewport()}>
      {(viewport) => {
        const ViewportComponent = viewport().component;
        const IconComponent = viewport().icon;
        
        return (
          <div class="fixed inset-0 z-50 flex flex-col bg-slate-900">
            <div class="flex items-center justify-between h-10 px-4 bg-slate-800 border-b border-slate-700">
              <div class="flex items-center gap-2">
                <Show when={IconComponent}>
                  <IconComponent class="w-4 h-4 text-gray-400" />
                </Show>
                <span class="text-white font-medium text-sm">{viewport().title}</span>
              </div>
              
              <Show when={viewport().closable}>
                <button
                  onClick={() => api.closeViewport()}
                  class="p-1 hover:bg-slate-700 rounded transition-colors"
                  title="Close viewport"
                >
                  <IconX class="w-4 h-4 text-gray-400 hover:text-white" />
                </button>
              </Show>
            </div>
            
            <div class="flex-1 overflow-hidden">
              <ViewportComponent {...viewport().props} />
            </div>
          </div>
        );
      }}
    </Show>
  );
}