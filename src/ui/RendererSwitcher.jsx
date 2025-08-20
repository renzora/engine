import { createSignal, For, Show } from 'solid-js';
import { useRendererSwitcher } from '@/api';
import { getRendererStatus } from '@/render';
import { ChevronDown } from '@/ui/icons';

// Ensure renderers are registered by importing the registration module
import '@/render';

export default function RendererSwitcher() {
  const switcher = useRendererSwitcher();
  const rendererStatus = getRendererStatus();

  // Make reactive
  const availableRenderers = () => {
    const renderers = switcher.availableRenderers();
    console.log('[RendererSwitcher] Available renderers:', renderers);
    console.log('[RendererSwitcher] Renderer status:', rendererStatus);
    console.log('[RendererSwitcher] Current renderer:', switcher.currentRenderer());
    return renderers;
  };
  const currentRenderer = () => switcher.currentRenderer();

  const rendererLabels = {
    babylon: 'Babylon.js',
    three: 'Three.js',
    torus: 'Torus',
    webgpu: 'WebGPU',
    playcanvas: 'PlayCanvas',
    pixi: 'PixiJS',
    phaser: 'Phaser'
  };

  const handleSwitchRenderer = async (rendererType) => {
    try {
      await switcher.switchRenderer(rendererType);
      // Close dropdown by removing focus
      document.activeElement?.blur();
    } catch (error) {
      console.error('Failed to switch renderer:', error);
    }
  };

  return (
    <div class="dropdown dropdown-end">
      <div 
        tabindex="0" 
        role="button" 
        class="btn btn-ghost btn-sm gap-2 text-xs"
      >
        <span>🎮</span>
        <span>{rendererLabels[currentRenderer()] || 'Renderer'}</span>
        <ChevronDown class="w-3 h-3" />
      </div>
      
      <ul tabindex="0" class="dropdown-content menu bg-base-200 rounded-box z-[1] w-48 p-2 shadow-xl border border-base-300">
        <li class="menu-title">
          <span class="text-xs text-base-content/60">Rendering Engine</span>
        </li>
        
        <For each={availableRenderers()}>
          {(rendererType) => {
            const status = rendererStatus[rendererType];
            const isActive = currentRenderer() === rendererType;
            const isAvailable = status?.available;
            
            return (
              <li>
                <button
                  class={`text-xs ${isActive ? 'active' : ''} ${!isAvailable ? 'opacity-50 cursor-not-allowed' : ''}`}
                  onClick={() => isAvailable && handleSwitchRenderer(rendererType)}
                  disabled={!isAvailable || switcher.isLoading()}
                  title={status?.reason}
                >
                  <span class="flex items-center gap-2">
                    <span>{isActive ? '●' : '○'}</span>
                    <span>{rendererLabels[rendererType]}</span>
                    <Show when={!isAvailable}>
                      <span class="text-xs text-warning">⚠️</span>
                    </Show>
                  </span>
                </button>
              </li>
            );
          }}
        </For>
        
        <Show when={switcher.isLoading()}>
          <li>
            <div class="text-xs text-center py-2">
              <span class="loading loading-spinner loading-xs mr-2"></span>
              Switching renderer...
            </div>
          </li>
        </Show>
        
        <Show when={switcher.error()}>
          <li>
            <div class="text-xs text-error text-center py-2">
              ⚠️ {switcher.error()}
            </div>
          </li>
        </Show>
      </ul>
    </div>
  );
}