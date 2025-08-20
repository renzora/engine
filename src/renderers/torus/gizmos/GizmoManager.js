import { RaycastUtils } from '../math/RaycastUtils.js';

/**
 * Gizmo Manager - Handles 3D manipulation gizmos for Torus renderer
 */
export class GizmoManager {
  constructor(renderer) {
    this.renderer = renderer;
    this.selectedMesh = null;
    this.gizmoMeshes = [];
    this.isDragging = false;
    this.dragAxis = null;
    this.dragStartPos = { x: 0, y: 0 };
    this.dragStartWorldPos = [0, 0, 0];
    this.gizmoSize = 1.0;
    this.hoveredAxis = null;
    
    // Gizmo colors
    this.colors = {
      x: { r: 1, g: 0.3, b: 0.3 }, // Red
      y: { r: 0.3, g: 1, b: 0.3 }, // Green  
      z: { r: 0.3, g: 0.3, b: 1 }, // Blue
      xHighlight: { r: 1, g: 0.8, b: 0.3 }, // Orange-yellow
      yHighlight: { r: 0.8, g: 1, b: 0.3 }, // Yellow-green
      zHighlight: { r: 0.3, g: 0.8, b: 1 }  // Light blue
    };
  }

  selectMesh(mesh) {
    this.selectedMesh = mesh;
    this.createGizmo();
  }

  deselectMesh() {
    this.selectedMesh = null;
    this.hoveredAxis = null;
    this.destroyGizmo();
  }

  createGizmo() {
    if (!this.selectedMesh) return;
    
    this.destroyGizmo(); // Clear existing gizmo
    
    const pos = this.selectedMesh.position;
    
    // Professional gizmo dimensions (Unreal Engine style)
    const shaftRadius = 0.015;
    const shaftLength = 1.2;
    const headRadius = 0.08;
    const headLength = 0.25;
    
    // X-axis arrow (red) - pointing right
    const xShaft = this.renderer.createPrimitive('cylinder', {
      position: { x: pos.x + shaftLength/2, y: pos.y, z: pos.z },
      rotation: { x: 0, y: 0, z: -Math.PI/2 }, // Rotate 90° around Z to point along X
      radiusTop: shaftRadius,
      radiusBottom: shaftRadius,
      height: shaftLength,
      radialSegments: 8,
      color: this.colors.x
    });
    
    // X-axis arrowhead (proper cone)
    const xHead = this.renderer.createPrimitive('cone', {
      position: { x: pos.x + shaftLength + headLength/2, y: pos.y, z: pos.z },
      rotation: { x: 0, y: 0, z: Math.PI/2 }, // Rotate 90° around Z to point along X (flipped)
      radius: headRadius,
      height: headLength,
      radialSegments: 12,
      color: this.colors.x
    });
    
    // Y-axis arrow (green) - pointing up (default orientation)
    const yShaft = this.renderer.createPrimitive('cylinder', {
      position: { x: pos.x, y: pos.y + shaftLength/2, z: pos.z },
      rotation: { x: 0, y: 0, z: 0 }, // No rotation needed - Y is default up
      radiusTop: shaftRadius,
      radiusBottom: shaftRadius,
      height: shaftLength,
      radialSegments: 8,
      color: this.colors.y
    });
    
    const yHead = this.renderer.createPrimitive('cone', {
      position: { x: pos.x, y: pos.y + shaftLength + headLength/2, z: pos.z },
      rotation: { x: 0, y: 0, z: 0 }, // No rotation needed - Y is default up
      radius: headRadius,
      height: headLength,
      radialSegments: 12,
      color: this.colors.y
    });
    
    // Z-axis arrow (blue) - pointing forward
    const zShaft = this.renderer.createPrimitive('cylinder', {
      position: { x: pos.x, y: pos.y, z: pos.z + shaftLength/2 },
      rotation: { x: Math.PI/2, y: 0, z: 0 }, // Rotate 90° around X to point along Z
      radiusTop: shaftRadius,
      radiusBottom: shaftRadius,
      height: shaftLength,
      radialSegments: 8,
      color: this.colors.z
    });
    
    const zHead = this.renderer.createPrimitive('cone', {
      position: { x: pos.x, y: pos.y, z: pos.z + shaftLength + headLength/2 },
      rotation: { x: -Math.PI/2, y: 0, z: 0 }, // Rotate -90° around X to point along Z (flipped)
      radius: headRadius,
      height: headLength,
      radialSegments: 12,
      color: this.colors.z
    });
    
    // Store gizmo parts with axis info
    this.gizmoMeshes = [
      { id: xShaft, axis: 'x', part: 'shaft' },
      { id: xHead, axis: 'x', part: 'head' },
      { id: yShaft, axis: 'y', part: 'shaft' },
      { id: yHead, axis: 'y', part: 'head' },
      { id: zShaft, axis: 'z', part: 'shaft' },
      { id: zHead, axis: 'z', part: 'head' }
    ];
    
    console.log('[Gizmo] Created professional gizmo for mesh:', this.selectedMesh.id);
  }

  destroyGizmo() {
    // Remove gizmo meshes from render queue
    if (this.gizmoMeshes.length > 0) {
      const renderQueue = this.renderer.getRenderQueue();
      this.gizmoMeshes.forEach(gizmo => {
        const index = renderQueue.findIndex(mesh => mesh.id === gizmo.id);
        if (index !== -1) {
          renderQueue.splice(index, 1);
        }
      });
      this.gizmoMeshes = [];
    }
  }

  updateGizmoPosition() {
    if (!this.selectedMesh || this.gizmoMeshes.length === 0) return;
    
    const pos = this.selectedMesh.position;
    const renderQueue = this.renderer.getRenderQueue();
    
    // Professional gizmo dimensions
    const shaftLength = 1.2;
    const headLength = 0.25;
    
    // Update positions of all gizmo parts
    this.gizmoMeshes.forEach(gizmo => {
      const mesh = renderQueue.find(m => m.id === gizmo.id);
      if (mesh) {
        if (gizmo.axis === 'x') {
          if (gizmo.part === 'shaft') {
            mesh.position.x = pos.x + shaftLength/2;
            mesh.position.y = pos.y;
            mesh.position.z = pos.z;
          } else if (gizmo.part === 'head') {
            mesh.position.x = pos.x + shaftLength + headLength/2;
            mesh.position.y = pos.y;
            mesh.position.z = pos.z;
          }
        } else if (gizmo.axis === 'y') {
          if (gizmo.part === 'shaft') {
            mesh.position.x = pos.x;
            mesh.position.y = pos.y + shaftLength/2;
            mesh.position.z = pos.z;
          } else if (gizmo.part === 'head') {
            mesh.position.x = pos.x;
            mesh.position.y = pos.y + shaftLength + headLength/2;
            mesh.position.z = pos.z;
          }
        } else if (gizmo.axis === 'z') {
          if (gizmo.part === 'shaft') {
            mesh.position.x = pos.x;
            mesh.position.y = pos.y;
            mesh.position.z = pos.z + shaftLength/2;
          } else if (gizmo.part === 'head') {
            mesh.position.x = pos.x;
            mesh.position.y = pos.y;
            mesh.position.z = pos.z + shaftLength + headLength/2;
          }
        }
      }
    });
  }

  updateGizmoColors() {
    if (!this.selectedMesh || this.gizmoMeshes.length === 0) return;
    
    const renderQueue = this.renderer.getRenderQueue();
    
    this.gizmoMeshes.forEach(gizmo => {
      const mesh = renderQueue.find(m => m.id === gizmo.id);
      if (mesh) {
        const isHovered = this.hoveredAxis === gizmo.axis;
        const isDragging = this.isDragging && this.dragAxis === gizmo.axis;
        
        if (isDragging || isHovered) {
          // Use highlight color
          mesh.color = this.colors[gizmo.axis + 'Highlight'];
        } else {
          // Use normal color
          mesh.color = this.colors[gizmo.axis];
        }
      }
    });
  }

  // Mouse interaction methods
  onMouseDown(event, camera) {
    if (!this.selectedMesh) return false;
    
    // Check if clicking on gizmo
    const clickedGizmo = this.getGizmoUnderMouse(event, camera);
    if (clickedGizmo) {
      this.isDragging = true;
      this.dragAxis = clickedGizmo.axis;
      this.dragStartPos = { x: event.clientX, y: event.clientY };
      this.dragStartWorldPos = [
        this.selectedMesh.position.x,
        this.selectedMesh.position.y, 
        this.selectedMesh.position.z
      ];
      this.updateGizmoColors(); // Update colors when starting drag
      console.log(`[Gizmo] Started dragging ${this.dragAxis} axis`);
      return true; // Consumed event
    }
    
    return false; // Didn't handle event
  }

  onMouseMove(event, camera) {
    // Always check for hover (even when not dragging)
    if (!this.isDragging) {
      const hoveredGizmo = this.getGizmoUnderMouse(event, camera);
      const newHoveredAxis = hoveredGizmo ? hoveredGizmo.axis : null;
      
      if (newHoveredAxis !== this.hoveredAxis) {
        this.hoveredAxis = newHoveredAxis;
        this.updateGizmoColors();
      }
    }
    
    if (!this.isDragging || !this.dragAxis) return false;
    
    const deltaX = event.clientX - this.dragStartPos.x;
    const deltaY = event.clientY - this.dragStartPos.y;
    
    // Improved movement - proper direction and sensitivity
    const sensitivity = 0.005;
    
    if (this.dragAxis === 'x') {
      // X-axis: drag right = move right
      this.selectedMesh.position.x = this.dragStartWorldPos[0] + deltaX * sensitivity;
    } else if (this.dragAxis === 'y') {
      // Y-axis: drag up = move up
      this.selectedMesh.position.y = this.dragStartWorldPos[1] + deltaY * sensitivity;
    } else if (this.dragAxis === 'z') {
      // Z-axis: drag right = move forward (positive Z)
      this.selectedMesh.position.z = this.dragStartWorldPos[2] + deltaX * sensitivity;
    }
    
    // Update gizmo position to follow object
    this.updateGizmoPosition();
    
    return true; // Consumed event
  }

  onMouseUp(event) {
    if (this.isDragging) {
      console.log(`[Gizmo] Finished dragging ${this.dragAxis} axis`);
      this.isDragging = false;
      this.dragAxis = null;
      this.updateGizmoColors(); // Update colors when ending drag
      return true; // Consumed event
    }
    return false;
  }

  getGizmoUnderMouse(event, camera) {
    if (!this.selectedMesh || this.gizmoMeshes.length === 0) return null;
    
    const canvas = this.renderer.webgl.canvas;
    const viewMatrix = this.renderer.scene.getViewMatrix();
    const projectionMatrix = this.renderer.scene.getProjectionMatrix();
    
    // Create ray from mouse position
    const ray = RaycastUtils.createRayFromMouse(
      event.clientX, 
      event.clientY, 
      canvas, 
      viewMatrix, 
      projectionMatrix
    );
    
    let closestHit = null;
    let closestDistance = Infinity;
    
    // Test ray against each gizmo part
    this.gizmoMeshes.forEach(gizmo => {
      const renderQueue = this.renderer.getRenderQueue();
      const mesh = renderQueue.find(m => m.id === gizmo.id);
      if (!mesh) return;
      
      let distance = null;
      
      if (gizmo.part === 'head') {
        // Test against arrowhead (treat as sphere)
        distance = RaycastUtils.raySphereIntersection(
          ray, 
          [mesh.position.x, mesh.position.y, mesh.position.z], 
          0.12 // Slightly larger than visual size for easier picking
        );
      } else if (gizmo.part === 'shaft') {
        // Test against shaft (treat as AABB)
        const shaftRadius = 0.03; // Slightly larger for easier picking
        const shaftLength = 1.2;
        
        let aabb;
        if (gizmo.axis === 'x') {
          aabb = {
            min: [mesh.position.x - shaftLength/2, mesh.position.y - shaftRadius, mesh.position.z - shaftRadius],
            max: [mesh.position.x + shaftLength/2, mesh.position.y + shaftRadius, mesh.position.z + shaftRadius]
          };
        } else if (gizmo.axis === 'y') {
          aabb = {
            min: [mesh.position.x - shaftRadius, mesh.position.y - shaftLength/2, mesh.position.z - shaftRadius],
            max: [mesh.position.x + shaftRadius, mesh.position.y + shaftLength/2, mesh.position.z + shaftRadius]
          };
        } else if (gizmo.axis === 'z') {
          aabb = {
            min: [mesh.position.x - shaftRadius, mesh.position.y - shaftRadius, mesh.position.z - shaftLength/2],
            max: [mesh.position.x + shaftRadius, mesh.position.y + shaftRadius, mesh.position.z + shaftLength/2]
          };
        }
        
        if (aabb) {
          distance = RaycastUtils.rayAABBIntersection(ray, aabb.min, aabb.max);
        }
      }
      
      // Track closest hit
      if (distance !== null && distance < closestDistance) {
        closestDistance = distance;
        closestHit = { axis: gizmo.axis, part: gizmo.part };
      }
    });
    
    return closestHit;
  }

  dispose() {
    this.destroyGizmo();
    this.selectedMesh = null;
  }
}