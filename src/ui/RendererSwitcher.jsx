import { createSignal, For, Show } from 'solid-js';
import { useRendererSwitcher, getRendererStatus } from '@/api/render';

// Ensure renderers are registered by importing the registration module
import '@/api/render';

export default function RendererSwitcher() {
  const switcher = useRendererSwitcher();
  const rendererStatus = getRendererStatus();
  const [isOpen, setIsOpen] = createSignal(false);

  // Make reactive
  const availableRenderers = () => {
    const renderers = switcher.availableRenderers();
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

  // Get library versions - these would ideally come from the renderer implementations
  const getRendererVersion = (rendererType) => {
    switch (rendererType) {
      case 'babylon':
        try {
          // Try to get Babylon.js version if available
          return window.BABYLON?.Engine?.Version || '7.x';
        } catch {
          return '7.x';
        }
      case 'torus':
        return 'v1.0'; // Custom renderer version
      case 'three':
        try {
          // Try to get Three.js version if available
          return window.THREE?.REVISION ? `r${window.THREE.REVISION}` : '0.160+';
        } catch {
          return '0.160+';
        }
      case 'pixi':
        return '8.x';
      case 'playcanvas':
        return '1.70+';
      case 'phaser':
        return '3.80+';
      case 'webgpu':
        return 'Draft';
      default:
        return '';
    }
  };

  const handleSwitchRenderer = async (rendererType) => {
    try {
      await switcher.switchRenderer(rendererType);
      setIsOpen(false);
    } catch (error) {
      console.error('Failed to switch renderer:', error);
    }
  };

  return (
    <div class="relative">
      <button
        onClick={() => setIsOpen(!isOpen())}
        class="px-2 py-1 text-xs bg-base-200 text-base-content rounded border border-base-300 hover:bg-base-300 transition-colors flex items-center gap-1"
      >
        <span>🎮 {rendererLabels[currentRenderer()] || 'Renderer'}</span>
        <svg class={`w-3 h-3 transition-transform ${isOpen() ? 'rotate-180' : ''}`} fill="currentColor" viewBox="0 0 20 20">
          <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
        </svg>
      </button>
      
      {isOpen() && (
        <div class="absolute top-full right-0 mt-1 w-40 bg-base-200 border border-base-300 rounded shadow-xl z-50">
          <For each={availableRenderers()}>
            {(rendererType) => {
              const status = rendererStatus[rendererType];
              const isActive = currentRenderer() === rendererType;
              const isAvailable = status?.available;
              const version = getRendererVersion(rendererType);
              
              return (
                <button
                  onClick={() => isAvailable && handleSwitchRenderer(rendererType)}
                  disabled={!isAvailable || switcher.isLoading()}
                  title={`${status?.reason}${version ? ` (${version})` : ''}`}
                  class={`w-full px-2 py-1 text-left text-xs transition-colors hover:bg-base-300 first:rounded-t last:rounded-b ${
                    isActive ? 'bg-primary text-primary-content' : 'text-base-content'
                  } ${!isAvailable ? 'opacity-50 cursor-not-allowed' : ''}`}
                >
                  <div class="flex flex-col gap-0">
                    <div class="flex items-center justify-between">
                      <span class="truncate font-medium">{rendererLabels[rendererType]}</span>
                      <span class="flex items-center gap-1">
                        {isActive && <span class="text-xs">✓</span>}
                        <Show when={!isAvailable}>
                          <span class="text-xs text-warning">⚠️</span>
                        </Show>
                      </span>
                    </div>
                    {version && (
                      <span class={`text-[10px] truncate ${
                        isActive ? 'text-primary-content/70' : 'text-base-content/60'
                      }`}>
                        {version}
                      </span>
                    )}
                  </div>
                </button>
              );
            }}
          </For>
          
          <Show when={switcher.isLoading()}>
            <div class="px-2 py-1 text-xs text-center border-t border-base-300">
              <span class="loading loading-spinner loading-xs mr-1"></span>
              Loading...
            </div>
          </Show>
          
          <Show when={switcher.error()}>
            <div class="px-2 py-1 text-xs text-error text-center border-t border-base-300">
              ⚠️ {switcher.error()}
            </div>
          </Show>
        </div>
      )}
    </div>
  );
}