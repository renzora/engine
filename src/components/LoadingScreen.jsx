import { createSignal, createEffect, onMount } from 'solid-js';
import { IconRocket } from '@tabler/icons-solidjs';

export default function LoadingScreen({ onLoadComplete }) {
  const [loadingStage, setLoadingStage] = createSignal('Initializing...');
  const [progress, setProgress] = createSignal(0);
  const [fadeOut, setFadeOut] = createSignal(false);

  onMount(async () => {
    // Show window immediately
    setTimeout(async () => {
      if (typeof window !== 'undefined' && window.__TAURI_INTERNALS__) {
        try {
          const { getCurrentWindow } = await import('@tauri-apps/api/window');
          const currentWindow = getCurrentWindow();
          await currentWindow.show();
        } catch (error) {
          console.warn('Failed to show window:', error);
        }
      }
    }, 50);

    // Simple loading with just one brief stage
    setLoadingStage('Loading...');
    setProgress(50);

    // Wait a short time for essential loading
    await new Promise(resolve => setTimeout(resolve, 800));
    
    setProgress(100);
    
    // Quick fade out
    setTimeout(() => {
      setFadeOut(true);
    }, 100);

    // Complete loading
    setTimeout(() => {
      document.body.classList.add('app-loaded');
      onLoadComplete?.();
    }, 300);
  });

  return (
    <div class={`loading-screen ${fadeOut() ? 'fade-out' : ''}`}>
      <div class="loading-logo">
        <IconRocket class="w-10 h-10 text-white" />
      </div>
      
      <h1 class="loading-title">
        Renzora Engine
      </h1>

      <div class="loading-spinner"></div>
    </div>
  );
}