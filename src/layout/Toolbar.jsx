import { createSignal, For, Show } from 'solid-js';
import Helper from './Helper.jsx';
import { helperVisible } from '@/api/plugin';
import { editorStore, editorActions } from "@/layout/stores/EditorStore";
import { IconPointer, IconArrowsMove, IconRefresh, IconMaximize, IconVideo, IconCopy, IconTrash, IconBox, IconCircle, IconCylinder, IconSquare, IconSun, IconBulb, IconPlayerPlay, IconPlayerPause, IconChevronDown, IconCube, IconBrush, IconMountain, IconTriangle, IconRectangle } from '@tabler/icons-solidjs';
import { renderStore, renderActions } from '@/render/store.jsx';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { Ray } from '@babylonjs/core/Culling/ray';
import { createAndAddObject } from '@/api/creation/ObjectCreationUtils.jsx';
import '@babylonjs/core/Meshes/Builders/boxBuilder';
import '@babylonjs/core/Meshes/Builders/sphereBuilder';
import '@babylonjs/core/Meshes/Builders/cylinderBuilder';
import '@babylonjs/core/Meshes/Builders/planeBuilder';

function Toolbar() {
  // Access store properties reactively
  const selection = () => editorStore.selection;
  const selectedEntity = () => selection().entity;
  const selectedTool = () => editorStore.ui.selectedTool;
  const transformMode = () => selection().transformMode;
  
  const { setSelectedTool, setTransformMode } = editorActions;
  
  // Camera view dropdown state
  const [cameraViewDropdownOpen, setCameraViewDropdownOpen] = createSignal(false);
  const [_currentViewName, setCurrentViewName] = createSignal("View");
  
  // Initialize global camera view name
  window._currentCameraViewName = "Camera";
  
  // Mode dropdown state
  const [modeDropdownOpen, setModeDropdownOpen] = createSignal(false);
  const currentMode = () => editorStore.ui.currentMode;
  
  // Light dropdown state
  const [lightDropdownOpen, setLightDropdownOpen] = createSignal(false);
  
  // Light type definitions
  const lightTypes = [
    { id: 'directional', name: 'Directional Light', icon: IconSun, description: 'Uniform lighting from a direction (like sunlight)' },
    { id: 'point', name: 'Point Light', icon: IconBulb, description: 'Light radiating from a single point' },
    { id: 'spot', name: 'Spot Light', icon: IconTriangle, description: 'Cone-shaped directional light' },
    { id: 'hemispheric', name: 'Hemispheric Light', icon: IconCircle, description: 'Ambient hemisphere lighting' },
    { id: 'rectArea', name: 'Rectangular Area Light', icon: IconRectangle, description: 'Light emitted from a rectangular surface' }
  ];
  
  // Brush settings for sculpting mode
  const getBrushSize = () => {
    const selectedObject = renderStore.selectedObject;
    if (selectedObject && selectedObject._terrainData) {
      return selectedObject._terrainData.brushSize || 8;
    }
    return 8; // Default brush size
  };
  
  const setBrushSize = (size) => {
    const selectedObject = renderStore.selectedObject;
    if (selectedObject && selectedObject._terrainData) {
      selectedObject._terrainData.brushSize = size;
      editorActions.addConsoleMessage(`Brush size: ${size.toFixed(1)}`, 'info');
    }
  };
  
  const getBrushStrength = () => {
    const selectedObject = renderStore.selectedObject;
    if (selectedObject && selectedObject._terrainData) {
      return selectedObject._terrainData.brushStrength || 0.2;
    }
    return 0.2; // Default brush strength
  };
  
  const setBrushStrength = (strength) => {
    const selectedObject = renderStore.selectedObject;
    if (selectedObject && selectedObject._terrainData) {
      selectedObject._terrainData.brushStrength = strength;
      editorActions.addConsoleMessage(`Brush strength: ${(strength * 100).toFixed(0)}%`, 'info');
    }
  };
  
  // Get user-friendly view names
  const getViewDisplayName = (viewType) => {
    const viewNames = {
      front: "Front",
      back: "Back", 
      right: "Right",
      left: "Left",
      top: "Top",
      bottom: "Bottom",
      frontLeft: "Front Left",
      frontRight: "Front Right"
    };
    return viewNames[viewType] || "View";
  };

  // Get user-friendly mode names and icons
  const getModeDisplayData = (mode) => {
    const modeData = {
      standard: { name: "Scene Editor", icon: IconCube, description: "Design scenes, add models, and transform objects" },
      sculpting: { name: "Sculpting", icon: IconBrush, description: "Terrain and mesh sculpting" }
    };
    return modeData[mode] || { name: "Unknown", icon: IconCube, description: "" };
  };
  
  const getSelectedTool = () => {
    if (['select', 'move', 'rotate', 'scale'].includes(transformMode())) {
      return transformMode();
    }
    if (['terrain_raise', 'terrain_lower', 'terrain_smooth', 'terrain_flatten', 'terrain_paint', 'terrain_noise'].includes(selectedTool())) {
      return selectedTool();
    }
    return selectedTool();
  };
  
  const getCurrentScene = () => {
    return renderStore.scene;
  };
  
  const _getObjectName = (type) => {
    return type.toLowerCase();
  };
  
  const _getViewportCenterPosition = async (scene, distance = 5) => {
    if (!scene || !scene._camera) {
      console.log('No scene or camera, using fallback position');
      return new Vector3(0, 1, 0);
    }

    const camera = scene._camera;
    
    try {
      const forward = camera.getDirection(Vector3.Forward()).normalize();
      const centerPosition = camera.position.add(forward.scale(distance));
      
      const ray = new Ray(centerPosition.add(Vector3.Up().scale(100)), Vector3.Down());
      const hit = scene.pickWithRay(ray, (mesh) => {
        if (!mesh || mesh.name === 'ground' || mesh.name === 'skybox') {
          return false;
        }
        return mesh.isPickable !== false && mesh.isVisible && mesh.material;
      });
      
      let finalY = 1;
      
      if (hit.hit && hit.pickedPoint) {
        finalY = hit.pickedPoint.y + 0.5;
        console.log('Hit ground at Y:', hit.pickedPoint.y, 'placing object at Y:', finalY);
      } else {
        console.log('No ground hit, using default Y:', finalY);
      }
      
      const finalPosition = new Vector3(centerPosition.x, finalY, centerPosition.z);
      console.log('Final object position:', finalPosition);
      
      return finalPosition;
    } catch (error) {
      console.error('Error calculating viewport center:', error);
      return new Vector3(0, 0.5, 0);
    }
  };

  // Using unified object creation utilities

  const createBabylonPrimitive = async (type) => {
    console.log('Creating primitive:', type);
    
    const scene = getCurrentScene();
    if (!scene) {
      editorActions.addConsoleMessage('No active scene available', 'error');
      return;
    }

    try {
      // Use unified creation system for consistent sizes and colors
      createAndAddObject(type, scene);
      editorActions.addConsoleMessage(`Created ${type}`, 'info');
    } catch (error) {
      console.error('Failed to create primitive:', error);
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
      // Use unified creation system for consistent behavior
      createAndAddObject(`${lightType}-light`, scene);
      editorActions.addConsoleMessage(`Created ${lightType} light`, 'info');
    } catch (error) {
      console.error('Failed to create light:', error);
      editorActions.addConsoleMessage(`Failed to create light: ${error.message}`, 'error');
    }
  };

  const createBabylonCamera = async () => {
    const scene = getCurrentScene();
    if (!scene) {
      editorActions.addConsoleMessage('No active scene available', 'error');
      return;
    }

    try {
      // Use unified creation system for consistent behavior
      createAndAddObject('camera', scene);
      editorActions.addConsoleMessage('Created camera', 'info');
    } catch (error) {
      console.error('Failed to create camera:', error);
      editorActions.addConsoleMessage(`Failed to create camera: ${error.message}`, 'error');
    }
  };

  const duplicateSelectedObject = async () => {
    const selectedObject = renderStore.selectedObject;
    if (!selectedObject) {
      editorActions.addConsoleMessage('No object selected to duplicate', 'warning');
      return;
    }
    
    try {
      let newObject = selectedObject.clone(selectedObject.name + '_duplicate', null, false, true);
      
      if (newObject) {
        newObject.position.x += 1;
        newObject.position.z += 1;
        
        // Add object to hierarchy (with folder awareness) first, then select it
        addObjectToHierarchy(newObject, `${selectedObject.name}_duplicate`);
        renderActions.selectObject(newObject);
        editorActions.addConsoleMessage(`Duplicated ${selectedObject.name}`, 'info');
      }
    } catch (error) {
      console.error('Failed to duplicate object:', error);
      editorActions.addConsoleMessage(`Failed to duplicate object: ${error.message}`, 'error');
    }
  };

  const deleteSelectedObject = () => {
    const selectedObject = renderStore.selectedObject;
    if (!selectedObject) {
      editorActions.addConsoleMessage('No object selected to delete', 'warning');
      return;
    }
    
    if (selectedObject.name === 'ground' || selectedObject.name === 'skybox') {
      editorActions.addConsoleMessage('Cannot delete default scene objects', 'warning');
      return;
    }
    
    try {
      selectedObject.dispose();
      
      renderActions.selectObject(null);
      editorActions.addConsoleMessage(`Deleted ${selectedObject.name}`, 'info');
    } catch (error) {
      console.error('Failed to delete object:', error);
      editorActions.addConsoleMessage(`Failed to delete object: ${error.message}`, 'error');
    }
  };

  // Camera view functions
  const setCameraView = (viewType) => {
    const scene = getCurrentScene();
    if (!scene) {
      editorActions.addConsoleMessage('No active scene available', 'error');
      console.error('No active scene found');
      return;
    }

    // Try to get camera from multiple sources
    const camera = scene.activeCamera || scene._camera || (scene.cameras && scene.cameras[0]);
    if (!camera) {
      editorActions.addConsoleMessage('No active camera available', 'error');
      console.error('No camera found in scene');
      return;
    }

    console.log(`Setting camera view to ${viewType}`, { camera });
    
    // Calculate current focus point (where camera is looking) - Blender style
    let focusPoint = new Vector3(0, 0, 0);
    let currentDistance = 15; // Default distance
    
    if (camera.getTarget && typeof camera.getTarget === 'function') {
      // Camera has a target (like ArcRotateCamera)
      focusPoint = camera.getTarget();
      currentDistance = Vector3.Distance(camera.position, focusPoint);
    } else {
      // For Universal/Free cameras, calculate where they're looking
      const forward = camera.getDirection ? 
        camera.getDirection(Vector3.Forward()) : 
        camera.getForwardRay().direction;
      
      // Use a reasonable distance based on current position or use selected object
      const selectedObject = renderStore.selectedObject;
      if (selectedObject && selectedObject.position) {
        focusPoint = selectedObject.position.clone();
        currentDistance = Vector3.Distance(camera.position, focusPoint);
      } else {
        // Project forward from camera to find focus point
        currentDistance = Math.max(10, Vector3.Distance(camera.position, Vector3.Zero()));
        focusPoint = camera.position.add(forward.normalize().scale(currentDistance));
      }
    }

    console.log(`Focus point: ${focusPoint}, Distance: ${currentDistance}`);

    // Define camera positions relative to focus point (Blender-style)
    // Maintain the current distance from focus point
    const positions = {
      // Front view - camera looks down negative Z axis (Blender standard)
      front: new Vector3(focusPoint.x, focusPoint.y, focusPoint.z + currentDistance),
      // Back view - camera looks down positive Z axis  
      back: new Vector3(focusPoint.x, focusPoint.y, focusPoint.z - currentDistance),
      // Right view - camera looks down negative X axis (from object's right side)
      right: new Vector3(focusPoint.x + currentDistance, focusPoint.y, focusPoint.z),
      // Left view - camera looks down positive X axis (from object's left side)
      left: new Vector3(focusPoint.x - currentDistance, focusPoint.y, focusPoint.z),
      // Top view - camera looks down negative Y axis (from above)
      top: new Vector3(focusPoint.x, focusPoint.y + currentDistance, focusPoint.z),
      // Bottom view - camera looks down positive Y axis (from below)  
      bottom: new Vector3(focusPoint.x, focusPoint.y - currentDistance, focusPoint.z),
      // Isometric views (Blender numpad 1+3, 7+1, etc.)
      frontRight: new Vector3(focusPoint.x + currentDistance * 0.7, focusPoint.y + currentDistance * 0.5, focusPoint.z + currentDistance * 0.7),
      frontLeft: new Vector3(focusPoint.x - currentDistance * 0.7, focusPoint.y + currentDistance * 0.5, focusPoint.z + currentDistance * 0.7)
    };

    if (positions[viewType]) {
      const newPosition = positions[viewType];
      console.log(`Moving camera from ${camera.position} to ${newPosition}, focus: ${focusPoint}`);
      
      // Temporarily disable camera movement controller if available
      const canvas = scene.getEngine()?.getRenderingCanvas();
      const cameraController = canvas?._cameraMovementController;
      if (cameraController) {
        cameraController.disable();
      }
      
      // Set camera position directly
      camera.position.copyFrom(newPosition);
      
      // Set camera target (look at focus point) 
      if (camera.setTarget && typeof camera.setTarget === 'function') {
        camera.setTarget(focusPoint);
      } else {
        // For cameras that don't have setTarget, manually calculate rotation
        const direction = focusPoint.subtract(newPosition).normalize();
        const yaw = Math.atan2(direction.x, direction.z);
        const pitch = Math.asin(-direction.y);
        camera.rotation.copyFrom(new Vector3(pitch, yaw, 0));
      }
      
      // Re-enable camera movement controller after a brief delay
      if (cameraController) {
        setTimeout(() => {
          cameraController.enable();
        }, 100);
      }
      
      setCameraViewDropdownOpen(false);
      const viewName = getViewDisplayName(viewType);
      setCurrentViewName(viewName);
      
      // Expose current view name globally for camera helper
      window._currentCameraViewName = viewName;
      
      editorActions.addConsoleMessage(`Camera view set to ${viewType} (focus: ${focusPoint.x.toFixed(1)}, ${focusPoint.y.toFixed(1)}, ${focusPoint.z.toFixed(1)})`, 'info');
      console.log(`Camera position set to: ${camera.position}, rotation: ${camera.rotation}`);
    } else {
      console.error(`Unknown camera view type: ${viewType}`);
    }
  };

  // Keyboard shortcuts for camera views
  const handleKeyDown = (event) => {
    // Only handle shortcuts when not in input fields
    if (event.target.tagName === 'INPUT' || event.target.tagName === 'TEXTAREA') {
      return;
    }

    // Numpad shortcuts for camera views (Blender-style)
    switch (event.code) {
      case 'Numpad1':
        event.preventDefault();
        if (event.ctrlKey) {
          setCameraView('back');
        } else {
          setCameraView('front');
        }
        break;
      case 'Numpad3':
        event.preventDefault();
        if (event.ctrlKey) {
          setCameraView('left');
        } else {
          setCameraView('right');
        }
        break;
      case 'Numpad7':
        event.preventDefault();
        if (event.ctrlKey) {
          setCameraView('bottom');
        } else {
          setCameraView('top');
        }
        break;
      case 'Numpad8':
        event.preventDefault();
        setCameraView('frontLeft');
        break;
      case 'Numpad6':
        event.preventDefault();
        setCameraView('frontRight');
        break;
    }
  };

  // Add global keyboard listener
  window.addEventListener('keydown', handleKeyDown);

  // Close dropdown when clicking outside
  const handleClickOutside = (event) => {
    if (!event.target.closest('[data-camera-dropdown]')) {
      setCameraViewDropdownOpen(false);
    }
    if (!event.target.closest('[data-mode-dropdown]')) {
      setModeDropdownOpen(false);
    }
  };

  window.addEventListener('click', handleClickOutside);

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
      // Toggle light dropdown instead of creating light directly
      setLightDropdownOpen(!lightDropdownOpen());
      // Close other dropdowns
      setCameraViewDropdownOpen(false);
      setModeDropdownOpen(false);
      return; // Don't deselect current tool
    }
    else if (toolId === 'camera') {
      await createBabylonCamera();
    }
    else if (toolId === 'duplicate') {
      if (!selectedEntity()) {
        editorActions.addConsoleMessage('Please select an object first', 'warning');
        return;
      }
      await duplicateSelectedObject();
    }
    else if (toolId === 'delete') {
      if (!selectedEntity()) {
        editorActions.addConsoleMessage('Please select an object first', 'warning');
        return;
      }
      deleteSelectedObject();
    }
    else if (toolId === 'play_pause') {
      editorActions.toggleScriptExecution();
      const newState = editorStore.scripts.isPlaying;
      editorActions.addConsoleMessage(
        newState ? 'Script execution started' : 'Script execution paused', 
        'info'
      );
    }
    else if (['terrain_raise', 'terrain_lower', 'terrain_smooth', 'terrain_flatten', 'terrain_paint', 'terrain_noise'].includes(toolId)) {
      if (!selectedEntity() || !selectedEntity()._terrainData) {
        editorActions.addConsoleMessage('Please select a terrain object first', 'warning');
        return;
      }
      setSelectedTool(toolId);
      
      // Start terrain editing mode with the selected tool
      const toolName = toolId.replace('terrain_', '');
      document.dispatchEvent(new CustomEvent('engine:start-terrain-edit', { 
        detail: { tool: toolName } 
      }));
      
      editorActions.addConsoleMessage(`Terrain tool activated: ${toolName}`, 'info');
    }
    else {
      editorActions.addConsoleMessage(`Tool activated: ${toolId}`, 'info');
    }
  };

  // Check if terrain object is selected to show terrain-specific tools
  const isTerrainSelected = () => {
    const _entity = selectedEntity();
    const babylonObject = renderStore.selectedObject; // Get the actual Babylon.js object
    return babylonObject && babylonObject._terrainData;
  };

  // Make tools reactive to selection changes
  const tools = () => {
    if (isTerrainSelected()) {
      // Terrain-specific toolbar
      return [
        { id: 'select', icon: IconPointer, tooltip: 'Select' },
        { id: 'move', icon: IconArrowsMove, tooltip: 'Move' },
        { id: 'rotate', icon: IconRefresh, tooltip: 'Rotate' },
        { id: 'scale', icon: IconMaximize, tooltip: 'Scale' },
        null, // Separator
        { id: 'terrain_raise', icon: IconMountain, tooltip: 'Raise Terrain (Ctrl+Scroll to resize brush)' },
        { id: 'terrain_lower', icon: IconSquare, tooltip: 'Lower Terrain (Ctrl+Scroll to resize brush)' },
        { id: 'terrain_smooth', icon: IconCircle, tooltip: 'Smooth Terrain (Ctrl+Scroll to resize brush)' },
        { id: 'terrain_flatten', icon: IconBrush, tooltip: 'Flatten Terrain (Ctrl+Scroll to resize brush)' },
        { id: 'terrain_paint', icon: IconSun, tooltip: 'Paint Texture (Ctrl+Scroll to resize brush)' },
        { id: 'terrain_noise', icon: IconBulb, tooltip: 'Add Noise (Ctrl+Scroll to resize brush)' },
        null, // Separator
        { id: 'duplicate', icon: IconCopy, tooltip: 'Duplicate' },
        { id: 'delete', icon: IconTrash, tooltip: 'Delete' }
      ];
    } else {
      // Standard toolbar
      return [
        { id: 'select', icon: IconPointer, tooltip: 'Select' },
        { id: 'move', icon: IconArrowsMove, tooltip: 'Move' },
        { id: 'rotate', icon: IconRefresh, tooltip: 'Rotate' },
        { id: 'scale', icon: IconMaximize, tooltip: 'Scale' },
        null, // Separator
        { id: 'camera', icon: IconVideo, tooltip: 'Add Camera' },
        { id: 'cube', icon: IconBox, tooltip: 'Add Cube' },
        { id: 'sphere', icon: IconCircle, tooltip: 'Add Sphere' },
        { id: 'cylinder', icon: IconCylinder, tooltip: 'Add Cylinder' },
        { id: 'plane', icon: IconSquare, tooltip: 'Add Plane' },
        { id: 'light', icon: IconBulb, tooltip: 'Add Light' },
        null, // Separator
        { id: 'duplicate', icon: IconCopy, tooltip: 'Duplicate' },
        { id: 'delete', icon: IconTrash, tooltip: 'Delete' }
      ];
    }
  };

  const setMode = (mode) => {
    editorActions.setCurrentMode(mode);
    setModeDropdownOpen(false);
    editorActions.addConsoleMessage(`Switched to ${getModeDisplayData(mode).name} mode`, 'info');
  };

  const createLightOfType = async (lightType) => {
    await createBabylonLight(lightType);
    setLightDropdownOpen(false);
  };

  return (
    <div class="w-full h-10 flex items-center bg-base-200 border-b border-base-300 px-2 gap-1">
      <For each={tools()}>
        {(tool) => 
          tool === null ? (
            <div class="w-px h-6 bg-base-content/20 mx-1"></div>
          ) : tool.id === 'light' ? (
            // Special handling for light dropdown button
            <div class="relative">
              <button 
                onClick={() => handleToolbarClick(tool.id)}
                class={`h-8 px-2 flex items-center justify-center gap-1 rounded transition-all group cursor-pointer ${
                  lightDropdownOpen()
                    ? 'bg-primary text-primary-content'
                    : 'text-base-content/60 hover:text-base-content hover:bg-base-300'
                }`} 
                title={tool.tooltip}
              >
                <tool.icon class="w-4 h-4" />
                <IconChevronDown class="w-2 h-2" />
                
                <div class="absolute top-full mt-2 bg-base-200 text-base-content text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                  {tool.tooltip}
                </div>
              </button>
              
              {/* Light Type Dropdown */}
              <Show when={lightDropdownOpen()}>
                <div class="absolute left-0 top-full mt-1 bg-base-200 border border-base-300 rounded shadow-lg z-50 min-w-64">
                  <div class="p-2 space-y-1">
                    <div class="text-xs text-base-content/60 px-2 py-1 font-medium">Light Types</div>
                    
                    <For each={lightTypes}>
                      {(lightType) => (
                        <button
                          onClick={() => createLightOfType(lightType.id)}
                          class="w-full flex items-center gap-3 px-3 py-2 text-xs rounded cursor-pointer hover:bg-base-300 text-left"
                        >
                          <lightType.icon class="w-4 h-4" />
                          <div class="flex-1">
                            <div class="font-medium">{lightType.name}</div>
                            <div class="text-xs text-base-content/60">{lightType.description}</div>
                          </div>
                        </button>
                      )}
                    </For>
                  </div>
                </div>
              </Show>
            </div>
          ) : (
            <button 
              onClick={() => handleToolbarClick(tool.id)}
              class={`w-8 h-8 flex items-center justify-center rounded transition-all group cursor-pointer ${
                getSelectedTool() === tool.id
                  ? 'bg-primary text-primary-content'
                  : 'text-base-content/60 hover:text-base-content hover:bg-base-300'
              }`} 
              title={tool.tooltip}
            >
              <tool.icon class="w-4 h-4" />
              
              <div class="absolute top-full mt-2 bg-base-200 text-base-content text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                {tool.tooltip}
              </div>
            </button>
          )
        }
      </For>
      
      {/* Sculpting Mode Controls */}
      {currentMode() === 'sculpting' && (
        <>
          <div class="w-px h-6 bg-base-content/20 mx-1"></div>
          
          {/* Brush Size Control */}
          <div class="flex items-center gap-2 px-2">
            <label class="text-xs text-base-content/60 whitespace-nowrap">Size:</label>
            <input
              type="range"
              min="1"
              max="32"
              step="0.5"
              value={getBrushSize()}
              onInput={(e) => setBrushSize(parseFloat(e.target.value))}
              class="range range-primary range-xs w-16"
              title={`Brush Size: ${getBrushSize()}`}
            />
            <span class="text-xs text-base-content/60 min-w-[2rem] text-center">{getBrushSize()}</span>
          </div>
          
          {/* Brush Strength Control */}
          <div class="flex items-center gap-2 px-2">
            <label class="text-xs text-base-content/60 whitespace-nowrap">Strength:</label>
            <input
              type="range"
              min="0.01"
              max="1.0"
              step="0.01"
              value={getBrushStrength()}
              onInput={(e) => setBrushStrength(parseFloat(e.target.value))}
              class="range range-primary range-xs w-16"
              title={`Brush Strength: ${(getBrushStrength() * 100).toFixed(0)}%`}
            />
            <span class="text-xs text-base-content/60 min-w-[2rem] text-center">{(getBrushStrength() * 100).toFixed(0)}%</span>
          </div>
        </>
      )}
      
      <div class="flex-1" />
      
      {/* Play/Pause Button */}
      <button 
        onClick={() => handleToolbarClick('play_pause')}
        class="h-8 w-8 flex items-center justify-center rounded transition-all group cursor-pointer text-base-content/60 hover:text-base-content hover:bg-base-300" 
        title={editorStore.scripts.isPlaying ? 'Pause Scripts' : 'Play Scripts'}
      >
        {editorStore.scripts.isPlaying ? 
          <IconPlayerPause class={`w-4 h-4 text-yellow-500`} /> : 
          <IconPlayerPlay class={`w-4 h-4 text-green-500`} />
        }
        
        <div class="absolute top-full mt-2 bg-base-200 text-base-content text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
          {editorStore.scripts.isPlaying ? 'Pause Scripts' : 'Play Scripts'}
        </div>
      </button>
      
      {/* Mode Dropdown */}
      <div class="relative" data-mode-dropdown>
        <button
          onClick={() => {
            setModeDropdownOpen(!modeDropdownOpen());
            // Close other dropdowns when mode dropdown opens
            setCameraViewDropdownOpen(false);
            setLightDropdownOpen(false);
          }}
          class="h-8 px-3 flex items-center gap-2 rounded cursor-pointer text-base-content/60 hover:text-base-content hover:bg-base-300 text-xs transition-all"
          title={`Current mode: ${getModeDisplayData(currentMode()).description}`}
        >
          {(() => {
            const ModeIcon = getModeDisplayData(currentMode()).icon;
            return <ModeIcon class="w-3 h-3" />;
          })()}
          <span>{getModeDisplayData(currentMode()).name}</span>
          <IconChevronDown class="w-3 h-3" />
        </button>
        
        <Show when={modeDropdownOpen()}>
          <div class="absolute right-0 top-full mt-1 bg-base-200 border border-base-300 rounded shadow-lg z-50 min-w-64">
            <div class="p-2 space-y-1">
              <div class="text-xs text-base-content/60 px-2 py-1 font-medium">Editor Modes</div>
              
              <button
                onClick={() => setMode('standard')}
                class={`w-full flex items-center gap-3 px-3 py-2 text-xs rounded cursor-pointer hover:bg-base-300 text-left ${
                  currentMode() === 'standard' ? 'bg-base-300 text-base-content' : ''
                }`}
              >
                <IconCube class="w-4 h-4" />
                <div class="flex-1">
                  <div class="font-medium">Scene Editor</div>
                  <div class="text-xs text-base-content/60">Design scenes, add models, and transform objects</div>
                </div>
              </button>
              
              <button
                onClick={() => setMode('sculpting')}
                class={`w-full flex items-center gap-3 px-3 py-2 text-xs rounded cursor-pointer hover:bg-base-300 text-left ${
                  currentMode() === 'sculpting' ? 'bg-base-300 text-base-content' : ''
                }`}
              >
                <IconBrush class="w-4 h-4" />
                <div class="flex-1">
                  <div class="font-medium">Sculpting</div>
                  <div class="text-xs text-base-content/60">Terrain and mesh sculpting</div>
                </div>
              </button>
              
            </div>
          </div>
        </Show>
      </div>
      
      {/* Camera View Dropdown */}
      <div class="relative" data-camera-dropdown>
        <button
          onClick={() => {
            setCameraViewDropdownOpen(!cameraViewDropdownOpen());
            // Close other dropdowns when camera dropdown opens
            setModeDropdownOpen(false);
            setLightDropdownOpen(false);
            if (window._closeHelperDropdowns) {
              window._closeHelperDropdowns();
            }
          }}
          class="h-8 px-3 flex items-center gap-1 rounded cursor-pointer text-base-content/60 hover:text-base-content hover:bg-base-100/50 text-xs transition-all"
          title="Camera Views (Numpad shortcuts)"
        >
          <IconVideo class="w-3 h-3" />
          <span>Camera</span>
          <IconChevronDown class="w-3 h-3" />
        </button>
        
        <Show when={cameraViewDropdownOpen()}>
          <div class="absolute right-0 top-full mt-1 bg-base-200 border border-base-300 rounded shadow-lg z-50 min-w-48">
            <div class="p-2 space-y-1">
              <div class="text-xs text-base-content/60 px-2 py-1 font-medium">Camera Views</div>
              
              <button
                onClick={() => setCameraView('front')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded cursor-pointer hover:bg-base-300 text-left"
              >
                <span>Front</span>
                <span class="text-base-content/60">1</span>
              </button>
              
              <button
                onClick={() => setCameraView('back')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded cursor-pointer hover:bg-base-300 text-left"
              >
                <span>Back</span>
                <span class="text-base-content/60">Ctrl+1</span>
              </button>
              
              <button
                onClick={() => setCameraView('right')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded cursor-pointer hover:bg-base-300 text-left"
              >
                <span>Right</span>
                <span class="text-base-content/60">3</span>
              </button>
              
              <button
                onClick={() => setCameraView('left')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded cursor-pointer hover:bg-base-300 text-left"
              >
                <span>Left</span>
                <span class="text-base-content/60">Ctrl+3</span>
              </button>
              
              <button
                onClick={() => setCameraView('top')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded cursor-pointer hover:bg-base-300 text-left"
              >
                <span>Top</span>
                <span class="text-base-content/60">7</span>
              </button>
              
              <button
                onClick={() => setCameraView('bottom')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded cursor-pointer hover:bg-base-300 text-left"
              >
                <span>Bottom</span>
                <span class="text-base-content/60">Ctrl+7</span>
              </button>
              
              <div class="w-full h-px bg-base-300 my-1"></div>
              
              <button
                onClick={() => setCameraView('frontLeft')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded cursor-pointer hover:bg-base-300 text-left"
              >
                <span>Front Left</span>
                <span class="text-base-content/60">8</span>
              </button>
              
              <button
                onClick={() => setCameraView('frontRight')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded cursor-pointer hover:bg-base-300 text-left"
              >
                <span>Front Right</span>
                <span class="text-base-content/60">6</span>
              </button>
            </div>
          </div>
        </Show>
      </div>
      
      <Show when={helperVisible()}>
        <Helper onHelperClick={() => setCameraViewDropdownOpen(false)} />
      </Show>
    </div>
  );
}

export default Toolbar;