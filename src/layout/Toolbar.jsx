import { createSignal, For } from 'solid-js';
import Helper from './Helper.jsx';
import { editorStore, editorActions } from "@/layout/stores/EditorStore";
import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";
import { IconSettings, IconX, IconPointer, IconArrowsMove, IconRefresh, IconMaximize, IconVideo, IconCopy, IconTrash, IconBox, IconCircle, IconCylinder, IconSquare, IconSun, IconBulb } from '@tabler/icons-solidjs';
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
      
      <div class="flex-1" />
      
      <Helper />
    </div>
  );
}

export default Toolbar;