import { onMount, onCleanup, createEffect } from 'solid-js';
import { GizmoManager } from '@babylonjs/core/Gizmos/gizmoManager';
import { UtilityLayerRenderer } from '@babylonjs/core/Rendering/utilityLayerRenderer';
import { renderStore, renderActions } from '../store.jsx';

// Standard Babylon.js Gizmo Component
export function GizmoManagerComponent() {
  let gizmoManager = null;
  let utilityLayer;

  onMount(() => {
    const scene = renderStore.scene;
    if (!scene) return;

    initializeGizmoManager(scene);
  });

  onCleanup(() => {
    cleanup();
  });

  // Watch for scene changes to reinitialize gizmo
  createEffect(() => {
    const scene = renderStore.scene;
    if (scene && !gizmoManager) {
      initializeGizmoManager(scene);
    }
  });

  // Watch for selected object changes
  createEffect(() => {
    const selectedObject = renderStore.selectedObject;
    const transformMode = renderStore.transformMode;
    
    if (gizmoManager) {
      gizmoManager.attachToMesh(selectedObject);
      updateGizmoMode(transformMode);
    }
  });

  const initializeGizmoManager = (scene) => {
    try {
      console.log('🎯 Creating standard Babylon.js gizmo manager');

      // Create utility layer for gizmos
      utilityLayer = UtilityLayerRenderer.DefaultUtilityLayer;

      // Create the standard gizmo manager - bit thicker
      gizmoManager = new GizmoManager(scene, 2, utilityLayer); // thickness = 2 (slightly thicker)
      
      // Set up gizmo properties
      gizmoManager.positionGizmoEnabled = false;
      gizmoManager.rotationGizmoEnabled = false;
      gizmoManager.scaleGizmoEnabled = false;
      gizmoManager.boundingBoxGizmoEnabled = false;
      
      // Store in render store
      renderActions.setGizmoManager(gizmoManager);

      console.log('✅ Standard gizmo manager created - smaller size');

    } catch (error) {
      console.error('❌ Failed to create gizmo manager:', error);
    }
  };

  const updateGizmoMode = (mode) => {
    if (!gizmoManager) return;
    
    // Disable all gizmos first
    gizmoManager.positionGizmoEnabled = false;
    gizmoManager.rotationGizmoEnabled = false;
    gizmoManager.scaleGizmoEnabled = false;
    gizmoManager.boundingBoxGizmoEnabled = false;
    
    // Enable appropriate gizmo based on mode - smaller size
    switch (mode) {
      case 'move':
        gizmoManager.positionGizmoEnabled = true;
        // Set scale ratio after enabling (bit longer)
        if (gizmoManager.gizmos.positionGizmo) {
          gizmoManager.gizmos.positionGizmo.scaleRatio = 1.8; // Bit longer than 1.5
        }
        break;
      case 'rotate':
        gizmoManager.rotationGizmoEnabled = true;
        // Set scale ratio after enabling
        if (gizmoManager.gizmos.rotationGizmo) {
          gizmoManager.gizmos.rotationGizmo.scaleRatio = 1.8;
        }
        break;
      case 'scale':
        gizmoManager.scaleGizmoEnabled = true;
        // Set scale ratio after enabling
        if (gizmoManager.gizmos.scaleGizmo) {
          gizmoManager.gizmos.scaleGizmo.scaleRatio = 1.8;
          
          // For plane objects, constrain Y-axis scaling by monitoring the gizmo
          const selectedObject = renderStore.selectedObject;
          if (selectedObject && selectedObject.name && selectedObject.name.toLowerCase().includes('plane')) {
            console.log('🎯 Plane detected - setting up Y-axis scaling constraint');
            
            // Monitor scaling changes and override Y-axis
            const scaleGizmo = gizmoManager.gizmos.scaleGizmo;
            if (scaleGizmo && !scaleGizmo._planeConstraintAdded) {
              const originalOnDragObservable = scaleGizmo.onDragStartObservable.clone();
              let initialScale = null;
              
              scaleGizmo.onDragStartObservable.add(() => {
                initialScale = selectedObject.scaling.clone();
              });
              
              scaleGizmo.onDragObservable.add(() => {
                if (initialScale && selectedObject.scaling.y !== initialScale.y) {
                  selectedObject.scaling.y = initialScale.y;
                }
              });
              
              scaleGizmo._planeConstraintAdded = true;
            }
          }
        }
        break;
      default:
        // No gizmos enabled for select mode
        break;
    }
  };

  const cleanup = () => {
    if (gizmoManager) {
      gizmoManager.dispose();
      gizmoManager = null;
    }
    
    if (utilityLayer && utilityLayer !== UtilityLayerRenderer.DefaultUtilityLayer) {
      utilityLayer.dispose();
      utilityLayer = null;
    }
  };


  return null; // This component doesn't render anything visible
}