import { createSignal, For, Show } from 'solid-js';
import Helper from './Helper.jsx';
import { helperVisible } from '@/api/plugin';
import { editorStore, editorActions } from "@/layout/stores/EditorStore";
import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";
import { IconSettings, IconX, IconPointer, IconArrowsMove, IconRefresh, IconMaximize, IconVideo, IconCopy, IconTrash, IconBox, IconCircle, IconCylinder, IconSquare, IconSun, IconBulb, IconPlayerPlay, IconPlayerPause, IconChevronDown, IconCube, IconDeviceGamepad2, IconBrush, IconMovie } from '@tabler/icons-solidjs';
import { renderStore, renderActions } from '@/render/store.jsx';
import { getScriptRuntime } from '@/api/script';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { Ray } from '@babylonjs/core/Culling/ray';
import { TransformNode } from '@babylonjs/core/Meshes/transformNode';
import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { PBRMaterial } from '@babylonjs/core/Materials/PBR/pbrMaterial';
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
  // Access store properties reactively
  const selection = () => editorStore.selection;
  const selectedEntity = () => selection().entity;
  const selectedTool = () => editorStore.ui.selectedTool;
  const transformMode = () => selection().transformMode;
  
  const { setSelectedTool, setTransformMode, selectEntity } = editorActions;
  
  // Camera view dropdown state
  const [cameraViewDropdownOpen, setCameraViewDropdownOpen] = createSignal(false);
  const [currentViewName, setCurrentViewName] = createSignal("View");
  
  // Mode dropdown state
  const [modeDropdownOpen, setModeDropdownOpen] = createSignal(false);
  const currentMode = () => editorStore.ui.currentMode;
  
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
      standard: { name: "Standard", icon: IconCube, description: "General scene editing" },
      levelPrototyping: { name: "Level Editor", icon: IconDeviceGamepad2, description: "Quick level prototyping with snapping" },
      sculpting: { name: "Sculpting", icon: IconBrush, description: "Terrain and mesh sculpting" },
      animation: { name: "Animation", icon: IconMovie, description: "Timeline and keyframe editing" }
    };
    return modeData[mode] || { name: "Unknown", icon: IconCube, description: "" };
  };
  
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
  
  const getViewportCenterPosition = async (scene, distance = 5) => {
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

  const createBabylonPrimitive = async (type) => {
    console.log('Creating primitive:', type);
    
    const scene = getCurrentScene();
    if (!scene) {
      editorActions.addConsoleMessage('No active scene available', 'error');
      return;
    }

    const position = new Vector3(0, 0.5, 0);
    
    const objectName = getObjectName(type);

    try {
      let primitive;

      switch (type) {
        case 'cube':
          primitive = MeshBuilder.CreateBox(objectName, { size: 1 }, scene);
          break;
        case 'sphere':
          primitive = MeshBuilder.CreateSphere(objectName, { diameter: 1 }, scene);
          break;
        case 'cylinder':
          primitive = MeshBuilder.CreateCylinder(objectName, { height: 1, diameter: 1 }, scene);
          break;
        case 'plane':
          primitive = MeshBuilder.CreateGround(objectName, { width: 1, height: 1 }, scene);
          break;
        default:
          throw new Error(`Unknown primitive type: ${type}`);
      }

      primitive.position = position;
      
      let material;
      if (type === 'plane') {
        material = new StandardMaterial(`${objectName}_material`, scene);
        material.diffuseColor = new Color3(0.8, 0.8, 0.8);
      } else {
        material = new PBRMaterial(`${objectName}_material`, scene);
        material.baseColor = new Color3(0.8, 0.8, 0.8);
        material.metallicFactor = 0.1;
        material.roughnessFactor = 0.8;
      }
      
      primitive.material = material;
      
      // Add object to hierarchy first, then select it
      renderActions.addObject(primitive);
      renderActions.selectObject(primitive);
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
      lightHelper.material = new StandardMaterial(`${lightName}_helper_material`, scene);
      lightHelper.material.emissiveColor = new Color3(1, 1, 0);
      lightHelper.material.disableLighting = true;
      lightHelper.parent = mainContainer;

      // Add object to hierarchy first, then select it
      renderActions.addObject(mainContainer);
      renderActions.selectObject(mainContainer);
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
      const cameraName = getObjectName('camera');
      const cameraPosition = await getViewportCenterPosition(scene, 6);
      cameraPosition.y += 2;

      const camera = new UniversalCamera(cameraName, cameraPosition, scene);
      camera.setTarget(Vector3.Zero());

      // Add object to hierarchy first, then select it
      renderActions.addObject(camera);
      renderActions.selectObject(camera);
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
        
        // Add object to hierarchy first, then select it
        renderActions.addObject(newObject);
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
      setCurrentViewName(getViewDisplayName(viewType));
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
      await createBabylonLight();
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
    else {
      editorActions.addConsoleMessage(`Tool activated: ${toolId}`, 'info');
    }
  };

  const tools = [
    { id: 'select', icon: IconPointer, tooltip: 'Select' },
    { id: 'move', icon: IconArrowsMove, tooltip: 'Move' },
    { id: 'rotate', icon: IconRefresh, tooltip: 'Rotate' },
    { id: 'scale', icon: IconMaximize, tooltip: 'Scale' },
    null, // Separator
    { 
      id: 'play_pause', 
      icon: IconPlayerPlay, 
      tooltip: 'Toggle Script Execution',
      isDynamic: true
    },
    null, // Separator
    { id: 'camera', icon: IconVideo, tooltip: 'Add Camera' },
    { id: 'cube', icon: IconBox, tooltip: 'Add Cube' },
    { id: 'sphere', icon: IconCircle, tooltip: 'Add Sphere' },
    { id: 'cylinder', icon: IconCylinder, tooltip: 'Add Cylinder' },
    { id: 'plane', icon: IconSquare, tooltip: 'Add Plane' },
    { id: 'light', icon: IconSun, tooltip: 'Add Light' },
    null, // Separator
    { id: 'duplicate', icon: IconCopy, tooltip: 'Duplicate' },
    { id: 'delete', icon: IconTrash, tooltip: 'Delete' }
  ];

  const setMode = (mode) => {
    editorActions.setCurrentMode(mode);
    setModeDropdownOpen(false);
    editorActions.addConsoleMessage(`Switched to ${getModeDisplayData(mode).name} mode`, 'info');
  };

  return (
    <div class="w-full h-10 flex items-center bg-base-200 border-b border-base-300 px-2 gap-1">
      <For each={tools}>
        {(tool) => 
          tool === null ? (
            <div class="w-px h-6 bg-base-content/20 mx-1"></div>
          ) : (
            <button 
              onClick={() => handleToolbarClick(tool.id)}
              class={`w-8 h-8 flex items-center justify-center rounded transition-all group ${
                getSelectedTool() === tool.id
                  ? 'bg-primary text-primary-content'
                  : tool.id === 'play_pause' && editorStore.scripts.isPlaying
                    ? 'bg-warning text-warning-content'
                    : 'text-base-content/60 hover:text-base-content hover:bg-base-300'
              }`} 
              title={tool.tooltip}
            >
              {tool.isDynamic && tool.id === 'play_pause' ? 
                (editorStore.scripts.isPlaying ? 
                  <IconPlayerPause class="w-4 h-4" /> : 
                  <IconPlayerPlay class="w-4 h-4" />
                ) :
                <tool.icon class="w-4 h-4" />
              }
              
              <div class="absolute top-full mt-2 bg-base-200 text-base-content text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                {tool.isDynamic && tool.id === 'play_pause' ? 
                  (editorStore.scripts.isPlaying ? 'Pause Scripts' : 'Play Scripts') :
                  tool.tooltip
                }
              </div>
            </button>
          )
        }
      </For>
      
      <div class="flex-1" />
      
      {/* Mode Dropdown */}
      <div class="relative" data-mode-dropdown>
        <button
          onClick={() => {
            setModeDropdownOpen(!modeDropdownOpen());
            // Close camera dropdown when mode dropdown opens
            setCameraViewDropdownOpen(false);
          }}
          class="h-8 px-3 flex items-center gap-2 rounded text-base-content/60 hover:text-base-content hover:bg-base-300 text-xs transition-all"
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
                class={`w-full flex items-center gap-3 px-3 py-2 text-xs rounded hover:bg-base-300 text-left ${
                  currentMode() === 'standard' ? 'bg-base-300 text-base-content' : ''
                }`}
              >
                <IconCube class="w-4 h-4" />
                <div class="flex-1">
                  <div class="font-medium">Standard</div>
                  <div class="text-xs text-base-content/60">General scene editing</div>
                </div>
              </button>
              
              <button
                onClick={() => setMode('levelPrototyping')}
                class={`w-full flex items-center gap-3 px-3 py-2 text-xs rounded hover:bg-base-300 text-left ${
                  currentMode() === 'levelPrototyping' ? 'bg-base-300 text-base-content' : ''
                }`}
              >
                <IconDeviceGamepad2 class="w-4 h-4" />
                <div class="flex-1">
                  <div class="font-medium">Level Editor</div>
                  <div class="text-xs text-base-content/60">Quick level prototyping with snapping</div>
                </div>
              </button>
              
              <button
                onClick={() => setMode('sculpting')}
                class={`w-full flex items-center gap-3 px-3 py-2 text-xs rounded hover:bg-base-300 text-left ${
                  currentMode() === 'sculpting' ? 'bg-base-300 text-base-content' : ''
                }`}
              >
                <IconBrush class="w-4 h-4" />
                <div class="flex-1">
                  <div class="font-medium">Sculpting</div>
                  <div class="text-xs text-base-content/60">Terrain and mesh sculpting</div>
                </div>
              </button>
              
              <button
                onClick={() => setMode('animation')}
                class={`w-full flex items-center gap-3 px-3 py-2 text-xs rounded hover:bg-base-300 text-left ${
                  currentMode() === 'animation' ? 'bg-base-300 text-base-content' : ''
                }`}
              >
                <IconMovie class="w-4 h-4" />
                <div class="flex-1">
                  <div class="font-medium">Animation</div>
                  <div class="text-xs text-base-content/60">Timeline and keyframe editing</div>
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
            // Close helper dropdowns when camera dropdown opens
            if (window._closeHelperDropdowns) {
              window._closeHelperDropdowns();
            }
          }}
          class="h-8 px-3 flex items-center gap-1 rounded text-base-content/60 hover:text-base-content hover:bg-base-100/50 text-xs transition-all"
          title="Camera Views (Numpad shortcuts)"
        >
          <IconVideo class="w-3 h-3" />
          <span>{currentViewName()}</span>
          <IconChevronDown class="w-3 h-3" />
        </button>
        
        <Show when={cameraViewDropdownOpen()}>
          <div class="absolute right-0 top-full mt-1 bg-base-200 border border-base-300 rounded shadow-lg z-50 min-w-48">
            <div class="p-2 space-y-1">
              <div class="text-xs text-base-content/60 px-2 py-1 font-medium">Camera Views</div>
              
              <button
                onClick={() => setCameraView('front')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded hover:bg-base-300 text-left"
              >
                <span>Front Orthographic</span>
                <span class="text-base-content/60">1</span>
              </button>
              
              <button
                onClick={() => setCameraView('back')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded hover:bg-base-300 text-left"
              >
                <span>Back</span>
                <span class="text-base-content/60">Ctrl+1</span>
              </button>
              
              <button
                onClick={() => setCameraView('right')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded hover:bg-base-300 text-left"
              >
                <span>Right Orthographic</span>
                <span class="text-base-content/60">3</span>
              </button>
              
              <button
                onClick={() => setCameraView('left')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded hover:bg-base-300 text-left"
              >
                <span>Left</span>
                <span class="text-base-content/60">Ctrl+3</span>
              </button>
              
              <button
                onClick={() => setCameraView('top')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded hover:bg-base-300 text-left"
              >
                <span>Top Orthographic</span>
                <span class="text-base-content/60">7</span>
              </button>
              
              <button
                onClick={() => setCameraView('bottom')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded hover:bg-base-300 text-left"
              >
                <span>Bottom</span>
                <span class="text-base-content/60">Ctrl+7</span>
              </button>
              
              <div class="w-full h-px bg-base-300 my-1"></div>
              
              <button
                onClick={() => setCameraView('frontLeft')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded hover:bg-base-300 text-left"
              >
                <span>Front Left</span>
                <span class="text-base-content/60">8</span>
              </button>
              
              <button
                onClick={() => setCameraView('frontRight')}
                class="w-full flex items-center justify-between px-2 py-1 text-xs rounded hover:bg-base-300 text-left"
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