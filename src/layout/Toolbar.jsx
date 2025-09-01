import { createSignal, createEffect, onCleanup, For, Show } from 'solid-js';
import { horizontalMenuButtonsEnabled } from '@/api/plugin';
import { Sun, Lightbulb, Pointer, Move, Refresh, Maximize, Video, Copy, Trash, Box, Circle, Rectangle } from '@/ui/icons';
import { Play, Pause } from '@/ui/icons/media';
import { Settings } from '@/ui/icons/development';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { toolbarButtons } from '@/api/plugin';
import ThemeSwitcher from '@/ui/ThemeSwitcher';
import { getScriptRuntime } from '@/api/script';
import { viewportStore, viewportActions, objectPropertiesActions } from '@/layout/stores/ViewportStore';
import { renderStore, renderActions } from '@/render/store.jsx';
import GridHelpers from '@/ui/display/GridHelpers.jsx';
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

function Toolbar() {
  const [showProjectManager, setShowProjectManager] = createSignal(false);
  const [flashingTool, setFlashingTool] = createSignal(null);
  const [showLightDropdown, setShowLightDropdown] = createSignal(false);
  const [lightDropdownPosition, setLightDropdownPosition] = createSignal(null);
  const [activePluginDropdown, setActivePluginDropdown] = createSignal(null);
  const [pluginDropdownPosition, setPluginDropdownPosition] = createSignal(null);
  const [scriptRuntimePlaying, setScriptRuntimePlaying] = createSignal(true);
  
  createEffect(() => {
    const handleClickOutside = (event) => {
      const target = event.target;
      const isToolbarButton = target.closest('.toolbar-button');
      const isDropdownContent = target.closest('.dropdown-content');
      
      if (!isToolbarButton && !isDropdownContent) {
        setActivePluginDropdown(null);
        setPluginDropdownPosition(null);
        setShowLightDropdown(false);
        setLightDropdownPosition(null);
      }
    };

    if (activePluginDropdown() || showLightDropdown()) {
      document.addEventListener('mousedown', handleClickOutside);
      onCleanup(() => {
        document.removeEventListener('mousedown', handleClickOutside);
      });
    }
  });

  // Sync script runtime state
  createEffect(() => {
    const interval = setInterval(() => {
      try {
        const runtime = getScriptRuntime();
        const stats = runtime.getStats();
        setScriptRuntimePlaying(stats.running);
      } catch (error) {
        // Runtime might not be initialized yet
        setScriptRuntimePlaying(false);
      }
    }, 1000);

    onCleanup(() => clearInterval(interval));
  });
  
  const selectedTool = () => editorStore.ui.selectedTool;
  const selection = () => editorStore.selection;
  const selectedEntity = () => selection().entity;
  const transformMode = () => selection().transformMode;
  const renderMode = () => viewportStore.renderMode || 'solid';
  
  const { setSelectedTool, setTransformMode, selectEntity } = editorActions;
  const { setRenderMode, setShowGrid } = viewportActions;
  
  const getCurrentWorkflow = () => {
    const currentViewport = viewportStore;
    if (!currentViewport.tabs || currentViewport.tabs.length === 0) {
      return '3d-viewport';
    }
    const activeTabData = currentViewport.tabs.find(tab => tab.id === currentViewport.activeTabId);
    return activeTabData?.type || '3d-viewport';
  };

  const lightTypes = [
    { id: 'directional', label: 'Directional Light', icon: Sun },
    { id: 'point', label: 'Point Light', icon: Lightbulb },
    { id: 'spot', label: 'Spot Light', icon: Sun },
    { id: 'hemisphere', label: 'Hemisphere Light', icon: Lightbulb }
  ];
  
  const workflowTools = {
    '3d-viewport': [
      { id: 'select', icon: Pointer, tooltip: 'Select' },
      { id: 'move', icon: Move, tooltip: 'Move', requiresSelection: true },
      { id: 'rotate', icon: Refresh, tooltip: 'Rotate', requiresSelection: true },
      { id: 'scale', icon: Maximize, tooltip: 'Scale', requiresSelection: true },
      { id: 'cube', icon: Box, tooltip: 'Add Cube' },
      { id: 'sphere', icon: Circle, tooltip: 'Add Sphere' },
      { id: 'cylinder', icon: Box, tooltip: 'Add Cylinder' },
      { id: 'plane', icon: Rectangle, tooltip: 'Add Plane' },
      { id: 'light', icon: Sun, tooltip: 'Add Light', isDropdown: true },
      { id: 'camera', icon: Video, tooltip: 'Add Camera' },
      { id: 'duplicate', icon: Copy, tooltip: 'Duplicate', requiresSelection: true },
      { id: 'delete', icon: Trash, tooltip: 'Delete', requiresSelection: true },
    ]
  };
  
  const currentWorkflow = () => getCurrentWorkflow();
  const tools = () => workflowTools[currentWorkflow()] || workflowTools['3d-viewport'];

  const getEffectiveSelectedTool = () => {
    if (['select', 'move', 'rotate', 'scale'].includes(transformMode())) {
      return transformMode();
    }
    return selectedTool();
  };

  const handleToolClick = async (toolId) => {
    console.log('HorizontalToolbar: Tool clicked:', toolId);
    
    const pluginButton = toolbarButtons().get(toolId);
    if (pluginButton && pluginButton.onClick) {
      pluginButton.onClick();
      return;
    }
    
    editorActions.addConsoleMessage(`Clicked ${toolId} button`, 'info');
    
    if (['select', 'move', 'rotate', 'scale'].includes(toolId)) {
      if (toolId !== 'select' && !selectedEntity()) {
        editorActions.addConsoleMessage('Please select an object first', 'warning');
        return;
      }
      
      setTransformMode(toolId);
      
      // Use render store for gizmo management
      renderActions.setTransformMode(toolId);
      
      editorActions.addConsoleMessage(`Switched to ${toolId} tool`, 'info');
    }
    else if (['cube', 'sphere', 'cylinder', 'plane'].includes(toolId)) {
      await createBabylonPrimitive(toolId);
    }
    else if (toolId === 'light') {
      return;
    }
    else if (toolId === 'camera') {
      await createBabylonCamera();
    }
    else if (toolId === 'duplicate') {
      if (!selectedEntity()) {
        editorActions.addConsoleMessage('Please select an object to duplicate', 'warning');
        return;
      }
      await duplicateSelectedObject();
    }
    else if (toolId === 'delete') {
      if (!selectedEntity()) {
        editorActions.addConsoleMessage('Please select an object to delete', 'warning');
        return;
      }
      deleteSelectedObject();
    }
    else {
      editorActions.addConsoleMessage(`Tool activated: ${toolId}`, 'info');
    }
  };

  const handleLightCreate = async (lightType) => {
    await createBabylonLight(lightType);
    setShowLightDropdown(false);
    setLightDropdownPosition(null);
  };

  const handleScriptPlayToggle = () => {
    const runtime = getScriptRuntime();
    const stats = runtime.getStats();
    
    if (stats.running) {
      runtime.pause();
      setScriptRuntimePlaying(false);
      editorActions.addConsoleMessage('Script execution paused', 'info');
      console.log('🔧 Script execution paused');
    } else {
      runtime.start();
      setScriptRuntimePlaying(true);
      editorActions.addConsoleMessage('Script execution resumed', 'success');
      console.log('🔧 Script execution resumed');
    }
  };

  const handleLightDropdownToggle = (e) => {
    if (showLightDropdown()) {
      setShowLightDropdown(false);
      setLightDropdownPosition(null);
    } else {
      setActivePluginDropdown(null);
      setPluginDropdownPosition(null);
      
      const rect = e.currentTarget.getBoundingClientRect();
      const position = calculateDropdownPosition(rect, 192);
      setLightDropdownPosition(position);
      setShowLightDropdown(true);
    }
  };

  const calculateDropdownPosition = (buttonRect, dropdownWidth = 192) => {
    const viewportWidth = window.innerWidth;
    const margin = 8;
    let left = buttonRect.left + (buttonRect.width / 2) - (dropdownWidth / 2);
    
    if (left + dropdownWidth + margin > viewportWidth) {
      left = viewportWidth - dropdownWidth - margin;
    }
    
    if (left < margin) {
      left = margin;
    }
    
    return {
      left,
      top: buttonRect.bottom + 4
    };
  };


  const handlePluginDropdownToggle = (e, button) => {
    if (activePluginDropdown() === button.id) {
      setActivePluginDropdown(null);
      setPluginDropdownPosition(null);
    } else {
      setShowLightDropdown(false);
      setLightDropdownPosition(null);
      
      const rect = e.currentTarget.getBoundingClientRect();
      const dropdownWidth = button.dropdownWidth || 192;
      const position = calculateDropdownPosition(rect, dropdownWidth);
      setPluginDropdownPosition(position);
      setActivePluginDropdown(button.id);
    }
  };

  const getCurrentScene = () => {
    return renderStore.scene;
  };

  const getObjectName = (type) => {
    return type.toLowerCase();
  };

  const getViewportCenterPosition = async (scene, distance = 5) => {
    if (!scene || !scene._camera) {
      console.log('No scene or camera, using fallback position');
      return new Vector3(0, 1, 0);
    }

    const camera = scene._camera;
    
    try {
      const forward = camera.getForwardRay().direction.normalize();
      const viewportCenter = camera.position.add(forward.scale(distance));
      
      // Cast ray downward from high position to find ground
      const rayOrigin = new Vector3(viewportCenter.x, 100, viewportCenter.z);
      const rayDirection = new Vector3(0, -1, 0);
      const ray = new Ray(rayOrigin, rayDirection);
      
      // Look for ground or any solid surface
      const hit = scene.pickWithRay(ray, (mesh) => {
        // Skip gizmos and helper objects
        if (mesh._isInternalMesh || mesh.name.includes('gizmo') || mesh.name.includes('helper')) {
          return false;
        }
        return mesh.isPickable !== false && mesh.isVisible && mesh.material;
      });
      
      let finalY = 1; // Default height above ground
      
      if (hit.hit && hit.pickedPoint) {
        finalY = hit.pickedPoint.y + 1; // Place 1 unit above the surface
        console.log('Found surface at Y:', hit.pickedPoint.y, 'placing object at Y:', finalY);
      } else {
        // No surface found, place at ground level (Y = 0.5 so bottom of cube touches Y = 0)
        finalY = 0.5;
        console.log('No surface found, placing at ground level Y:', finalY);
      }
      
      const finalPosition = new Vector3(viewportCenter.x, finalY, viewportCenter.z);
      
      console.log('Calculated position:', finalPosition, 'from viewport center at distance:', distance);
      return finalPosition;
    } catch (error) {
      console.error('Error calculating viewport center:', error);
      return new Vector3(0, 0.5, 0);
    }
  };

  const createBabylonPrimitive = async (type) => {
    console.log('Creating primitive:', type);
    const scene = getCurrentScene();
    console.log('Current scene:', scene);
    
    if (!scene) {
      console.error('No scene available');
      editorActions.addConsoleMessage('No active scene available', 'error');
      return;
    }

    // For now, place objects at origin to debug the positioning issue
    const position = new Vector3(0, 0.5, 0);
    const objectName = getObjectName(type);
    
    console.log('Creating', type, 'at position:', position);
    
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
          mesh.rotation.x = Math.PI / 2; // Rotate 90 degrees to lay flat
          break;
      }
      
      if (mesh) {
        mesh.parent = mainContainer;
        mesh.position = Vector3.Zero();
        const material = new StandardMaterial(`${objectName}_material`, scene);
        material.diffuseColor = new Color3(0.7, 0.7, 0.9);
        material.specularColor = new Color3(0.2, 0.2, 0.2);
        mesh.material = material;
        
        if (scene._applyRenderMode) {
          const currentRenderMode = renderMode();
          if (currentRenderMode === 'wireframe') {
            material.wireframe = true;
          }
        }
        
        // Add to scene tree and select
        renderActions.addObject(mainContainer);
        renderActions.selectObject(mainContainer);
        renderActions.setTransformMode('move'); // Set to move mode when creating objects
        
        const objectId = mainContainer.uniqueId || mainContainer.name;
        
        // Initialize object properties with transform data
        objectPropertiesActions.ensureDefaultComponents(objectId);
        objectPropertiesActions.updateObjectProperty(objectId, 'transform.position', [position.x, position.y, position.z]);
        objectPropertiesActions.updateObjectProperty(objectId, 'transform.rotation', [0, 0, 0]);
        // Set proper scale for planes (thin Y-axis for physics)
        const scaleValue = type === 'plane' ? [1, 0.01, 1] : [1, 1, 1];
        objectPropertiesActions.updateObjectProperty(objectId, 'transform.scale', scaleValue);
        
        editorActions.selectEntity(objectId);
        setTransformMode('move'); // Set to move mode when creating objects
        
        console.log('Primitive created successfully:', mainContainer.name, 'ID:', mainContainer.uniqueId);
        editorActions.addConsoleMessage(`Created ${type}`, 'success');
      }
    } catch (error) {
      console.error(`Error creating ${type}:`, error);
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
      const lightPosition = await getViewportCenterPosition(scene, 4);
      lightPosition.y += 3;
      
      const mainContainer = new TransformNode(lightName, scene);
      mainContainer.position = lightPosition;
      
      let light;
      switch (lightType) {
        case 'point':
          light = new PointLight(`${lightName}_light`, Vector3.Zero(), scene);
          light.diffuse = new Color3(1, 0.95, 0.8);
          light.specular = new Color3(1, 1, 1);
          light.intensity = 10;
          break;
        case 'spot':
          light = new SpotLight(`${lightName}_light`, Vector3.Zero(), new Vector3(0, -1, 0), Math.PI / 3, 2, scene);
          light.diffuse = new Color3(1, 0.95, 0.8);
          light.specular = new Color3(1, 1, 1);
          light.intensity = 15;
          break;
        case 'hemisphere':
          light = new HemisphericLight(`${lightName}_light`, new Vector3(0, 1, 0), scene);
          light.diffuse = new Color3(1, 0.95, 0.8);
          light.groundColor = new Color3(0.3, 0.3, 0.3);
          light.intensity = 0.7;
          break;
        default:
          light = new DirectionalLight(`${lightName}_light`, new Vector3(-1, -1, -1), scene);
          light.diffuse = new Color3(1, 0.95, 0.8);
          light.specular = new Color3(1, 1, 1);
          light.intensity = 1;
          break;
      }
      
      light.position = Vector3.Zero();
      light.parent = mainContainer;
      const lightHelper = MeshBuilder.CreateSphere(`${lightName}_helper`, { diameter: 0.5 }, scene);
      lightHelper.position = Vector3.Zero();
      lightHelper.parent = mainContainer;
      const helperMaterial = new StandardMaterial(`${lightName}_helper_material`, scene);
      helperMaterial.emissiveColor = new Color3(1, 1, 0.8);
      helperMaterial.disableLighting = true;
      lightHelper.material = helperMaterial;
      lightHelper._isInternalMesh = true;
      
      // Add to scene tree and select
      renderActions.addObject(mainContainer);
      renderActions.selectObject(mainContainer);
      
      const objectId = mainContainer.uniqueId || mainContainer.name;
      
      // Initialize object properties with transform data
      objectPropertiesActions.ensureDefaultComponents(objectId);
      objectPropertiesActions.updateObjectProperty(objectId, 'transform.position', [lightPosition.x, lightPosition.y, lightPosition.z]);
      objectPropertiesActions.updateObjectProperty(objectId, 'transform.rotation', [0, 0, 0]);
      objectPropertiesActions.updateObjectProperty(objectId, 'transform.scale', [1, 1, 1]);
      
      editorActions.selectEntity(objectId);
      
      editorActions.addConsoleMessage(`Created ${lightType} light`, 'success');
    } catch (error) {
      console.error('Error creating light:', error);
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
      const cameraName = getObjectName('camera');
      const cameraPosition = await getViewportCenterPosition(scene, 6);
      cameraPosition.y += 1.7;
      const mainContainer = new TransformNode(cameraName, scene);
      mainContainer.position = cameraPosition;
      const camera = new UniversalCamera(`${cameraName}_camera`, Vector3.Zero(), scene);
      camera.setTarget(new Vector3(0, 0, 1));
      camera.fov = Math.PI / 3;
      camera.parent = mainContainer;
      const cameraHelper = MeshBuilder.CreateBox(`${cameraName}_helper`, { width: 1, height: 0.6, depth: 1.5 }, scene);
      cameraHelper.position = Vector3.Zero();
      cameraHelper.parent = mainContainer;
      const helperMaterial = new StandardMaterial(`${cameraName}_helper_material`, scene);
      helperMaterial.diffuseColor = new Color3(0.2, 0.2, 0.8);
      helperMaterial.specularColor = new Color3(0.1, 0.1, 0.1);
      cameraHelper.material = helperMaterial;
      cameraHelper._isInternalMesh = true;
      
      // Add to scene tree and select
      renderActions.addObject(mainContainer);
      renderActions.selectObject(mainContainer);
      
      const objectId = mainContainer.uniqueId || mainContainer.name;
      
      // Initialize object properties with transform data
      objectPropertiesActions.ensureDefaultComponents(objectId);
      objectPropertiesActions.updateObjectProperty(objectId, 'transform.position', [cameraPosition.x, cameraPosition.y, cameraPosition.z]);
      objectPropertiesActions.updateObjectProperty(objectId, 'transform.rotation', [0, 0, 0]);
      objectPropertiesActions.updateObjectProperty(objectId, 'transform.scale', [1, 1, 1]);
      
      editorActions.selectEntity(objectId);
      
      editorActions.addConsoleMessage('Created camera', 'success');
    } catch (error) {
      console.error('Error creating camera:', error);
      editorActions.addConsoleMessage(`Failed to create camera: ${error.message}`, 'error');
    }
  };

  const duplicateSelectedObject = async () => {
    const scene = getCurrentScene();
    const selectedObject = renderStore.selectedObject;
    
    if (!scene || !selectedObject) {
      editorActions.addConsoleMessage('No object selected to duplicate', 'warning');
      return;
    }
    
    try {
      let newObject = selectedObject.clone(selectedObject.name + '_duplicate', null, false, true);
      
      if (newObject) {
        newObject.parent = null;
        newObject.position = selectedObject.position.add(new Vector3(2, 0, 2));
        if (selectedObject.rotation && newObject.rotation) {
          newObject.rotation = selectedObject.rotation.clone();
        }
        if (selectedObject.scaling && newObject.scaling) {
          newObject.scaling = selectedObject.scaling.clone();
        }
        
        const objectId = newObject.uniqueId || newObject.name;
        
        // Use render store for selection
        renderActions.selectObject(newObject);
        
        editorActions.selectEntity(objectId);
        
        editorActions.addConsoleMessage('Object duplicated', 'success');
      }
    } catch (error) {
      console.error('Error duplicating object:', error);
      editorActions.addConsoleMessage(`Failed to duplicate object: ${error.message}`, 'error');
    }
  };

  const deleteSelectedObject = () => {
    const scene = getCurrentScene();
    const selectedObject = renderStore.selectedObject;
    
    if (!scene || !selectedObject) {
      editorActions.addConsoleMessage('No object selected to delete', 'warning');
      return;
    }
    
    if (selectedObject.name === 'ground' || selectedObject.name === 'skybox') {
      editorActions.addConsoleMessage('Cannot delete default scene objects', 'warning');
      return;
    }
    
    try {
      selectedObject.dispose();
      
      // Use render store to clear selection
      renderActions.selectObject(null);
      
      editorActions.selectEntity(null);
      
      editorActions.addConsoleMessage('Object deleted', 'success');
    } catch (error) {
      console.error('Error deleting object:', error);
      editorActions.addConsoleMessage(`Failed to delete object: ${error.message}`, 'error');
    }
  };

  return (
    <>
      <div class="relative w-full h-10 bg-base-200/90 backdrop-blur-md shadow-lg flex items-center">
        <div class="flex items-center h-full px-4 gap-1">
          
          <For each={tools()}>
            {(tool, index) => {
              const effectiveSelectedTool = getEffectiveSelectedTool();
              const isActive = (effectiveSelectedTool === tool.id) || flashingTool() === tool.id;
              const isDisabled = tool.requiresSelection && !selectedEntity();
              const showDivider = (index() === 3) || (index() === 7) || (index() === 9);
              
              return (
                <>
                  {tool.isDropdown && tool.id === 'light' ? (
                    <button
                      onClick={handleLightDropdownToggle}
                      class={`toolbar-button w-8 h-8 flex items-center justify-center rounded transition-all relative group cursor-pointer ${
                        isActive
                          ? 'bg-primary text-primary-content' 
                          : 'text-base-content/60 hover:text-base-content hover:bg-base-300'
                      }`}
                    >
                      <tool.icon class="w-5 h-5" />
                      <svg class="w-2 h-2 ml-1" fill="currentColor" viewBox="0 0 20 20">
                        <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
                      </svg>
                      
                      <div class="absolute bottom-full mb-2 left-1/2 transform -translate-x-1/2 bg-base-200 text-base-content text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                        {tool.tooltip}
                        <div class="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-base-200" />
                      </div>
                    </button>
                  ) : (
                    <button
                      onClick={() => isDisabled ? null : handleToolClick(tool.id)}
                      class={`w-8 h-8 flex items-center justify-center rounded transition-all relative group ${
                        isDisabled 
                          ? 'text-base-content/40 opacity-50'
                          : 'cursor-pointer'
                      } ${
                        isActive && !isDisabled
                          ? 'bg-primary text-primary-content' 
                          : !isDisabled
                            ? 'text-base-content/60 hover:text-base-content hover:bg-base-300'
                            : ''
                      }`}
                    >
                      <tool.icon class="w-5 h-5" />
                      
                      {!isDisabled && (
                        <div class="absolute bottom-full mb-2 left-1/2 transform -translate-x-1/2 bg-base-200 text-base-content text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                          {tool.tooltip}
                          <div class="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-base-200" />
                        </div>
                      )}
                    </button>
                  )}
                  
                  {showDivider && (
                    <div class="w-px h-6 bg-base-300 mx-1"></div>
                  )}
                  
                </>
              );
            }}
          </For>

          {/* Script control button */}
          <div class="w-px h-6 bg-base-300 mx-1"></div>
          <button
            onClick={handleScriptPlayToggle}
            class={`w-8 h-8 flex items-center justify-center rounded transition-all relative group cursor-pointer ${
              scriptRuntimePlaying()
                ? 'text-green-500 hover:text-green-400 hover:bg-green-500/10'
                : 'text-orange-500 hover:text-orange-400 hover:bg-orange-500/10'
            }`}
            title={scriptRuntimePlaying() ? 'Pause script execution' : 'Resume script execution'}
          >
            {scriptRuntimePlaying() ? (
              <Pause class="w-5 h-5" />
            ) : (
              <Play class="w-5 h-5" />
            )}
            
            <div class="absolute bottom-full mb-2 left-1/2 transform -translate-x-1/2 bg-base-200 text-base-content text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
              {scriptRuntimePlaying() ? 'Pause Scripts' : 'Play Scripts'}
              <div class="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-base-200" />
            </div>
          </button>

        </div>
        
        {/* Right side - Plugin buttons */}
        <div class="absolute right-4 top-0 h-full flex items-center gap-1">
          <For each={Array.from(toolbarButtons().values()).filter(button => button.section === 'right').sort((a, b) => (a.order || 0) - (b.order || 0))}>
            {(button) => {
              const isEnabled = horizontalMenuButtonsEnabled();

              // Handle custom component buttons
              if (button.isCustomComponent && button.customComponent) {
                const CustomComponent = button.customComponent;
                return (
                  <div class="flex items-center" title={button.title}>
                    <CustomComponent />
                  </div>
                );
              }

              if (button.hasDropdown && button.dropdownComponent) {
                const isActive = activePluginDropdown() === button.id;
                return (
                  <button
                    onClick={(e) => isEnabled && handlePluginDropdownToggle(e, button)}
                    disabled={!isEnabled}
                    class={`toolbar-button w-8 h-8 flex items-center justify-center rounded transition-all relative group ${
                      !isEnabled 
                        ? 'cursor-not-allowed text-base-content/30 opacity-50'
                        : isActive
                          ? 'bg-primary text-primary-content cursor-pointer' 
                          : 'text-base-content/60 hover:text-base-content hover:bg-base-300 cursor-pointer'
                    }`}
                  >
                    <button.icon class="w-5 h-5" />
                    <svg class="w-2 h-2 ml-1" fill="currentColor" viewBox="0 0 20 20">
                      <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
                    </svg>
                    
                    <div class="absolute bottom-full mb-2 left-1/2 transform -translate-x-1/2 bg-base-300/95 text-base-content text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                      {button.title}
                      <div class="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-base-300/95" />
                    </div>
                  </button>
                );
              }
              
              return (
                <button
                  onClick={() => isEnabled && handleToolClick(button.id)}
                  disabled={!isEnabled}
                  class={`toolbar-button w-8 h-8 flex items-center justify-center rounded transition-all relative group ${
                    !isEnabled
                      ? 'cursor-not-allowed text-base-content/30 opacity-50'
                      : 'cursor-pointer text-base-content/60 hover:text-base-content hover:bg-base-300'
                  }`}
                >
                  <button.icon class="w-5 h-5" />
                  
                  <div class="absolute bottom-full mb-2 left-1/2 transform -translate-x-1/2 bg-base-300/95 text-base-content text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                    {button.title}
                    <div class="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-base-300/95" />
                  </div>
                </button>
              );
            }}
          </For>
        </div>
        
      </div>
      
      {/* Light dropdown */}
      {showLightDropdown() && lightDropdownPosition() && (
        <div 
          class="dropdown-content fixed w-48 bg-base-200 backdrop-blur-sm rounded-lg shadow-xl border border-base-300 z-[210]"
          style={{
            left: `${lightDropdownPosition().left}px`,
            top: `${lightDropdownPosition().top}px`
          }}
        >
          <For each={lightTypes}>
            {(lightType) => (
              <button
                onClick={() => handleLightCreate(lightType.id)}
                class="w-full px-3 py-2 text-left text-sm transition-colors flex items-center gap-2 first:rounded-t-lg last:rounded-b-lg text-base-content hover:bg-base-300 hover:text-base-content"
              >
                <lightType.icon class="w-4 h-4" />
                {lightType.label}
              </button>
            )}
          </For>
        </div>
      )}


      {/* Plugin dropdowns */}
      {activePluginDropdown() && pluginDropdownPosition() && (
        <div 
          class="dropdown-content fixed bg-base-200 backdrop-blur-sm rounded-lg shadow-xl border border-base-300 z-[210] text-base-content text-xs"
          style={{
            left: `${pluginDropdownPosition().left}px`,
            top: `${pluginDropdownPosition().top}px`
          }}
        >
          {(() => {
            const activeButton = Array.from(toolbarButtons().values())
              .find(b => b.id === activePluginDropdown());
            if (activeButton && activeButton.dropdownComponent) {
              const DropdownComponent = activeButton.dropdownComponent;
              return <DropdownComponent />;
            }
            return null;
          })()}
        </div>
      )}
    </>
  );
}

export default Toolbar;