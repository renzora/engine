import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';

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
    if (camera && camera.setTarget) {
      camera.setTarget(new Vector3(x, y, z));
    }
  }
  
  getCameraTarget() {
    const camera = this.scene.activeCamera;
    if (!camera || !camera.getTarget) return [0, 0, 0];
    const target = camera.getTarget();
    return [target.x, target.y, target.z];
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
}