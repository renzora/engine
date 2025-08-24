import { editorStore } from "@/layout/stores/EditorStore";
import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";
import { Settings, X } from '@/ui/icons';
import ViewportTabs from './ViewportTabs.jsx';
import { ViewportCanvas } from '@/render/babylonjs';
import { VulkanViewport } from '@/render/custom-vulkan/VulkanViewport.jsx';
import { BabylonNativeViewport } from '@/render/babylon-native/BabylonNativeViewport.jsx';
import { ThreeViewport } from '@/render/threejs/ThreeViewport.jsx';
import { PlayCanvasViewport } from '@/render/playcanvas/PlayCanvasViewport.jsx';
import { PixiViewport } from '@/render/pixijs/PixiViewport.jsx';
import { PhaserViewport } from '@/render/phaser/PhaserViewport.jsx';
import { MelonViewport } from '@/render/melonjs/MelonViewport.jsx';
import { viewportTypes, propertiesPanelVisible, bottomPanelVisible } from "@/api/plugin";
import { Show, createMemo, createSignal, createEffect, onCleanup } from 'solid-js';
import CodeEditorPanel from '@/pages/editor/AssetLibrary/CodeEditorPanel.jsx';

const PersistentRenderViewport = (props) => {
  const [currentRenderer, setCurrentRenderer] = createSignal('babylon');
  
  console.log('🔧 PersistentRenderViewport: Component created/mounted');

  // Listen for renderer changes
  createEffect(() => {
    const handleRendererChange = (event) => {
      console.log('🔄 Viewport: Renderer changed to:', event.detail.renderer);
      setCurrentRenderer(event.detail.renderer);
    };

    window.addEventListener('renderer-changed', handleRendererChange);
    
    onCleanup(() => {
      console.log('🧹 PersistentRenderViewport: Component unmounting/cleanup');
      window.removeEventListener('renderer-changed', handleRendererChange);
    });
  });
  
  return (
    <>
      {(() => {
        const renderer = currentRenderer();
        console.log('🎨 Viewport: Current renderer is:', renderer);
        
        if (renderer === 'vulkan') {
          console.log('🎮 Viewport: Rendering VulkanViewport');
          return (
            <VulkanViewport
              onContextMenu={props.contextMenuHandler}
              style={{ width: '100%', height: '100%' }}
              showStats={editorStore.settings.editor.showStats}
            />
          );
        } else if (renderer === 'babylon-native') {
          console.log('🏛️ Viewport: Rendering BabylonNativeViewport');
          return (
            <BabylonNativeViewport
              onContextMenu={props.contextMenuHandler}
              style={{ width: '100%', height: '100%' }}
              showStats={editorStore.settings.editor.showStats}
            />
          );
        } else if (renderer === 'threejs') {
          console.log('🎮 Viewport: Rendering ThreeViewport');
          return (
            <ThreeViewport
              onContextMenu={props.contextMenuHandler}
              style={{ width: '100%', height: '100%' }}
              showStats={editorStore.settings.editor.showStats}
            />
          );
        } else if (renderer === 'playcanvas') {
          console.log('🎯 Viewport: Rendering PlayCanvasViewport');
          return (
            <PlayCanvasViewport
              onContextMenu={props.contextMenuHandler}
              style={{ width: '100%', height: '100%' }}
              showStats={editorStore.settings.editor.showStats}
            />
          );
        } else if (renderer === 'pixijs') {
          console.log('🎨 Viewport: Rendering PixiViewport');
          return (
            <PixiViewport
              onContextMenu={props.contextMenuHandler}
              style={{ width: '100%', height: '100%' }}
              showStats={editorStore.settings.editor.showStats}
            />
          );
        } else if (renderer === 'phaser') {
          console.log('🎮 Viewport: Rendering PhaserViewport');
          return (
            <PhaserViewport
              onContextMenu={props.contextMenuHandler}
              style={{ width: '100%', height: '100%' }}
              showStats={editorStore.settings.editor.showStats}
            />
          );
        } else if (renderer === 'melonjs') {
          console.log('🍉 Viewport: Rendering MelonViewport');
          return (
            <MelonViewport
              onContextMenu={props.contextMenuHandler}
              style={{ width: '100%', height: '100%' }}
              showStats={editorStore.settings.editor.showStats}
            />
          );
        } else {
          console.log('🌐 Viewport: Rendering ViewportCanvas');
          return (
            <ViewportCanvas
              onContextMenu={props.contextMenuHandler} 
              style={{ width: '100%', height: '100%' }}
            />
          );
        }
      })()}
    </>
  );
};

const Viewport = () => {
  const [contextMenu, setContextMenu] = createSignal(null);
  const [splitViewMode, setSplitViewMode] = createSignal(false);
  const [selectedScript, setSelectedScript] = createSignal(null);
  const [editorSide, setEditorSide] = createSignal('left'); // 'left' or 'right'
  
  // Get reactive store values
  const ui = () => editorStore.ui;
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

  const handleContextMenu = (e, item, context = 'scene') => {
    if (!e) return;
    
    e.preventDefault();
    e.stopPropagation();

    const { clientX: x, clientY: y } = e;
    setContextMenu({
      position: { x, y },
      items: [
        { label: 'Create Object', action: () => console.log('Create object') },
        { label: 'Delete', action: () => console.log('Delete') }
      ],
    });
  };

  const handleDragOver = (e) => {
    // Exclude specific elements that should handle their own drops
    const shouldExclude = e.target.closest('.overflow-y-auto') || // Scene properties panels
                          e.target.closest('[data-drop-zone="scripts"]') || // Script drop zones
                          e.target.getAttribute('data-drop-zone'); // Any element marked as drop zone
    
    if (shouldExclude) {
      return;
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
    } catch (error) {
      // Silently ignore errors and let other handlers process
    }
  };

  const handleDragEnter = (e) => {
    // Exclude specific elements that should handle their own drops
    const shouldExclude = e.target.closest('.overflow-y-auto') || // Scene properties panels
                          e.target.closest('[data-drop-zone="scripts"]') || // Script drop zones
                          e.target.getAttribute('data-drop-zone'); // Any element marked as drop zone
    
    if (shouldExclude) {
      return;
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
                          e.target.getAttribute('data-drop-zone'); // Any element marked as drop zone
    
    if (shouldExclude) {
      return;
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
                          e.target.getAttribute('data-drop-zone'); // Any element marked as drop zone
    
    if (shouldExclude) {
      // The drop is on a child element that should handle it, let it handle the drop
      return;
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
        
        console.log(`Script dropped on ${dropSide} side of viewport for split view:`, data);
        
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
    } catch (error) {
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

  // Listen for context menu "Open in Viewport" events
  createEffect(() => {
    const handleOpenInViewport = (event) => {
      const { asset, side, script } = event.detail;
      console.log(`🎯 Context menu: Opening ${asset.name} in viewport (${side} side)`);
      
      // Set editor side
      setEditorSide(side);
      
      // Open split view with the script
      setSelectedScript(script);
      setSplitViewMode(true);
    };

    document.addEventListener('asset:open-in-viewport', handleOpenInViewport);
    
    onCleanup(() => {
      document.removeEventListener('asset:open-in-viewport', handleOpenInViewport);
    });
  });
  
  const getViewportPositioning = () => {
    const top = '0px';
    const left = isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0px';
    const right = !isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0px';
    const bottomPanelSpace = isAssetPanelOpen() ? `${bottomPanelHeight()}px` : (bottomPanelVisible() ? '40px' : '0px');
    const footerHeight = '24px'; // 6 * 4 = 24px (h-6 in Tailwind)
    const bottom = bottomPanelSpace === '0px' ? footerHeight : `calc(${bottomPanelSpace} + ${footerHeight})`;
    
    return { top, left, right, bottom };
  };
  
  const activeTab = createMemo(() => {
    const tab = viewportStore.tabs.find(tab => tab.id === viewportStore.activeTabId);
    console.log('🎯 ViewportContainer - Active tab:', tab);
    console.log('🎯 ViewportContainer - All tabs:', viewportStore.tabs);
    console.log('🎯 ViewportContainer - Active tab ID:', viewportStore.activeTabId);
    return tab;
  });
  const isOverlayActive = createMemo(() => {
    const active = activeTab() && activeTab().type !== '3d-viewport';
    console.log('🎯 ViewportContainer - Overlay active:', active, activeTab()?.type);
    return active;
  });
  
  const renderOverlayPanel = (tab) => {
    if (!tab) return null;
    
    console.log('🎯 Rendering overlay panel for tab type:', tab.type);
    
    switch (tab.type) {
        
      default:
        // Check if this is a plugin viewport type
        console.log('🎯 Checking for plugin viewport type:', tab.type);
        console.log('🎯 Available viewport types:', Array.from(viewportTypes().keys()));
        const pluginViewportType = viewportTypes().get(tab.type);
        console.log('🎯 Found plugin viewport type:', pluginViewportType);
        if (pluginViewportType && pluginViewportType.component) {
          const PluginComponent = pluginViewportType.component;
          console.log('🎯 Rendering plugin component for:', tab.type);
          
          // Special handling for splash viewport - no header
          if (tab.type === 'splash-viewport') {
            return (
              <div className="absolute inset-0 bg-base-100">
                <PluginComponent tab={tab} />
              </div>
            );
          }
          
          return (
            <div className="absolute inset-0 bg-base-100 flex flex-col">
              <div className="flex items-center justify-between p-3 border-b border-base-300 bg-base-200">
                <div className="flex items-center gap-2">
                  <Show when={pluginViewportType.icon} fallback={<Settings class="w-4 h-4 text-base-content/60" />}>
                    <div class="w-4 h-4 text-base-content/60">
                      <pluginViewportType.icon class="w-4 h-4" />
                    </div>
                  </Show>
                  <span className="text-sm font-medium text-base-content">{tab.name}</span>
                </div>
                <button
                  onClick={() => {
                    const threeDTab = viewportStore.tabs.find(t => t.type === '3d-viewport');
                    if (threeDTab) {
                      viewportActions.setActiveViewportTab(threeDTab.id);
                    }
                  }}
                  className="p-1 hover:bg-base-300 rounded transition-colors"
                  title="Close overlay (return to 3D view)"
                >
                  <X class="w-4 h-4 text-base-content/60" />
                </button>
              </div>
              
              <div className="flex-1 overflow-hidden">
                <PluginComponent tab={tab} />
              </div>
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
      <div className="w-full h-full flex flex-col bg-base-100">
        <ViewportTabs />
        <div 
          className="flex-1 relative overflow-hidden"
          onContextMenu={(e) => {
            // Only prevent default if the target doesn't have its own context menu
            if (!e.target.closest('.asset-context-menu') && e.target === e.currentTarget) {
              e.preventDefault();
            }
          }}
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
            } bg-base-100 h-full`}
            {...(splitViewMode() && {
              onDragOver: handleDragOver,
              onDragEnter: handleDragEnter, 
              onDragLeave: handleDragLeave,
              onDrop: handleDrop
            })}
          >
            <PersistentRenderViewport
              contextMenuHandler={() => handleContextMenu}
              showGrid={true}
            />
            
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