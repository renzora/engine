import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';
import { ArcRotateCamera } from '@babylonjs/core/Cameras/arcRotateCamera.js';
import { UniversalCamera } from '@babylonjs/core/Cameras/universalCamera.js';

/**
 * CameraAPI - Camera control and manipulation
 */
export class CameraAPI {
  constructor(scene, babylonObject) {
    this.scene = scene;
    this.babylonObject = babylonObject;
  }

  // === CAMERA CONTROL ===
  
  getActiveCamera() {
    return this.scene.activeCamera;
  }
  
  setCameraPosition(x, y, z) {
    const camera = this.scene.activeCamera;
    if (camera) {
      camera.position.set(x, y, z);
    }
  }
  
  getCameraPosition() {
    const camera = this.scene.activeCamera;
    if (!camera) return [0, 0, 0];
    return [camera.position.x, camera.position.y, camera.position.z];
  }
  
  setCameraTarget(x, y, z) {
    const camera = this.scene.activeCamera;
    const target = new Vector3(x, y, z);
    
    if (camera && camera.setTarget) {
      // ArcRotateCamera - use native setTarget
      camera.setTarget(target);
    } else if (camera) {
      // UniversalCamera - store target and maintain radius distance
      camera._arcTarget = target;
      
      // If we have a stored radius, maintain that distance
      const radius = camera._arcRadius || 10.0;
      const currentPos = camera.position;
      const direction = currentPos.subtract(target).normalize();
      camera.position = target.add(direction.scale(radius));
      
      // Make camera look at target
      camera.setTarget(target);
    }
  }
  
  getCameraTarget() {
    const camera = this.scene.activeCamera;
    if (!camera) return [0, 0, 0];
    
    if (camera.getTarget) {
      // ArcRotateCamera - use native getTarget
      const target = camera.getTarget();
      return [target.x, target.y, target.z];
    } else if (camera._arcTarget) {
      // UniversalCamera - return stored target
      return [camera._arcTarget.x, camera._arcTarget.y, camera._arcTarget.z];
    }
    
    return [0, 0, 0];
  }
  
  setCameraRotation(x, y, z) {
    const camera = this.scene.activeCamera;
    if (camera) {
      camera.rotation.set(x, y, z);
    }
  }
  
  getCameraRotation() {
    const camera = this.scene.activeCamera;
    if (!camera) return [0, 0, 0];
    return [camera.rotation.x, camera.rotation.y, camera.rotation.z];
  }
  
  setCameraFOV(fov) {
    const camera = this.scene.activeCamera;
    if (camera && camera.fov !== undefined) {
      camera.fov = fov;
    }
  }
  
  getCameraFOV() {
    const camera = this.scene.activeCamera;
    if (!camera) return 0.8;
    return camera.fov || 0.8;
  }
  
  setCameraRadius(radius) {
    const camera = this.scene.activeCamera;
    if (camera && camera.radius !== undefined) {
      // ArcRotateCamera - use native radius
      camera.radius = radius;
    } else if (camera) {
      // UniversalCamera - simulate radius by positioning camera at distance from target
      const target = camera._arcTarget || Vector3.Zero();
      const currentPos = camera.position;
      const direction = currentPos.subtract(target).normalize();
      camera.position = target.add(direction.scale(radius));
      camera._arcRadius = radius; // Store for future use
      
      // Make sure camera is looking at the target
      camera.setTarget(target);
    }
  }
  
  getCameraRadius() {
    const camera = this.scene.activeCamera;
    if (!camera) return 10.0;
    if (camera.radius !== undefined) {
      // ArcRotateCamera - use native radius
      return camera.radius;
    }
    // UniversalCamera - return stored radius or calculate from target
    if (camera._arcRadius !== undefined) {
      return camera._arcRadius;
    }
    const target = camera._arcTarget || Vector3.Zero();
    return Vector3.Distance(camera.position, target);
  }
  
  orbitCamera(speed, direction = 1) {
    const camera = this.scene.activeCamera;
    if (!camera) return;
    
    // Get current time for smooth orbiting
    const currentTime = Date.now() / 1000; // Convert to seconds
    
    if (camera.alpha !== undefined && camera.beta !== undefined) {
      // ArcRotateCamera - use native properties with time-based calculation
      if (!camera._orbitStartTime) camera._orbitStartTime = currentTime;
      const elapsed = currentTime - camera._orbitStartTime;
      camera.alpha = elapsed * speed * direction;
    } else {
      // UniversalCamera - simulate orbit by calculating position
      const target = camera._arcTarget || Vector3.Zero();
      const currentRadius = camera._arcRadius || 10.0;
      
      // Calculate orbit angle based on time and speed
      if (!camera._orbitStartTime) camera._orbitStartTime = currentTime;
      const elapsed = currentTime - camera._orbitStartTime;
      const alpha = elapsed * speed * direction;
      const beta = Math.PI / 3; // Fixed elevation angle
      
      // Calculate new position using spherical coordinates
      const x = target.x + currentRadius * Math.sin(beta) * Math.cos(alpha);
      const y = target.y + currentRadius * Math.cos(beta);
      const z = target.z + currentRadius * Math.sin(beta) * Math.sin(alpha);
      
      camera.position.set(x, y, z);
      camera.setTarget(target);
    }
  }
  
  setCameraType(type) {
    const canvas = this.scene.getEngine().getRenderingCanvas();
    
    if (type === 'arc' || type === 'arcrotate') {
      // Disable UniversalCamera movement system for ArcRotateCamera mode
      if (canvas && canvas._cameraMovementController) {
        canvas._cameraMovementController.disable();
        // Disabled UniversalCamera movement system for ArcRotateCamera mode
      }
    } else if (type === 'universal' || type === 'free') {
      // Re-enable UniversalCamera movement system
      if (canvas && canvas._cameraMovementController) {
        canvas._cameraMovementController.enable();
        // Enabled UniversalCamera movement system
      }
    } else {
      console.warn('Unknown camera type:', type);
      return;
    }
    
    console.log(`Camera mode changed to: ${type}`);
  }
  
  // === SHORT NAME ALIASES ===
  
  activeCamera() {
    return this.getActiveCamera();
  }
  
  cameraPosition() {
    return this.getCameraPosition();
  }
  
  cameraTarget() {
    return this.getCameraTarget();
  }
  
  cameraRotation() {
    return this.getCameraRotation();
  }
  
  cameraFOV() {
    return this.getCameraFOV();
  }
  
  cameraRadius() {
    return this.getCameraRadius();
  }
}