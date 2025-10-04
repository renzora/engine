import { createEffect, onCleanup, createSignal } from 'solid-js';
import { useViewportContextMenu } from '@/ui/ViewportContextMenu.jsx';

// Component to handle global keyboard shortcuts for context menus
const KeyboardShortcuts = () => {
  const { showContextMenu } = useViewportContextMenu();
  const [mousePosition, setMousePosition] = createSignal({ x: 0, y: 0 });

  createEffect(() => {
    const handleMouseMove = (e) => {
      setMousePosition({ x: e.clientX, y: e.clientY });
    };

    const handleKeyDown = (e) => {
      // Handle Shift+A for context menu in panels (not render viewport)
      if (e.shiftKey && e.key.toLowerCase() === 'a') {
        const mousePos = mousePosition();
        
        // Get the element at the current mouse position
        const elementAtMouse = document.elementFromPoint(mousePos.x, mousePos.y);
        
        
        // Check if mouse is over any panel by traversing up the DOM
        let currentElement = elementAtMouse;
        let context = null;
        
        while (currentElement && currentElement !== document.body) {
          const className = currentElement.className || '';
          
          // Check for right panel (scene hierarchy) - look for Scene component or right panel container
          if (className.includes('absolute') && className.includes('top-0') && className.includes('right-0')) {
            context = 'scene';
            break;
          }
          
          // Check for bottom panel - look for asset library or bottom panel container
          if (className.includes('absolute') && className.includes('pointer-events-auto') && 
              (className.includes('no-select') || className.includes('z-'))) {
            context = 'bottom-panel';
            break;
          }
          
          // Also check for specific component markers
          if (currentElement.querySelector && (
              currentElement.querySelector('[class*="Scene"]') ||
              currentElement.querySelector('[data-scene-panel]') ||
              currentElement.textContent?.includes('Scene')
          )) {
            context = 'scene';
            break;
          }
          
          if (currentElement.querySelector && (
              currentElement.querySelector('[class*="AssetLibrary"]') ||
              currentElement.querySelector('[data-asset-panel]') ||
              currentElement.textContent?.includes('Assets')
          )) {
            context = 'bottom-panel';
            break;
          }
          
          currentElement = currentElement.parentElement;
        }
        
        // Only show context menu if we found a valid panel context
        if (context) {
          e.preventDefault();
          e.stopPropagation();
          
          // Create a synthetic mouse event at the current mouse position
          const syntheticEvent = {
            clientX: mousePos.x,
            clientY: mousePos.y,
            preventDefault: () => {},
            stopPropagation: () => {}
          };
          
          // Show context menu at mouse position
          showContextMenu(syntheticEvent, null, context);
        }
      }
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('keydown', handleKeyDown);
    
    onCleanup(() => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('keydown', handleKeyDown);
    });
  });

  // This component doesn't render anything
  return null;
};

export default KeyboardShortcuts;