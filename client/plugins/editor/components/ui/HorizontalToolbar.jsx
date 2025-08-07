import React, { useState, useEffect, useRef } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';
import { useSnapshot } from 'valtio';
import { globalStore, actions, babylonScene } from "@/store.js";
import ProjectManager from '@/plugins/projects/components/ProjectManager.jsx';
import CameraHelpers from './CameraHelpers.jsx';
import GridHelpers from './GridHelpers.jsx';
import * as BABYLON from '@babylonjs/core';

function HorizontalToolbar() {
  const [showProjectManager, setShowProjectManager] = useState(false);
  const [flashingTool, setFlashingTool] = useState(null);
  const [showLightDropdown, setShowLightDropdown] = useState(false);
  const [lightDropdownPosition, setLightDropdownPosition] = useState(null);
  const { selection, ui, camera, viewport, settings } = useSnapshot(globalStore.editor);
  const { selectedTool } = ui;
  const { transformMode, entity: selectedEntity } = selection;
  const { setSelectedTool, setTransformMode } = actions.editor;
  
  // Get current active viewport type for workflow filtering
  const getCurrentWorkflow = () => {
    if (!viewport.tabs || viewport.tabs.length === 0) {
      return '3d-viewport';
    }
    const activeTabData = viewport.tabs.find(tab => tab.id === viewport.activeTabId);
    return activeTabData?.type || '3d-viewport';
  };

  // Light types for dropdown
  const lightTypes = [
    { id: 'directional', label: 'Directional Light', icon: Icons.LightDirectional },
    { id: 'point', label: 'Point Light', icon: Icons.LightPoint },
    { id: 'spot', label: 'Spot Light', icon: Icons.LightSpot },
    { id: 'hemisphere', label: 'Hemisphere Light', icon: Icons.LightBulb }
  ];
  

  // Workflow-specific tool sets
  const workflowTools = {
    '3d-viewport': [
      // Transform tools
      { id: 'select', icon: Icons.MousePointer, tooltip: 'Select' },
      { id: 'move', icon: Icons.Move, tooltip: 'Move', requiresSelection: true },
      { id: 'rotate', icon: Icons.RotateCcw, tooltip: 'Rotate', requiresSelection: true },
      { id: 'scale', icon: Icons.Maximize, tooltip: 'Scale', requiresSelection: true },
      
      // Create tools
      { id: 'cube', icon: Icons.Cube3D, tooltip: 'Add Cube' },
      { id: 'sphere', icon: Icons.Circle, tooltip: 'Add Sphere' },
      { id: 'cylinder', icon: Icons.Cylinder, tooltip: 'Add Cylinder' },
      { id: 'plane', icon: Icons.Square, tooltip: 'Add Plane' },
      { id: 'light', icon: Icons.Sun, tooltip: 'Add Light', isDropdown: true },
      { id: 'camera', icon: Icons.Video, tooltip: 'Add Camera' },
      
      // Edit tools
      { id: 'duplicate', icon: Icons.Copy, tooltip: 'Duplicate', requiresSelection: true },
      { id: 'delete', icon: Icons.Trash, tooltip: 'Delete', requiresSelection: true },
    ]
  };
  
  // Get tools for current workflow
  const currentWorkflow = getCurrentWorkflow();
  const tools = workflowTools[currentWorkflow] || workflowTools['3d-viewport'];

  // Get effective selected tool (matches the old logic)
  const getEffectiveSelectedTool = () => {
    if (['select', 'move', 'rotate', 'scale'].includes(transformMode)) {
      return transformMode;
    }
    return selectedTool;
  };

  const handleToolClick = (toolId) => {
    console.log('HorizontalToolbar: Tool clicked:', toolId);
    
    // Add immediate feedback
    actions.editor.addConsoleMessage(`Clicked ${toolId} button`, 'info');
    
    // Handle transform tools
    if (['select', 'move', 'rotate', 'scale'].includes(toolId)) {
      // Check if selection is required for non-select tools
      if (toolId !== 'select' && !selectedEntity) {
        actions.editor.addConsoleMessage('Please select an object first', 'warning');
        return;
      }
      
      setTransformMode(toolId);
      // Don't call setSelectedTool to avoid switching right panel tabs
      
      // Update gizmo mode in Babylon scene
      const scene = getCurrentScene();
      if (scene && scene._gizmoManager) {
        // Disable all gizmos first
        scene._gizmoManager.positionGizmoEnabled = false;
        scene._gizmoManager.rotationGizmoEnabled = false;
        scene._gizmoManager.scaleGizmoEnabled = false;
        
        // Enable the selected gizmo
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
            // Keep all gizmos disabled for select mode
            break;
        }
        
        // Ensure gizmo thickness is applied after switching modes
        if (scene._ensureGizmoThickness) {
          scene._ensureGizmoThickness();
        }
        
        actions.editor.addConsoleMessage(`Switched to ${toolId} tool`, 'info');
      }
    }
    // Handle create tools - Create actual Babylon.js objects
    else if (['cube', 'sphere', 'cylinder', 'plane'].includes(toolId)) {
      createBabylonPrimitive(toolId);
    }
    else if (toolId === 'light') {
      // Light dropdown will be handled separately
      return;
    }
    else if (toolId === 'camera') {
      createBabylonCamera();
    }
    // Handle duplicate and delete
    else if (toolId === 'duplicate') {
      if (!selectedEntity) {
        actions.editor.addConsoleMessage('Please select an object to duplicate', 'warning');
        return;
      }
      duplicateSelectedObject();
    }
    else if (toolId === 'delete') {
      if (!selectedEntity) {
        actions.editor.addConsoleMessage('Please select an object to delete', 'warning');
        return;
      }
      deleteSelectedObject();
    }
    // Handle other tools
    else {
      actions.editor.addConsoleMessage(`Tool activated: ${toolId}`, 'info');
    }
  };

  const handleLightCreate = (lightType) => {
    createBabylonLight(lightType);
    setShowLightDropdown(false);
    setLightDropdownPosition(null);
  };

  const handleLightDropdownToggle = (e) => {
    if (showLightDropdown) {
      setShowLightDropdown(false);
      setLightDropdownPosition(null);
    } else {
      const rect = e.currentTarget.getBoundingClientRect();
      const dropdownWidth = 192; // w-48 = 192px
      setLightDropdownPosition({
        left: rect.left + (rect.width / 2) - (dropdownWidth / 2),
        top: rect.bottom + 4
      });
      setShowLightDropdown(true);
    }
  };

  // Helper function to get current Babylon scene
  const getCurrentScene = () => {
    return babylonScene.current;
  };


  // Helper function to get simple object name
  const getObjectName = (type) => {
    return type.toLowerCase();
  };

  // Helper function to calculate viewport center position on ground
  const getViewportCenterPosition = (scene, distance = 5) => { // Reduced from 10 to 5 for smaller floor
    if (!scene || !scene._camera) {
      console.log('No scene or camera, using fallback position');
      return new BABYLON.Vector3(0, 1, 0); // Fallback position
    }

    const camera = scene._camera;
    
    try {
      // Get camera's forward direction
      const forward = camera.getForwardRay().direction.normalize();
      
      // Calculate position in front of camera at specified distance
      const viewportCenter = camera.position.add(forward.scale(distance));
      
      // Project this position down to the ground plane (Y = 0)
      const groundPosition = new BABYLON.Vector3(viewportCenter.x, 0, viewportCenter.z);
      
      // Cast a ray downward from above to find the actual ground or surface
      const rayOrigin = new BABYLON.Vector3(groundPosition.x, 100, groundPosition.z);
      const rayDirection = new BABYLON.Vector3(0, -1, 0);
      const ray = new BABYLON.Ray(rayOrigin, rayDirection);
      
      const hit = scene.pickWithRay(ray, (mesh) => {
        // Hit ground or other objects that can support the new object
        return mesh.name === 'ground' || (mesh.material && mesh.isPickable !== false && mesh.isVisible);
      });
      
      let finalY = 1; // Default height above ground
      
      if (hit.hit && hit.pickedPoint) {
        // Place object on the surface that was hit
        finalY = hit.pickedPoint.y + 1;
        console.log('Found surface at Y:', hit.pickedPoint.y, 'placing object at Y:', finalY);
      } else {
        // No surface found, use default ground level
        console.log('No surface found, using default ground level');
      }
      
      const finalPosition = new BABYLON.Vector3(groundPosition.x, finalY, groundPosition.z);
      
      console.log('Calculated ground position:', finalPosition, 'from viewport center at distance:', distance);
      return finalPosition;
    } catch (error) {
      console.error('Error calculating viewport center:', error);
      return new BABYLON.Vector3(0, 1, 0); // Fallback position
    }
  };

  // Create Babylon.js primitives
  const createBabylonPrimitive = (type) => {
    console.log('Creating primitive:', type);
    const scene = getCurrentScene();
    console.log('Current scene:', scene);
    
    if (!scene) {
      console.error('No scene available');
      actions.editor.addConsoleMessage('No active scene available', 'error');
      return;
    }

    const position = getViewportCenterPosition(scene); // Place at viewport center
    const objectName = getObjectName(type);
    
    try {
      // Create a TransformNode as the main container (consistent with imported models)
      const mainContainer = new BABYLON.TransformNode(objectName, scene);
      mainContainer.position = position;
      
      // Create the actual mesh as a child
      let mesh;
      const meshName = `${objectName}_mesh`;
      
      switch (type) {
        case 'cube':
          mesh = BABYLON.MeshBuilder.CreateBox(meshName, { size: 2 }, scene);
          break;
        case 'sphere':
          mesh = BABYLON.MeshBuilder.CreateSphere(meshName, { diameter: 2 }, scene);
          break;
        case 'cylinder':
          mesh = BABYLON.MeshBuilder.CreateCylinder(meshName, { height: 2, diameter: 2 }, scene);
          break;
        case 'plane':
          mesh = BABYLON.MeshBuilder.CreatePlane(meshName, { size: 2 }, scene);
          break;
      }
      
      if (mesh) {
        // Parent the mesh to the container
        mesh.parent = mainContainer;
        mesh.position = BABYLON.Vector3.Zero(); // Relative to parent
        
        // Create default material
        const material = new BABYLON.StandardMaterial(`${objectName}_material`, scene);
        material.diffuseColor = new BABYLON.Color3(0.7, 0.7, 0.9);
        material.specularColor = new BABYLON.Color3(0.2, 0.2, 0.2);
        mesh.material = material;
        
        // Apply current render mode to new object
        if (scene._applyRenderMode) {
          const currentRenderMode = globalStore.editor.viewport.renderMode || 'solid';
          if (currentRenderMode === 'wireframe') {
            material.wireframe = true;
          }
        }
        
        // Mark the mesh as internal (similar to imported models)
        mesh._isInternalMesh = true;
        
        // Select the main container (not the internal mesh)
        if (scene._gizmoManager) {
          scene._gizmoManager.attachToMesh(mainContainer);
          
          // Add highlight to child meshes (using the new highlighting logic)
          if (scene._highlightLayer) {
            scene._highlightLayer.removeAllMeshes();
            try {
              const childMeshes = mainContainer.getChildMeshes();
              childMeshes.forEach(childMesh => {
                if (childMesh.getClassName() === 'Mesh') {
                  scene._highlightLayer.addMesh(childMesh, BABYLON.Color3.Yellow());
                }
              });
            } catch (error) {
              console.warn('Could not add highlight to primitive:', error);
            }
          }
        }
        
        // Update store selection with container ID
        const objectId = mainContainer.uniqueId || mainContainer.name;
        actions.editor.setSelectedEntity(objectId);
        actions.editor.selectSceneObject(objectId);
        
        // Refresh scene data
        actions.editor.refreshSceneData();
        
        console.log('Primitive created successfully:', mainContainer.name, 'ID:', mainContainer.uniqueId);
        console.log('Container details:', {
          name: mainContainer.name,
          id: mainContainer.uniqueId,
          type: mainContainer.getClassName(),
          childMeshes: mainContainer.getChildMeshes().length
        });
        console.log('Position:', position);
        actions.editor.addConsoleMessage(`Created ${type} on ground`, 'success');
      }
    } catch (error) {
      console.error(`Error creating ${type}:`, error);
      actions.editor.addConsoleMessage(`Failed to create ${type}: ${error.message}`, 'error');
    }
  };

  // Create Babylon.js light
  const createBabylonLight = (lightType = 'directional') => {
    const scene = getCurrentScene();
    if (!scene) {
      actions.editor.addConsoleMessage('No active scene available', 'error');
      return;
    }

    try {
      const lightName = getObjectName('light');
      const lightPosition = getViewportCenterPosition(scene, 4); // Place on ground at viewport center
      lightPosition.y += 3; // Elevate the light above objects (4 units total above ground)
      
      // Create TransformNode as main container
      const mainContainer = new BABYLON.TransformNode(lightName, scene);
      mainContainer.position = lightPosition;
      
      // Create light based on type
      let light;
      switch (lightType) {
        case 'point':
          light = new BABYLON.PointLight(`${lightName}_light`, BABYLON.Vector3.Zero(), scene);
          light.diffuse = new BABYLON.Color3(1, 0.95, 0.8);
          light.specular = new BABYLON.Color3(1, 1, 1);
          light.intensity = 10;
          break;
        case 'spot':
          light = new BABYLON.SpotLight(`${lightName}_light`, BABYLON.Vector3.Zero(), new BABYLON.Vector3(0, -1, 0), Math.PI / 3, 2, scene);
          light.diffuse = new BABYLON.Color3(1, 0.95, 0.8);
          light.specular = new BABYLON.Color3(1, 1, 1);
          light.intensity = 15;
          break;
        case 'hemisphere':
          light = new BABYLON.HemisphericLight(`${lightName}_light`, new BABYLON.Vector3(0, 1, 0), scene);
          light.diffuse = new BABYLON.Color3(1, 0.95, 0.8);
          light.groundColor = new BABYLON.Color3(0.3, 0.3, 0.3);
          light.intensity = 0.7;
          break;
        default: // directional
          light = new BABYLON.DirectionalLight(`${lightName}_light`, new BABYLON.Vector3(-1, -1, -1), scene);
          light.diffuse = new BABYLON.Color3(1, 0.95, 0.8);
          light.specular = new BABYLON.Color3(1, 1, 1);
          light.intensity = 1;
          break;
      }
      
      light.position = BABYLON.Vector3.Zero(); // Relative to parent
      light.parent = mainContainer;
      
      // Create visual representation (helper) as child
      const lightHelper = BABYLON.MeshBuilder.CreateSphere(`${lightName}_helper`, { diameter: 0.5 }, scene);
      lightHelper.position = BABYLON.Vector3.Zero(); // Relative to parent
      lightHelper.parent = mainContainer;
      
      // Create light helper material
      const helperMaterial = new BABYLON.StandardMaterial(`${lightName}_helper_material`, scene);
      helperMaterial.emissiveColor = new BABYLON.Color3(1, 1, 0.8);
      helperMaterial.disableLighting = true;
      lightHelper.material = helperMaterial;
      
      // Mark helper as internal
      lightHelper._isInternalMesh = true;
      
      // Select the main container
      if (scene._gizmoManager) {
        scene._gizmoManager.attachToMesh(mainContainer);
        
        // Add highlight to child meshes
        if (scene._highlightLayer) {
          scene._highlightLayer.removeAllMeshes();
          try {
            const childMeshes = mainContainer.getChildMeshes();
            childMeshes.forEach(childMesh => {
              if (childMesh.getClassName() === 'Mesh') {
                scene._highlightLayer.addMesh(childMesh, BABYLON.Color3.Yellow());
              }
            });
          } catch (error) {
            console.warn('Could not add highlight to light:', error);
          }
        }
      }
      
      // Update store selection with container ID
      const objectId = mainContainer.uniqueId || mainContainer.name;
      actions.editor.setSelectedEntity(objectId);
      actions.editor.selectSceneObject(objectId);
      
      // Refresh scene data
      actions.editor.refreshSceneData();
      
      actions.editor.addConsoleMessage(`Created ${lightType} light`, 'success');
    } catch (error) {
      console.error('Error creating light:', error);
      actions.editor.addConsoleMessage(`Failed to create light: ${error.message}`, 'error');
    }
  };

  // Create Babylon.js camera
  const createBabylonCamera = () => {
    const scene = getCurrentScene();
    if (!scene) {
      actions.editor.addConsoleMessage('No active scene available', 'error');
      return;
    }

    try {
      const cameraName = getObjectName('camera');
      const cameraPosition = getViewportCenterPosition(scene, 6); // Place on ground at viewport center
      cameraPosition.y += 1.7; // Elevate camera to eye level (2.7 units total above ground ~= human height)
      
      // Create TransformNode as main container
      const mainContainer = new BABYLON.TransformNode(cameraName, scene);
      mainContainer.position = cameraPosition;
      
      // Create new camera as child
      const camera = new BABYLON.UniversalCamera(`${cameraName}_camera`, BABYLON.Vector3.Zero(), scene);
      camera.setTarget(new BABYLON.Vector3(0, 0, 1)); // Look forward relative to container
      camera.fov = Math.PI / 3; // 60 degrees
      camera.parent = mainContainer;
      
      // Create visual representation (helper) as child
      const cameraHelper = BABYLON.MeshBuilder.CreateBox(`${cameraName}_helper`, { width: 1, height: 0.6, depth: 1.5 }, scene);
      cameraHelper.position = BABYLON.Vector3.Zero(); // Relative to parent
      cameraHelper.parent = mainContainer;
      
      // Create camera helper material
      const helperMaterial = new BABYLON.StandardMaterial(`${cameraName}_helper_material`, scene);
      helperMaterial.diffuseColor = new BABYLON.Color3(0.2, 0.2, 0.8);
      helperMaterial.specularColor = new BABYLON.Color3(0.1, 0.1, 0.1);
      cameraHelper.material = helperMaterial;
      
      // Mark helper as internal
      cameraHelper._isInternalMesh = true;
      
      // Select the main container
      if (scene._gizmoManager) {
        scene._gizmoManager.attachToMesh(mainContainer);
        
        // Add highlight to child meshes
        if (scene._highlightLayer) {
          scene._highlightLayer.removeAllMeshes();
          try {
            const childMeshes = mainContainer.getChildMeshes();
            childMeshes.forEach(childMesh => {
              if (childMesh.getClassName() === 'Mesh') {
                scene._highlightLayer.addMesh(childMesh, BABYLON.Color3.Yellow());
              }
            });
          } catch (error) {
            console.warn('Could not add highlight to camera:', error);
          }
        }
      }
      
      // Update store selection with container ID
      const objectId = mainContainer.uniqueId || mainContainer.name;
      actions.editor.setSelectedEntity(objectId);
      actions.editor.selectSceneObject(objectId);
      
      // Refresh scene data
      actions.editor.refreshSceneData();
      
      actions.editor.addConsoleMessage('Created camera', 'success');
    } catch (error) {
      console.error('Error creating camera:', error);
      actions.editor.addConsoleMessage(`Failed to create camera: ${error.message}`, 'error');
    }
  };

  // Duplicate selected object
  const duplicateSelectedObject = () => {
    const scene = getCurrentScene();
    if (!scene || !scene._gizmoManager?.attachedMesh) {
      actions.editor.addConsoleMessage('No object selected to duplicate', 'warning');
      return;
    }

    const attachedMesh = scene._gizmoManager.attachedMesh;
    
    try {
      // Use the same duplication logic from the render plugin
      let newObject = attachedMesh.clone(attachedMesh.name + '_duplicate', null, false, true);
      
      if (newObject) {
        // Make sure the new object is at root level
        newObject.parent = null;
        
        // Offset position slightly so it doesn't overlap
        newObject.position = attachedMesh.position.add(new BABYLON.Vector3(2, 0, 2));
        if (attachedMesh.rotation && newObject.rotation) {
          newObject.rotation = attachedMesh.rotation.clone();
        }
        if (attachedMesh.scaling && newObject.scaling) {
          newObject.scaling = attachedMesh.scaling.clone();
        }
        
        // Select the new duplicated object
        const objectId = newObject.uniqueId || newObject.name;
        
        // Clear previous highlight
        if (scene._highlightLayer) {
          scene._highlightLayer.removeAllMeshes();
        }
        
        // Attach gizmo to new object
        scene._gizmoManager.attachToMesh(newObject);
        
        // Add yellow outline to new object
        if (scene._highlightLayer) {
          try {
            scene._highlightLayer.addMesh(newObject, BABYLON.Color3.Yellow());
          } catch (highlightError) {
            console.warn('Could not add highlight to duplicated object:', highlightError);
          }
        }
        
        // Update selection in store
        actions.editor.setSelectedEntity(objectId);
        actions.editor.selectSceneObject(objectId);
        
        // Refresh scene data
        setTimeout(() => {
          actions.editor.refreshSceneData();
        }, 100);
        
        actions.editor.addConsoleMessage('Object duplicated', 'success');
      }
    } catch (error) {
      console.error('Error duplicating object:', error);
      actions.editor.addConsoleMessage(`Failed to duplicate object: ${error.message}`, 'error');
    }
  };

  // Delete selected object
  const deleteSelectedObject = () => {
    const scene = getCurrentScene();
    if (!scene || !scene._gizmoManager?.attachedMesh) {
      actions.editor.addConsoleMessage('No object selected to delete', 'warning');
      return;
    }

    const attachedMesh = scene._gizmoManager.attachedMesh;
    
    if (attachedMesh.name === 'ground' || attachedMesh.name === 'skybox') {
      actions.editor.addConsoleMessage('Cannot delete default scene objects', 'warning');
      return;
    }
    
    try {
      // Delete the selected object
      attachedMesh.dispose();
      
      // Clear gizmo and highlight
      scene._gizmoManager.attachToMesh(null);
      if (scene._highlightLayer) {
        scene._highlightLayer.removeAllMeshes();
      }
      
      // Clear selection in store
      actions.editor.setSelectedEntity(null);
      actions.editor.selectSceneObject(null);
      
      // Refresh scene data
      actions.editor.refreshSceneData();
      
      actions.editor.addConsoleMessage('Object deleted', 'success');
    } catch (error) {
      console.error('Error deleting object:', error);
      actions.editor.addConsoleMessage(`Failed to delete object: ${error.message}`, 'error');
    }
  };

  return (
    <>
      <div className="relative w-full h-10 bg-gray-900/95 backdrop-blur-sm border-b border-gray-800 flex items-center">
        <div className="flex items-center h-full w-full px-4 gap-1">
          
          {/* All Tools in One Line */}
          {tools.map((tool, index) => {
            const effectiveSelectedTool = getEffectiveSelectedTool();
            const isActive = (effectiveSelectedTool === tool.id) || flashingTool === tool.id;
            const isDisabled = tool.requiresSelection && !selectedEntity;
            
            // Add dividers after certain tool groups
            const showDivider = (index === 3) || (index === 9); // After scale (transform tools) and after camera (create tools)
            
            return (
              <React.Fragment key={tool.id}>
                {tool.isDropdown && tool.id === 'light' ? (
                  <button
                    onClick={handleLightDropdownToggle}
                    className={`w-8 h-8 flex items-center justify-center rounded transition-all relative group cursor-pointer ${
                      isActive
                        ? 'bg-blue-600/90 text-white' 
                        : 'text-gray-400 hover:text-gray-200 hover:bg-slate-800'
                    }`}
                  >
                    <tool.icon className="w-4 h-4" />
                    <svg className="w-2 h-2 ml-1" fill="currentColor" viewBox="0 0 20 20">
                      <path fillRule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clipRule="evenodd" />
                    </svg>
                    
                    {/* Tooltip */}
                    <div className="absolute bottom-full mb-2 left-1/2 transform -translate-x-1/2 bg-gray-900/95 text-white text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                      {tool.tooltip}
                      <div className="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-gray-900/95" />
                    </div>
                  </button>
                ) : (
                  <button
                    onClick={() => isDisabled ? null : handleToolClick(tool.id)}
                    className={`w-8 h-8 flex items-center justify-center rounded transition-all relative group ${
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
                    <tool.icon className="w-4 h-4" />
                    
                    {/* Tooltip - only show for enabled buttons */}
                    {!isDisabled && (
                      <div className="absolute bottom-full mb-2 left-1/2 transform -translate-x-1/2 bg-gray-900/95 text-white text-xs px-2 py-1 rounded whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                        {tool.tooltip}
                        <div className="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-gray-900/95" />
                      </div>
                    )}
                  </button>
                )}
                
                {/* Divider */}
                {showDivider && (
                  <div className="w-px h-6 bg-gray-700 mx-1"></div>
                )}
              </React.Fragment>
            );
          })}

          {/* Spacer */}
          <div className="flex-1" />

          {/* Right side quick actions */}
          <div className="flex items-center">
            {/* Camera Settings - Only show for 3D workflows */}
            {(currentWorkflow === '3d-viewport' || currentWorkflow === 'material-editor') && (
              <CameraHelpers />
            )}
            
            {/* Grid and Snap controls - Only show for 3D workflows */}
            {(currentWorkflow === '3d-viewport' || currentWorkflow === 'material-editor') && (
              <GridHelpers />
            )}
          </div>
        </div>
      </div>
      
      {/* Light Dropdown - Fixed positioned */}
      {showLightDropdown && lightDropdownPosition && (
        <>
          {/* Backdrop */}
          <div 
            className="fixed inset-0 z-[200]" 
            onClick={() => {
              setShowLightDropdown(false);
              setLightDropdownPosition(null);
            }}
          />
          {/* Dropdown panel */}
          <div 
            className="fixed w-48 bg-gray-800/95 backdrop-blur-sm rounded-lg shadow-xl border border-gray-600/50 z-[210]"
            style={{
              left: lightDropdownPosition.left,
              top: lightDropdownPosition.top
            }}
          >
            {lightTypes.map((lightType) => (
              <button
                key={lightType.id}
                onClick={() => handleLightCreate(lightType.id)}
                className="w-full px-3 py-2 text-left text-sm transition-colors flex items-center gap-2 first:rounded-t-lg last:rounded-b-lg text-gray-300 hover:bg-gray-900/60 hover:text-white"
              >
                <lightType.icon className="w-4 h-4" />
                {lightType.label}
              </button>
            ))}
          </div>
        </>
      )}

      {/* Project Manager Modal */}
      {showProjectManager && (
        <ProjectManager
          onProjectLoad={(name, path) => {
            console.log(`Project loaded: ${name} at ${path}`)
            actions.editor.addConsoleMessage(`Project "${name}" loaded successfully`, 'success')
          }}
          onClose={() => setShowProjectManager(false)}
        />
      )}
    </>
  );
}

export default HorizontalToolbar;