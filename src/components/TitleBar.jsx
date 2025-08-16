import { createSignal, onMount } from 'solid-js';

const TitleBar = () => {
  const [isMaximized, setIsMaximized] = createSignal(false);
  let titleBarRef;

  // Check if we're running in Tauri
  const isTauri = () => typeof window !== 'undefined' && window.__TAURI_IPC__;

  onMount(async () => {
    if (isTauri()) {
      // Add data attribute to body for Tauri-specific styling
      document.body.setAttribute('data-tauri', 'true');
      
      // Import Tauri APIs only when in Tauri context
      const { appWindow } = await import('@tauri-apps/api/window');
      
      // Check initial maximized state
      const maximized = await appWindow.isMaximized();
      setIsMaximized(maximized);
      
      // Listen for window resize events
      await appWindow.listen('tauri://resize', async () => {
        const maximized = await appWindow.isMaximized();
        setIsMaximized(maximized);
      });
    }
  });

  const handleMinimize = async () => {
    if (isTauri()) {
      const { appWindow } = await import('@tauri-apps/api/window');
      await appWindow.minimize();
    }
  };

  const handleMaximize = async () => {
    if (isTauri()) {
      const { appWindow } = await import('@tauri-apps/api/window');
      await appWindow.toggleMaximize();
    }
  };

  const handleClose = async () => {
    if (isTauri()) {
      const { appWindow } = await import('@tauri-apps/api/window');
      await appWindow.close();
    }
  };

  return (
    <div class="title-bar">
      {/* Draggable area */}
      <div 
        ref={titleBarRef}
        class="title-bar-drag-region"
        data-tauri-drag-region
      >
        <div class="title-bar-title">
          Renzora
        </div>
      </div>
      
      {/* Window controls */}
      <div class="title-bar-controls">
        <button 
          class="title-bar-button minimize-button"
          onClick={handleMinimize}
          title="Minimize"
        >
          <svg width="10" height="10" viewBox="0 0 10 10">
            <rect x="0" y="4" width="10" height="1" fill="currentColor" />
          </svg>
        </button>
        
        <button 
          class="title-bar-button maximize-button"
          onClick={handleMaximize}
          title={isMaximized() ? "Restore Down" : "Maximize"}
        >
          {isMaximized() ? (
            <svg width="10" height="10" viewBox="0 0 10 10">
              <rect x="1" y="1" width="7" height="7" fill="none" stroke="currentColor" stroke-width="1" />
              <rect x="2" y="0" width="7" height="7" fill="none" stroke="currentColor" stroke-width="1" />
            </svg>
          ) : (
            <svg width="10" height="10" viewBox="0 0 10 10">
              <rect x="0" y="0" width="10" height="10" fill="none" stroke="currentColor" stroke-width="1" />
            </svg>
          )}
        </button>
        
        <button 
          class="title-bar-button close-button"
          onClick={handleClose}
          title="Close"
        >
          <svg width="10" height="10" viewBox="0 0 10 10">
            <line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" stroke-width="1" />
            <line x1="10" y1="0" x2="0" y2="10" stroke="currentColor" stroke-width="1" />
          </svg>
        </button>
      </div>
    </div>
  );
};

export default TitleBar;