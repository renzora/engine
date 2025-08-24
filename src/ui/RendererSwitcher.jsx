import { createSignal, For, Show, createEffect } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
// Simple renderer display for Babylon.js

export default function RendererSwitcher() {
  const [isOpen, setIsOpen] = createSignal(false);
  const [currentRenderer, setCurrentRenderer] = createSignal('babylon');
  const [isVulkanSupported, setIsVulkanSupported] = createSignal(false);
  const [isTauriEnv, setIsTauriEnv] = createSignal(false);

  // Check if running in Tauri environment
  createEffect(async () => {
    try {
      // Try to call a Tauri command to detect environment
      await invoke('check_vulkan_support');
      setIsTauriEnv(true);
      
      // Check Vulkan support
      const vulkanSupported = await invoke('check_vulkan_support');
      setIsVulkanSupported(vulkanSupported);
    } catch (e) {
      // Not in Tauri environment
      setIsTauriEnv(false);
      setIsVulkanSupported(false);
    }
  });

  const availableRenderers = () => {
    const renderers = ['babylon', 'threejs', 'playcanvas', 'pixijs', 'phaser', 'melonjs'];
    if (isTauriEnv()) {
      renderers.push('webgpu');
      renderers.push('babylon-native');
      if (isVulkanSupported()) {
        renderers.push('vulkan');
      }
    }
    return renderers;
  };

  const rendererLabels = {
    babylon: 'Babylon.js (WebGL)',
    webgpu: 'Babylon.js (WebGPU)',
    'babylon-native': 'Babylon Native',
    vulkan: 'Torus R1',
    threejs: 'Three.js',
    three: 'Three.js',
    torus: 'Torus',
    playcanvas: 'PlayCanvas',
    pixijs: 'PixiJS',
    pixi: 'PixiJS',
    phaser: 'Phaser',
    melonjs: 'MelonJS'
  };

  // Get library versions and languages
  const getRendererInfo = (rendererType) => {
    switch (rendererType) {
      case 'babylon':
        try {
          const version = window.BABYLON?.Engine?.Version || '8.20.0';
          return { version, language: 'JavaScript' };
        } catch {
          return { version: '8.20.0', language: 'JavaScript' };
        }
      case 'webgpu':
        return { version: '8.20.0', language: 'JavaScript/WebGPU' };
      case 'babylon-native':
        return { version: '8.20.0', language: 'C++' };
      case 'torus':
      case 'vulkan':
        return { version: 'v1.0', language: 'Rust' };
      case 'three':
      case 'threejs':
        try {
          const version = window.THREE?.REVISION ? `r${window.THREE.REVISION}` : '0.169+';
          return { version, language: 'JavaScript' };
        } catch {
          return { version: '0.169+', language: 'JavaScript' };
        }
      case 'pixi':
      case 'pixijs':
        return { version: '8.12.0', language: 'JavaScript' };
      case 'playcanvas':
        return { version: '1.70+', language: 'JavaScript' };
      case 'phaser':
        return { version: '3.90.0', language: 'JavaScript' };
      case 'melonjs':
        return { version: '17.4.0', language: 'JavaScript' };
      default:
        return { version: '', language: '' };
    }
  };

  const handleSwitchRenderer = async (rendererType) => {
    try {
      if (rendererType === 'vulkan' && isTauriEnv()) {
        // Initialize native Vulkan renderer
        await invoke('init_vulkan_renderer');
        console.log('Switched to native Vulkan renderer');
      } else if (rendererType === 'babylon-native' && isTauriEnv()) {
        // Initialize Babylon Native renderer
        await invoke('init_babylon_native', { width: 800, height: 600 });
        console.log('Switched to Babylon Native renderer');
      }
      setCurrentRenderer(rendererType);
      setIsOpen(false);
      
      // Emit custom event for renderer change
      window.dispatchEvent(new CustomEvent('renderer-changed', { 
        detail: { renderer: rendererType } 
      }));
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
              const isActive = currentRenderer() === rendererType;
              const isAvailable = true;
              const { version, language } = getRendererInfo(rendererType);
              
              return (
                <button
                  onClick={() => isAvailable && handleSwitchRenderer(rendererType)}
                  disabled={!isAvailable}
                  title={`${version} • ${language}`}
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
                    <div class="flex items-center justify-between gap-1">
                      <span class={`text-[10px] truncate ${
                        isActive ? 'text-primary-content/70' : 'text-base-content/60'
                      }`}>
                        {version}
                      </span>
                      <span class={`text-[10px] truncate ${
                        isActive ? 'text-primary-content/70' : 'text-base-content/60'
                      }`}>
                        {language}
                      </span>
                    </div>
                  </div>
                </button>
              );
            }}
          </For>
          
        </div>
      )}
    </div>
  );
}