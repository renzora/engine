import { editorStore } from "@/layout/stores/EditorStore";
import { viewportStore } from "@/layout/stores/ViewportStore";
import ViewportTabs from './ViewportTabs.jsx';
import Toolbar from './Toolbar.jsx';
import { viewportTypes, propertiesPanelVisible, bottomPanelVisible, footerVisible, viewportTabsVisible, toolbarVisible } from "@/api/plugin";
import { Show, createMemo, createSignal, createEffect, onCleanup } from 'solid-js';
import CodeEditorPanel from '@/pages/editor/AssetLibrary/CodeEditorPanel.jsx';
import BabylonRenderer from '@/render/index.jsx';

// Babylon.js viewport with new render system
const BabylonViewport = (props) => {
  return (
    <div 
      className="w-full h-full bg-base-300"
      style={props.style}
    >
      <BabylonRenderer />
    </div>
  );
};

const PersistentRenderViewport = (_props) => {
  return (
    <BabylonViewport
      style={{ width: '100%', height: '100%' }}
    />
  );
};

const Viewport = () => {
  const [splitViewMode, setSplitViewMode] = createSignal(false);
  const [selectedScript, setSelectedScript] = createSignal(null);
  const [editorSide, setEditorSide] = createSignal('left'); // 'left' or 'right'
  const [_scriptRuntimePlaying, _setScriptRuntimePlaying] = createSignal(true);
  const [_showLightDropdown, _setShowLightDropdown] = createSignal(false);
  
  // Track mouse button states for drag detection and cancellation
  const [rightMouseState, setRightMouseState] = createSignal({ 
    isDown: false, 
    startX: 0, 
    startY: 0, 
    hasMoved: false 
  });
  
  const [leftMouseState, setLeftMouseState] = createSignal({
    isDown: false,
    startX: 0,
    startY: 0,
    hasMoved: false
  });
  
  // Get reactive store values
  const _ui = () => editorStore.ui;
  const settings = () => editorStore.settings;
  const rightPanelWidth = () => editorStore.ui.rightPanelWidth;
  const bottomPanelHeight = () => editorStore.ui.bottomPanelHeight;
  
  const isScenePanelOpen = () => {
    return propertiesPanelVisible() && editorStore.panels.isScenePanelOpen;
  };
  
  const isAssetPanelOpen = () => {
    return bottomPanelVisible() && editorStore.panels.isAssetPanelOpen;
  };
  
  const panelPosition = () => settings().editor.panelPosition || 'right';
  const isLeftPanel = () => panelPosition() === 'left';

  const handleMouseDown = (e) => {
    if (e.button === 0) { // Left click
      // Track left mouse down
      setLeftMouseState({
        isDown: true,
        startX: e.clientX,
        startY: e.clientY,
        hasMoved: false
      });
    } else if (e.button === 2) { // Right click
      // Track right mouse down for drag detection
      setRightMouseState({
        isDown: true,
        startX: e.clientX,
        startY: e.clientY,
        hasMoved: false
      });
    }
  };

  const handleMouseMove = (e) => {
    const rightState = rightMouseState();
    const leftState = leftMouseState();
    
    // Track right mouse movement for drag detection
    if (rightState.isDown) {
      const deltaX = Math.abs(e.clientX - rightState.startX);
      const deltaY = Math.abs(e.clientY - rightState.startY);
      const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
      
      // Consider it a drag if moved more than 5 pixels
      if (distance > 5) {
        setRightMouseState(prev => ({ ...prev, hasMoved: true }));
      }
    }
    
    // Track left mouse movement 
    if (leftState.isDown) {
      const deltaX = Math.abs(e.clientX - leftState.startX);
      const deltaY = Math.abs(e.clientY - leftState.startY);
      const distance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
      
      // Consider it a drag if moved more than 5 pixels
      if (distance > 5) {
        setLeftMouseState(prev => ({ ...prev, hasMoved: true }));
      }
    }
  };

  const handleMouseUp = (e) => {
    if (e.button === 0) { // Left click release
      // Reset left mouse state
      setLeftMouseState({ isDown: false, startX: 0, startY: 0, hasMoved: false });
    } else if (e.button === 2) { // Right click release
      // Reset right mouse state
      setRightMouseState({ isDown: false, startX: 0, startY: 0, hasMoved: false });
    }
  };


  const handleDragOver = (e) => {
    // Exclude specific elements that should handle their own drops
    const shouldExclude = e.target.closest('.overflow-y-auto') || // Scene properties panels
                          e.target.closest('[data-drop-zone="scripts"]') || // Script drop zones
                          e.target.getAttribute('data-drop-zone') ||
                          e.target.tagName === 'CANVAS'; // Let canvas handle 3D model drops
    
    if (shouldExclude) {
      return;
    }
    
    // Check if this is a 3D model drag - let canvas handle it
    const dragData = window._currentDragData;
    if (dragData) {
      const extension = dragData.extension?.toLowerCase();
      if (['.glb', '.gltf', '.obj'].includes(extension)) {
        return; // Let canvas handle 3D models
      }
    }
    
    // Only handle script drag operations and avoid interfering with other drags
    try {
      // Quick check if this might be script data without parsing
      const hasData = e.dataTransfer.types.includes('application/json') || e.dataTransfer.types.includes('text/plain');
      
      if (hasData) {
        // We'll determine if it's a script in the drop handler
        // For now, allow the drag but don't set effects that conflict with other handlers
        e.preventDefault();
        e.dataTransfer.dropEffect = 'move';
      }
    } catch {
      // Silently ignore errors and let other handlers process
    }
  };

  const handleDragEnter = (e) => {
    // Exclude specific elements that should handle their own drops
    const shouldExclude = e.target.closest('.overflow-y-auto') || // Scene properties panels
                          e.target.closest('[data-drop-zone="scripts"]') || // Script drop zones
                          e.target.getAttribute('data-drop-zone') ||
                          e.target.tagName === 'CANVAS'; // Let canvas handle 3D model drops
    
    if (shouldExclude) {
      return;
    }
    
    // Check if this is a 3D model drag - let canvas handle it
    const dragData = window._currentDragData;
    if (dragData) {
      const extension = dragData.extension?.toLowerCase();
      if (['.glb', '.gltf', '.obj'].includes(extension)) {
        return; // Let canvas handle 3D models
      }
    }
    
    // Minimal interference - just prevent default for potential drops
    if (e.dataTransfer.types.includes('application/json') || e.dataTransfer.types.includes('text/plain')) {
      e.preventDefault();
    }
  };

  const handleDragLeave = (e) => {
    // Exclude specific elements that should handle their own drops
    const shouldExclude = e.target.closest('.overflow-y-auto') || // Scene properties panels
                          e.target.closest('[data-drop-zone="scripts"]') || // Script drop zones
                          e.target.getAttribute('data-drop-zone') ||
                          e.target.tagName === 'CANVAS'; // Let canvas handle 3D model drops
    
    if (shouldExclude) {
      return;
    }
    
    // Check if this is a 3D model drag - let canvas handle it
    const dragData = window._currentDragData;
    if (dragData) {
      const extension = dragData.extension?.toLowerCase();
      if (['.glb', '.gltf', '.obj'].includes(extension)) {
        return; // Let canvas handle 3D models
      }
    }
    
    // Only prevent default, don't stop propagation to avoid blocking other handlers
    if (e.dataTransfer.types.includes('application/json') || e.dataTransfer.types.includes('text/plain')) {
      e.preventDefault();
    }
  };

  const handleDrop = (e) => {
    // Exclude specific elements that should handle their own drops
    const shouldExclude = e.target.closest('.overflow-y-auto') || // Scene properties panels
                          e.target.closest('[data-drop-zone="scripts"]') || // Script drop zones
                          e.target.getAttribute('data-drop-zone') ||
                          e.target.tagName === 'CANVAS'; // Let canvas handle 3D model drops
    
    if (shouldExclude) {
      // The drop is on a child element that should handle it, let it handle the drop
      return;
    }
    
    // Check if this is a 3D model drag - let canvas handle it
    const dragData = window._currentDragData;
    if (dragData) {
      const extension = dragData.extension?.toLowerCase();
      if (['.glb', '.gltf', '.obj'].includes(extension)) {
        return; // Let canvas handle 3D models
      }
    }
    
    try {
      // Try to get data from either format
      let rawData = e.dataTransfer.getData('application/json') || e.dataTransfer.getData('text/plain');
      
      if (!rawData) {
        // No data found - completely ignore this drop
        return;
      }
      
      const data = JSON.parse(rawData);
      
      // Only handle script assets for split view, let everything else pass through
      if (data.type === 'asset' && (data.fileType === 'script' || data.category === 'scripts')) {
        e.preventDefault();
        e.stopPropagation();
        
        // Calculate which side of the viewport the drop occurred on
        const rect = e.currentTarget.getBoundingClientRect();
        const dropX = e.clientX - rect.left;
        const viewportWidth = rect.width;
        const dropSide = dropX < viewportWidth / 2 ? 'left' : 'right';
        
        // Handle script drop for split view
        
        // Set editor side based on drop position
        setEditorSide(dropSide);
        
        // Open split view with the script
        setSelectedScript({
          name: data.name,
          path: data.path
        });
        setSplitViewMode(true);
      }
      // For non-script assets, don't prevent default - let other handlers process
    } catch {
      // Failed to parse - let other handlers deal with it
      return;
    }
  };

  const closeSplitView = () => {
    setSplitViewMode(false);
    setSelectedScript(null);
  };

  const toggleEditorSide = () => {
    setEditorSide(current => current === 'left' ? 'right' : 'left');
  };

  // Listen for context menu "Open in Viewport" events and global mouse events
  createEffect(() => {
    const handleOpenInViewport = (event) => {
      const { asset: _asset, side, script } = event.detail;
      // Open asset in viewport split view
      
      // Set editor side
      setEditorSide(side);
      
      // Open split view with the script
      setSelectedScript(script);
      setSplitViewMode(true);
    };

    // Global mouse handlers to track drag state even when mouse leaves viewport
    const handleGlobalMouseMove = (e) => {
      handleMouseMove(e);
    };

    const handleGlobalMouseUp = (e) => {
      handleMouseUp(e);
    };

    const handleGlobalMouseDown = (e) => {
      handleMouseDown(e);
    };

    document.addEventListener('asset:open-in-viewport', handleOpenInViewport);
    document.addEventListener('mousedown', handleGlobalMouseDown);
    document.addEventListener('mousemove', handleGlobalMouseMove);
    document.addEventListener('mouseup', handleGlobalMouseUp);
    
    onCleanup(() => {
      document.removeEventListener('asset:open-in-viewport', handleOpenInViewport);
      document.removeEventListener('mousedown', handleGlobalMouseDown);
      document.removeEventListener('mousemove', handleGlobalMouseMove);
      document.removeEventListener('mouseup', handleGlobalMouseUp);
    });
  });
  
  
  const getViewportPositioning = () => {
    const top = '0px';
    const left = isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0px';
    const right = !isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0px';
    const bottomPanelSpace = isAssetPanelOpen() ? `${bottomPanelHeight()}px` : (bottomPanelVisible() ? '32px' : '0px');
    const footerHeight = footerVisible() ? '24px' : '0px'; // 6 * 4 = 24px (h-6 in Tailwind)
    // Match the bottom panel positioning: both positions use same adjustment
    const bottomAdjustment = '-1px';
    const bottom = bottomPanelSpace === '0px' ? `calc(${footerHeight} + ${bottomAdjustment})` : `calc(${bottomPanelSpace} + ${footerHeight} + ${bottomAdjustment})`;
    
    return { top, left, right, bottom };
  };
  
  const activeTab = createMemo(() => {
    const tab = viewportStore.tabs.find(tab => tab.id === viewportStore.activeTabId);
    // Track active viewport tab
    return tab;
  });
  
  
  const isOverlayActive = createMemo(() => {
    const active = activeTab() && activeTab().type !== '3d-viewport';
    // Determine if overlay should be shown
    return active;
  });
  
  const renderOverlayPanel = (tab) => {
    if (!tab) return null;
    
    // Render overlay panel for current tab
    
    switch (tab.type) {
      default:
        // Check if this is a plugin viewport type
        // Check for plugin-registered viewport type
        const pluginViewportType = viewportTypes().get(tab.type);
        if (pluginViewportType && pluginViewportType.component) {
          const PluginComponent = pluginViewportType.component;
          // Render plugin component
          
          // All plugin viewports render without headers
          return (
            <div className="absolute inset-0 bg-base-100">
              <PluginComponent tab={tab} />
            </div>
          );
        }
        
        return (
          <div className="absolute inset-0 bg-base-100 flex items-center justify-center">
            <div className="text-center">
              <div className="text-lg text-base-content/60 mb-2">Unknown Overlay</div>
              <div className="text-sm text-base-content/50">Overlay type "{tab.type}" not found</div>
            </div>
          </div>
        );
    }
  };

  return (
    <div 
      class="absolute pointer-events-auto viewport-container"
      style={getViewportPositioning()}
    >
      <div className="w-full h-full flex flex-col gap-0">
        <Show when={toolbarVisible()}>
          <Toolbar />
        </Show>
        <Show when={viewportTabsVisible()}>
          <ViewportTabs />
        </Show>
        <div 
          className="flex-1 relative overflow-hidden"
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          // Only attach drag handlers when NOT in split view mode to avoid interfering with scene properties
          {...(!splitViewMode() && {
            onDragOver: handleDragOver,
            onDragEnter: handleDragEnter,
            onDragLeave: handleDragLeave,
            onDrop: handleDrop
          })}
        >
          {/* Single Persistent Viewport */}
          <div 
            className={`${splitViewMode() ? 
              (editorSide() === 'left' ? 'w-1/2 ml-auto' : 'w-1/2 border-r border-base-300') : 
              'w-full'
            } bg-base-100 h-full overflow-hidden`}
            {...(splitViewMode() && {
              onDragOver: handleDragOver,
              onDragEnter: handleDragEnter, 
              onDragLeave: handleDragLeave,
              onDrop: handleDrop
            })}
          >
            <div class="relative w-full h-full">
              <PersistentRenderViewport
                showGrid={true}
              />
            </div>
            
            <Show when={isOverlayActive()}>
              {renderOverlayPanel(activeTab())}
            </Show>
          </div>

          {/* Code Editor Panel (only shown in split view) */}
          <Show when={splitViewMode()}>
            <div 
              className={`w-1/2 bg-base-100 absolute top-0 h-full ${
                editorSide() === 'left' ? 'left-0 border-r border-base-300' : 'right-0'
              }`}
            >
              <CodeEditorPanel
                isOpen={() => true}
                onClose={closeSplitView}
                selectedFile={selectedScript}
                width={400}
                onToggleSide={toggleEditorSide}
                currentSide={editorSide()}
              />
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
};

export default Viewport;