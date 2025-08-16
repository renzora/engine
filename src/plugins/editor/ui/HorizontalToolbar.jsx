import { createSignal, createEffect, onCleanup, For } from 'solid-js';
import { horizontalMenuButtonsEnabled } from '@/plugins/core/engine/EngineAPI';
import { 
  IconSun, 
  IconBulb, 
  IconPointer, 
  IconArrowsMove, 
  IconRotateClockwise2, 
  IconMaximize, 
  IconCube, 
  IconCircle, 
  IconSquare, 
  IconVideo, 
  IconCopy, 
  IconTrash 
} from '@tabler/icons-solidjs';
import { editorStore, editorActions } from '@/plugins/editor/stores/EditorStore';
import { toolbarButtons } from '@/plugins/core/engine';
import { sceneStore, sceneActions, babylonScene } from '@/plugins/core/render/store';
import { viewportStore, viewportActions, objectPropertiesActions } from '@/plugins/editor/stores/ViewportStore';
import CameraHelpers from './CameraHelpers';
import GridHelpers from './GridHelpers';
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
// Required BabylonJS imports for mesh builders
import '@babylonjs/core/Meshes/Builders/boxBuilder';
import '@babylonjs/core/Meshes/Builders/sphereBuilder';
import '@babylonjs/core/Meshes/Builders/cylinderBuilder';
import '@babylonjs/core/Meshes/Builders/planeBuilder';

function HorizontalToolbar() {
  const [showProjectManager, setShowProjectManager] = createSignal(false);
  const [flashingTool, setFlashingTool] = createSignal(null);
  const [showLightDropdown, setShowLightDropdown] = createSignal(false);
  const [lightDropdownPosition, setLightDropdownPosition] = createSignal(null);
  // Plugin dropdown state
  const [activePluginDropdown, setActivePluginDropdown] = createSignal(null);
  const [pluginDropdownPosition, setPluginDropdownPosition] = createSignal(null);
  
  // Click outside detection for dropdowns
  createEffect(() => {
    const handleClickOutside = (event) => {
      // Check if click is on a toolbar button or inside a dropdown
      const target = event.target;
      const isToolbarButton = target.closest('.toolbar-button');
      const isDropdownContent = target.closest('.dropdown-content');
      
      if (!isToolbarButton && !isDropdownContent) {
        // Close all dropdowns
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
    { id: 'directional', label: 'Directional Light', icon: IconSun },
    { id: 'point', label: 'Point Light', icon: IconBulb },
    { id: 'spot', label: 'Spot Light', icon: IconSun },
    { id: 'hemisphere', label: 'Hemisphere Light', icon: IconBulb }
  ];
  
  const workflowTools = {
    '3d-viewport': [
      { id: 'select', icon: IconPointer, tooltip: 'Select' },
      { id: 'move', icon: IconArrowsMove, tooltip: 'Move', requiresSelection: true },
      { id: 'rotate', icon: IconRotateClockwise2, tooltip: 'Rotate', requiresSelection: true },
      { id: 'scale', icon: IconMaximize, tooltip: 'Scale', requiresSelection: true },
      { id: 'cube', icon: IconCube, tooltip: 'Add Cube' },
      { id: 'sphere', icon: IconCircle, tooltip: 'Add Sphere' },
      { id: 'cylinder', icon: IconCube, tooltip: 'Add Cylinder' },
      { id: 'plane', icon: IconSquare, tooltip: 'Add Plane' },
      { id: 'light', icon: IconSun, tooltip: 'Add Light', isDropdown: true },
      { id: 'camera', icon: IconVideo, tooltip: 'Add Camera' },
      { id: 'duplicate', icon: IconCopy, tooltip: 'Duplicate', requiresSelection: true },
      { id: 'delete', icon: IconTrash, tooltip: 'Delete', requiresSelection: true },
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
    
    // Handle plugin toolbar buttons
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
      
      const scene = getCurrentScene();
      if (scene && scene._gizmoManager) {
        scene._gizmoManager.positionGizmoEnabled = false;
        scene._gizmoManager.rotationGizmoEnabled = false;
        scene._gizmoManager.scaleGizmoEnabled = false;
        
        switch (toolId) {
          case 'move':
            scene._gizmoManager.positionGizmoEnabled = true;
            break;
          case 'rotate':
            scene._gizmoManager.rotationGizmoEnabled = true;
            break;
          case 'scale':
            scene._gizmoManager.scaleGizmoEnabled = true;
            break;
          case 'select':
            break;
        }
        
        if (scene._ensureGizmoThickness) {
          scene._ensureGizmoThickness();
        }
        
        editorActions.addConsoleMessage(`Switched to ${toolId} tool`, 'info');
      }
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

  const handleLightDropdownToggle = (e) => {
    if (showLightDropdown()) {
      setShowLightDropdown(false);
      setLightDropdownPosition(null);
    } else {
      // Close plugin dropdown if open
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
    const margin = 8; // Margin from edge of screen
    
    // Try to center the dropdown under the button
    let left = buttonRect.left + (buttonRect.width / 2) - (dropdownWidth / 2);
    
    // Check if dropdown would go off the right edge
    if (left + dropdownWidth + margin > viewportWidth) {
      left = viewportWidth - dropdownWidth - margin;
    }
    
    // Check if dropdown would go off the left edge
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
      // Close light dropdown if open
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
    console.log('HorizontalToolbar: Getting current scene - babylonScene:', babylonScene);
    console.log('HorizontalToolbar: babylonScene.current:', babylonScene.current);
    return babylonScene.current;
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

    // Use imported modules

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
        
        // Don't mark user-created objects as internal meshes!
        // mesh._isInternalMesh = true; // REMOVED - this was hiding the mesh from scene sync
        
        if (scene._gizmoManager) {
          scene._gizmoManager.attachToMesh(mainContainer);
          
          // Enable move gizmo by default when creating objects
          scene._gizmoManager.positionGizmoEnabled = true;
          scene._gizmoManager.rotationGizmoEnabled = false;
          scene._gizmoManager.scaleGizmoEnabled = false;
          
          if (scene._highlightLayer) {
            scene._highlightLayer.removeAllMeshes();
            try {
              const childMeshes = mainContainer.getChildMeshes();
              childMeshes.forEach(childMesh => {
                if (childMesh.getClassName() === 'Mesh') {
                  scene._highlightLayer.addMesh(childMesh, Color3.Yellow());
                }
              });
            } catch (error) {
              console.warn('Could not add highlight to primitive:', error);
            }
          }
        }
        
        const objectId = mainContainer.uniqueId || mainContainer.name;
        
        // Initialize object properties with transform data
        objectPropertiesActions.ensureDefaultComponents(objectId);
        objectPropertiesActions.updateObjectProperty(objectId, 'transform.position', [position.x, position.y, position.z]);
        objectPropertiesActions.updateObjectProperty(objectId, 'transform.rotation', [0, 0, 0]);
        objectPropertiesActions.updateObjectProperty(objectId, 'transform.scale', [1, 1, 1]);
        
        editorActions.selectEntity(objectId);
        setTransformMode('move'); // Set to move mode when creating objects
        
        // Import and call scene actions to select and refresh
        import('@/plugins/core/render/store.jsx').then(({ sceneActions }) => {
          sceneActions.selectObject(objectId);
          sceneActions.updateScene(scene);
        });
        
        console.log('Primitive created successfully:', mainContainer.name, 'ID:', mainContainer.uniqueId);
        editorActions.addConsoleMessage(`Created ${type} on ground`, 'success');
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

    // Use imported modules

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
      
      if (scene._gizmoManager) {
        scene._gizmoManager.attachToMesh(mainContainer);
        
        if (scene._highlightLayer) {
          scene._highlightLayer.removeAllMeshes();
          try {
            const childMeshes = mainContainer.getChildMeshes();
            childMeshes.forEach(childMesh => {
              if (childMesh.getClassName() === 'Mesh') {
                scene._highlightLayer.addMesh(childMesh, Color3.Yellow());
              }
            });
          } catch (error) {
            console.warn('Could not add highlight to light:', error);
          }
        }
      }
      
      const objectId = mainContainer.uniqueId || mainContainer.name;
      
      // Initialize object properties with transform data
      objectPropertiesActions.ensureDefaultComponents(objectId);
      objectPropertiesActions.updateObjectProperty(objectId, 'transform.position', [lightPosition.x, lightPosition.y, lightPosition.z]);
      objectPropertiesActions.updateObjectProperty(objectId, 'transform.rotation', [0, 0, 0]);
      objectPropertiesActions.updateObjectProperty(objectId, 'transform.scale', [1, 1, 1]);
      
      editorActions.selectEntity(objectId);
      
      // Import and call scene actions to select and refresh
      import('@/plugins/core/render/store.jsx').then(({ sceneActions }) => {
        sceneActions.selectObject(objectId);
        sceneActions.updateScene(scene);
      });
      
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

    // Use imported modules

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
      
      if (scene._gizmoManager) {
        scene._gizmoManager.attachToMesh(mainContainer);
      
        if (scene._highlightLayer) {
          scene._highlightLayer.removeAllMeshes();
          try {
            const childMeshes = mainContainer.getChildMeshes();
            childMeshes.forEach(childMesh => {
              if (childMesh.getClassName() === 'Mesh') {
                scene._highlightLayer.addMesh(childMesh, Color3.Yellow());
              }
            });
          } catch (error) {
            console.warn('Could not add highlight to camera:', error);
          }
        }
      }
      
      const objectId = mainContainer.uniqueId || mainContainer.name;
      
      // Initialize object properties with transform data
      objectPropertiesActions.ensureDefaultComponents(objectId);
      objectPropertiesActions.updateObjectProperty(objectId, 'transform.position', [cameraPosition.x, cameraPosition.y, cameraPosition.z]);
      objectPropertiesActions.updateObjectProperty(objectId, 'transform.rotation', [0, 0, 0]);
      objectPropertiesActions.updateObjectProperty(objectId, 'transform.scale', [1, 1, 1]);
      
      editorActions.selectEntity(objectId);
      
      // Import and call scene actions to select and refresh
      import('@/plugins/core/render/store.jsx').then(({ sceneActions }) => {
        sceneActions.selectObject(objectId);
        sceneActions.updateScene(scene);
      });
      
      editorActions.addConsoleMessage('Created camera', 'success');
    } catch (error) {
      console.error('Error creating camera:', error);
      editorActions.addConsoleMessage(`Failed to create camera: ${error.message}`, 'error');
    }
  };

  const duplicateSelectedObject = async () => {
    const scene = getCurrentScene();
    if (!scene || !scene._gizmoManager?.attachedMesh) {
      editorActions.addConsoleMessage('No object selected to duplicate', 'warning');
      return;
    }

    // Use imported modules

    const attachedMesh = scene._gizmoManager.attachedMesh;
    
    try {
      let newObject = attachedMesh.clone(attachedMesh.name + '_duplicate', null, false, true);
      
      if (newObject) {
        newObject.parent = null;
        newObject.position = attachedMesh.position.add(new Vector3(2, 0, 2));
        if (attachedMesh.rotation && newObject.rotation) {
          newObject.rotation = attachedMesh.rotation.clone();
        }
        if (attachedMesh.scaling && newObject.scaling) {
          newObject.scaling = attachedMesh.scaling.clone();
        }
        
        const objectId = newObject.uniqueId || newObject.name;
        
        if (scene._highlightLayer) {
          scene._highlightLayer.removeAllMeshes();
        }
        
        scene._gizmoManager.attachToMesh(newObject);
        
        if (scene._highlightLayer) {
          try {
            scene._highlightLayer.addMesh(newObject, Color3.Yellow());
          } catch (highlightError) {
            console.warn('Could not add highlight to duplicated object:', highlightError);
          }
        }
        
        editorActions.selectEntity(objectId);
        
        // Import and call scene actions to select and refresh
        setTimeout(() => {
          import('@/plugins/core/render/store.jsx').then(({ sceneActions }) => {
            sceneActions.selectObject(objectId);
            sceneActions.updateScene(scene);
          });
        }, 100);
        
        editorActions.addConsoleMessage('Object duplicated', 'success');
      }
    } catch (error) {
      console.error('Error duplicating object:', error);
      editorActions.addConsoleMessage(`Failed to duplicate object: ${error.message}`, 'error');
    }
  };

  const deleteSelectedObject = () => {
    const scene = getCurrentScene();
    if (!scene || !scene._gizmoManager?.attachedMesh) {
      editorActions.addConsoleMessage('No object selected to delete', 'warning');
      return;
    }

    const attachedMesh = scene._gizmoManager.attachedMesh;
    
    if (attachedMesh.name === 'ground' || attachedMesh.name === 'skybox') {
      editorActions.addConsoleMessage('Cannot delete default scene objects', 'warning');
      return;
    }
    
    try {
      attachedMesh.dispose();
      scene._gizmoManager.attachToMesh(null);
      if (scene._highlightLayer) {
        scene._highlightLayer.removeAllMeshes();
      }
      
      editorActions.selectEntity(null);
      
      // Import and call scene actions to deselect and refresh
      import('@/plugins/core/render/store.jsx').then(({ sceneActions }) => {
        sceneActions.selectObject(null);
        sceneActions.updateScene(scene);
      });
      
      editorActions.addConsoleMessage('Object deleted', 'success');
    } catch (error) {
      console.error('Error deleting object:', error);
      editorActions.addConsoleMessage(`Failed to delete object: ${error.message}`, 'error');
    }
  };

  return (
    <>
      <div class="relative w-full h-10 bg-gray-900/95 backdrop-blur-sm border-b border-gray-800 flex items-center">
        <div class="flex items-center h-full px-4 gap-1">
          
          <For each={tools()}>
            {(tool, index) => {
              const effectiveSelectedTool = getEffectiveSelectedTool();
              const isActive = (effectiveSelectedTool === tool.id) || flashingTool() === tool.id;
              const isDisabled = tool.requiresSelection && !selectedEntity();
              const showDivider = (index() === 3) || (index() === 9);
              
              return (
                <>
                  {tool.isDropdown && tool.id === 'light' ? (
                    <button
                      onClick={handleLightDropdownToggle}
                      class={`toolbar-button w-8 h-8 flex items-center justify-center rounded transition-all relative group cursor-pointer ${
                        isActive
                          ? 'bg-blue-600/90 text-white' 
                          : 'text-gray-400 hover:text-gray-200 hover:bg-slate-800'
                      }`}
                    >
                      <tool.icon class="w-4 h-4" />
                      <svg class="w-2 h-2 ml-1" fill="currentColor" viewBox="0 0 20 20">
                        <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
                      </svg>
                      
                      <div class="absolute bottom-full mb-2 left-1/2 transform -translate-x-1/2 bg-gray-900/95 text-white text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                        {tool.tooltip}
                        <div class="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-gray-900/95" />
                      </div>
                    </button>
                  ) : (
                    <button
                      onClick={() => isDisabled ? null : handleToolClick(tool.id)}
                      class={`w-8 h-8 flex items-center justify-center rounded transition-all relative group ${
                        isDisabled 
                          ? 'text-gray-600 opacity-50'
                          : 'cursor-pointer'
                      } ${
                        isActive && !isDisabled
                          ? 'bg-blue-600/90 text-white' 
                          : !isDisabled
                            ? 'text-gray-400 hover:text-gray-200 hover:bg-slate-800'
                            : ''
                      }`}
                    >
                      <tool.icon class="w-4 h-4" />
                      
                      {!isDisabled && (
                        <div class="absolute bottom-full mb-2 left-1/2 transform -translate-x-1/2 bg-gray-900/95 text-white text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                          {tool.tooltip}
                          <div class="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-gray-900/95" />
                        </div>
                      )}
                    </button>
                  )}
                  
                  {showDivider && (
                    <div class="w-px h-6 bg-gray-700 mx-1"></div>
                  )}
                  
                </>
              );
            }}
          </For>

        </div>
        
        {/* Right side menu with dynamic toolbar buttons */}
        <div class="absolute right-4 top-0 h-full flex items-center gap-1">
          <For each={Array.from(toolbarButtons().values()).filter(button => button.section === 'right').sort((a, b) => (a.order || 0) - (b.order || 0))}>
            {(button) => {
              const isEnabled = horizontalMenuButtonsEnabled();
              
              // Handle dropdown buttons
              if (button.hasDropdown && button.dropdownComponent) {
                const isActive = activePluginDropdown() === button.id;
                return (
                  <button
                    onClick={(e) => isEnabled && handlePluginDropdownToggle(e, button)}
                    disabled={!isEnabled}
                    class={`toolbar-button w-8 h-8 flex items-center justify-center rounded transition-all relative group ${
                      !isEnabled 
                        ? 'cursor-not-allowed text-gray-600 opacity-50'
                        : isActive
                          ? 'bg-blue-600/90 text-white cursor-pointer' 
                          : 'text-gray-400 hover:text-gray-200 hover:bg-slate-800 cursor-pointer'
                    }`}
                  >
                    <button.icon class="w-4 h-4" />
                    <svg class="w-2 h-2 ml-1" fill="currentColor" viewBox="0 0 20 20">
                      <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
                    </svg>
                    
                    <div class="absolute bottom-full mb-2 left-1/2 transform -translate-x-1/2 bg-gray-900/95 text-white text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                      {button.title}
                      <div class="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-gray-900/95" />
                    </div>
                  </button>
                );
              }
              
              // Handle regular buttons
              return (
                <button
                  onClick={() => isEnabled && handleToolClick(button.id)}
                  disabled={!isEnabled}
                  class={`toolbar-button w-8 h-8 flex items-center justify-center rounded transition-all relative group ${
                    !isEnabled
                      ? 'cursor-not-allowed text-gray-600 opacity-50'
                      : 'cursor-pointer text-gray-400 hover:text-gray-200 hover:bg-slate-800'
                  }`}
                >
                  <button.icon class="w-4 h-4" />
                  
                  <div class="absolute bottom-full mb-2 left-1/2 transform -translate-x-1/2 bg-gray-900/95 text-white text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                    {button.title}
                    <div class="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-gray-900/95" />
                  </div>
                </button>
              );
            }}
          </For>
        </div>
        
      </div>
      
      {showLightDropdown() && lightDropdownPosition() && (
        <div 
          class="dropdown-content fixed w-48 bg-gray-800/95 backdrop-blur-sm rounded-lg shadow-xl border border-gray-600/50 z-[210]"
          style={{
            left: `${lightDropdownPosition().left}px`,
            top: `${lightDropdownPosition().top}px`
          }}
        >
          <For each={lightTypes}>
            {(lightType) => (
              <button
                onClick={() => handleLightCreate(lightType.id)}
                class="w-full px-3 py-2 text-left text-sm transition-colors flex items-center gap-2 first:rounded-t-lg last:rounded-b-lg text-gray-300 hover:bg-gray-900/60 hover:text-white"
              >
                <lightType.icon class="w-4 h-4" />
                {lightType.label}
              </button>
            )}
          </For>
        </div>
      )}

      {/* Plugin dropdown */}
      {activePluginDropdown() && pluginDropdownPosition() && (
        <div 
          class="dropdown-content fixed bg-gray-800/95 backdrop-blur-sm rounded-lg shadow-xl border border-gray-600/50 z-[210] text-white text-xs"
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

      {/* ProjectManager component not imported - TODO: import or remove
      {showProjectManager() && (
        <ProjectManager
          onProjectLoad={(name, path) => {
            console.log(`Project loaded: ${name} at ${path}`)
            editorActions.addConsoleMessage(`Project "${name}" loaded successfully`, 'success')
          }}
          onClose={() => setShowProjectManager(false)}
        />
      )} */}
    </>
  );
}

export default HorizontalToolbar;