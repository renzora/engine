import { editorStore, editorActions } from "@/layout/stores/EditorStore";
import { viewportStore, viewportActions, objectPropertiesActions } from "@/layout/stores/ViewportStore";
import { Settings, X, Pointer, Move, Refresh, Maximize, Video, Copy, Trash, Box, Circle, Rectangle, Sun, Lightbulb } from '@/ui/icons';
import { Play, Pause } from '@/ui/icons/media';
import ViewportTabs from './ViewportTabs.jsx';
import { viewportTypes, propertiesPanelVisible, bottomPanelVisible, footerVisible, viewportTabsVisible, pluginAPI } from "@/api/plugin";
import { Show, createMemo, createSignal, createEffect, onCleanup, For } from 'solid-js';
import CodeEditorPanel from '@/pages/editor/AssetLibrary/CodeEditorPanel.jsx';
import BabylonRenderer from '@/render/index.jsx';
import { renderStore, renderActions } from '@/render/store.jsx';
import { getScriptRuntime } from '@/api/script';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { Ray } from '@babylonjs/core/Culling/ray';
import { TransformNode } from '@babylonjs/core/Meshes/transformNode';
import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { Color3 } from '@babylonjs/core/Maths/math.color';
import { PointLight } from '@babylonjs/core/Lights/pointLight';
import { SpotLight } from '@babylonjs/core/Lights/spotLight';
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight';
import { DirectionalLight } from '@babylonjs/core/Lights/directionalLight';
import { UniversalCamera } from '@babylonjs/core/Cameras/universalCamera';
import '@babylonjs/core/Meshes/Builders/boxBuilder';
import '@babylonjs/core/Meshes/Builders/sphereBuilder';
import '@babylonjs/core/Meshes/Builders/cylinderBuilder';
import '@babylonjs/core/Meshes/Builders/planeBuilder';

// Babylon.js viewport with new render system
const BabylonViewport = (props) => {
  return (
    <div 
      className="w-full h-full bg-base-300"
      onContextMenu={props.onContextMenu}
      style={props.style}
    >
      <BabylonRenderer onContextMenu={props.onContextMenu} />
    </div>
  );
};

const PersistentRenderViewport = (props) => {
  return (
    <BabylonViewport
      onContextMenu={props.contextMenuHandler} 
      style={{ width: '100%', height: '100%' }}
    />
  );
};

const Viewport = () => {
  const [contextMenu, setContextMenu] = createSignal(null);
  const [splitViewMode, setSplitViewMode] = createSignal(false);
  const [selectedScript, setSelectedScript] = createSignal(null);
  const [editorSide, setEditorSide] = createSignal('left'); // 'left' or 'right'
  const [scriptRuntimePlaying, setScriptRuntimePlaying] = createSignal(true);
  const [showLightDropdown, setShowLightDropdown] = createSignal(false);
  
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
    } catch (error) {
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
  
  // Toolbar functionality
  const selectedTool = () => editorStore.ui.selectedTool;
  const selection = () => editorStore.selection;
  const selectedEntity = () => selection().entity;
  const transformMode = () => selection().transformMode;
  
  const { setSelectedTool, setTransformMode, selectEntity } = editorActions;
  
  const getSelectedTool = () => {
    if (['select', 'move', 'rotate', 'scale'].includes(transformMode())) {
      return transformMode();
    }
    return selectedTool();
  };
  
  const getCurrentScene = () => {
    return renderStore.scene;
  };
  
  const getObjectName = (type) => {
    return type.toLowerCase();
  };
  
  const createBabylonPrimitive = async (type) => {
    const scene = getCurrentScene();
    if (!scene) {
      editorActions.addConsoleMessage('No active scene available', 'error');
      return;
    }

    const position = new Vector3(0, 0.5, 0);
    const objectName = getObjectName(type);
    
    try {
      const mainContainer = new TransformNode(objectName, scene);
      mainContainer.position = position;
      let mesh;
      const meshName = `${objectName}_mesh`;
      
      switch (type) {
        case 'cube':
          mesh = MeshBuilder.CreateBox(meshName, { size: 1 }, scene);
          break;
        case 'sphere':
          mesh = MeshBuilder.CreateSphere(meshName, { diameter: 1 }, scene);
          break;
        case 'cylinder':
          mesh = MeshBuilder.CreateCylinder(meshName, { height: 1, diameter: 1 }, scene);
          break;
        case 'plane':
          mesh = MeshBuilder.CreatePlane(meshName, { size: 1 }, scene);
          mesh.rotation.x = Math.PI / 2;
          break;
      }
      
      if (mesh) {
        mesh.parent = mainContainer;
        mesh.position = Vector3.Zero();
        const material = new StandardMaterial(`${objectName}_material`, scene);
        material.diffuseColor = new Color3(0.7, 0.7, 0.9);
        material.specularColor = new Color3(0.2, 0.2, 0.2);
        mesh.material = material;
        
        renderActions.addObject(mainContainer);
        renderActions.selectObject(mainContainer);
        renderActions.setTransformMode('move');
        
        const objectId = mainContainer.uniqueId || mainContainer.name;
        objectPropertiesActions.ensureDefaultComponents(objectId);
        objectPropertiesActions.updateObjectProperty(objectId, 'transform.position', [position.x, position.y, position.z]);
        objectPropertiesActions.updateObjectProperty(objectId, 'transform.rotation', [0, 0, 0]);
        const scaleValue = type === 'plane' ? [1, 0.01, 1] : [1, 1, 1];
        objectPropertiesActions.updateObjectProperty(objectId, 'transform.scale', scaleValue);
        
        editorActions.selectEntity(objectId);
        setTransformMode('move');
        
        editorActions.addConsoleMessage(`Created ${type}`, 'success');
      }
    } catch (error) {
      editorActions.addConsoleMessage(`Failed to create ${type}: ${error.message}`, 'error');
    }
  };
  
  const createBabylonLight = async (lightType = 'directional') => {
    const scene = getCurrentScene();
    if (!scene) {
      editorActions.addConsoleMessage('No active scene available', 'error');
      return;
    }

    try {
      const lightName = getObjectName('light');
      const lightPosition = new Vector3(0, 4, 0);
      
      const mainContainer = new TransformNode(lightName, scene);
      mainContainer.position = lightPosition;
      
      let light;
      switch (lightType) {
        case 'point':
          light = new PointLight(`${lightName}_light`, Vector3.Zero(), scene);
          light.diffuse = new Color3(1, 0.95, 0.8);
          light.intensity = 10;
          break;
        case 'spot':
          light = new SpotLight(`${lightName}_light`, Vector3.Zero(), new Vector3(0, -1, 0), Math.PI / 3, 2, scene);
          light.diffuse = new Color3(1, 0.95, 0.8);
          light.intensity = 15;
          break;
        case 'hemisphere':
          light = new HemisphericLight(`${lightName}_light`, new Vector3(0, 1, 0), scene);
          light.diffuse = new Color3(1, 0.95, 0.8);
          light.intensity = 0.7;
          break;
        default:
          light = new DirectionalLight(`${lightName}_light`, new Vector3(-1, -1, -1), scene);
          light.diffuse = new Color3(1, 0.95, 0.8);
          light.intensity = 1;
          break;
      }
      
      light.position = Vector3.Zero();
      light.parent = mainContainer;
      
      renderActions.addObject(mainContainer);
      renderActions.selectObject(mainContainer);
      
      const objectId = mainContainer.uniqueId || mainContainer.name;
      objectPropertiesActions.ensureDefaultComponents(objectId);
      objectPropertiesActions.updateObjectProperty(objectId, 'transform.position', [lightPosition.x, lightPosition.y, lightPosition.z]);
      
      editorActions.selectEntity(objectId);
      editorActions.addConsoleMessage(`Created ${lightType} light`, 'success');
    } catch (error) {
      editorActions.addConsoleMessage(`Failed to create light: ${error.message}`, 'error');
    }
  };
  
  const handleToolbarClick = async (toolId) => {
    if (['select', 'move', 'rotate', 'scale'].includes(toolId)) {
      if (toolId !== 'select' && !selectedEntity()) {
        editorActions.addConsoleMessage('Please select an object first', 'warning');
        return;
      }
      setTransformMode(toolId);
      renderActions.setTransformMode(toolId);
    }
    else if (['cube', 'sphere', 'cylinder', 'plane'].includes(toolId)) {
      await createBabylonPrimitive(toolId);
    }
    else if (toolId === 'light') {
      await createBabylonLight();
    }
    else if (toolId === 'camera') {
      // Camera creation logic would go here
    }
    else if (toolId === 'duplicate') {
      if (!selectedEntity()) {
        editorActions.addConsoleMessage('Please select an object to duplicate', 'warning');
        return;
      }
      // Duplicate logic would go here
    }
    else if (toolId === 'delete') {
      if (!selectedEntity()) {
        editorActions.addConsoleMessage('Please select an object to delete', 'warning');
        return;
      }
      // Delete logic would go here
    }
  };
  
  const getViewportPositioning = () => {
    const top = '0px';
    const left = isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0px';
    const right = !isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0px';
    const bottomPanelSpace = isAssetPanelOpen() ? `${bottomPanelHeight()}px` : (bottomPanelVisible() ? '40px' : '0px');
    const footerHeight = footerVisible() ? '24px' : '0px'; // 6 * 4 = 24px (h-6 in Tailwind)
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
        <Show when={viewportTabsVisible()}>
          <ViewportTabs />
        </Show>
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
                contextMenuHandler={() => handleContextMenu}
                showGrid={true}
              />
              
              {/* Vertical Toolbar in Viewport */}
              <div class="absolute top-4 left-0 flex flex-col gap-0.5 bg-base-300 rounded-tr rounded-br p-1">
                <For each={[
                  { id: 'select', icon: Pointer, tooltip: 'Select' },
                  { id: 'move', icon: Move, tooltip: 'Move' },
                  { id: 'rotate', icon: Refresh, tooltip: 'Rotate' },
                  { id: 'scale', icon: Maximize, tooltip: 'Scale' },
                  null, // Separator
                  { id: 'cube', icon: Box, tooltip: 'Add Cube' },
                  { id: 'sphere', icon: Circle, tooltip: 'Add Sphere' },
                  { id: 'cylinder', icon: Box, tooltip: 'Add Cylinder' },
                  { id: 'plane', icon: Rectangle, tooltip: 'Add Plane' },
                  { id: 'light', icon: Sun, tooltip: 'Add Light' },
                  { id: 'camera', icon: Video, tooltip: 'Add Camera' },
                  null, // Separator
                  { id: 'duplicate', icon: Copy, tooltip: 'Duplicate' },
                  { id: 'delete', icon: Trash, tooltip: 'Delete' }
                ]}>
                  {(tool) => 
                    tool === null ? (
                      <div class="w-full h-px bg-base-content/20 my-1"></div>
                    ) : (
                      <button 
                        onClick={() => handleToolbarClick(tool.id)}
                        class={`w-8 h-8 flex items-center justify-center rounded transition-all group ${
                          getSelectedTool() === tool.id
                            ? 'bg-primary text-primary-content'
                            : 'text-base-content/60 hover:text-base-content hover:bg-base-300'
                        }`} 
                        title={tool.tooltip}
                      >
                        <tool.icon class="w-5 h-5" />
                        
                        <div class="absolute left-full ml-2 bg-base-200 text-base-content text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                          {tool.tooltip}
                        </div>
                      </button>
                    )
                  }
                </For>
              </div>
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