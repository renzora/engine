// === XR/VR/AR API MODULE ===

import {
  WebXRDefaultExperience,
  WebXRFeatureName,
  WebXRFeaturesManager,
  WebXRControllerComponent,
  WebXRInputSource,
  WebXRCamera,
  Vector3,
  Quaternion,
  Matrix,
  Ray,
  AbstractMesh
} from '@babylonjs/core';

export class XRAPI {
  constructor(scene) {
    this.scene = scene;
    this.xrExperience = null;
    this.controllers = new Map();
  }

  // === XR SETUP ===

  async initializeXR(options = {}) {
    try {
      this.xrExperience = await this.scene.createDefaultXRExperienceAsync({
        floorMeshes: options.floorMeshes || [],
        disablePointerSelection: options.disablePointerSelection || false,
        disableNearInteraction: options.disableNearInteraction || false,
        disableTeleportation: options.disableTeleportation || false,
        uiOptions: {
          sessionMode: options.sessionMode || 'immersive-vr',
          referenceSpaceType: options.referenceSpaceType || 'local-floor'
        },
        ...options
      });
      
      return this.xrExperience;
    } catch (error) {
      console.error('Failed to initialize XR:', error);
      return null;
    }
  }

  async enterVR() {
    if (!this.xrExperience) {
      await this.initializeXR({ sessionMode: 'immersive-vr' });
    }
    
    if (this.xrExperience && this.xrExperience.baseExperience) {
      return await this.xrExperience.baseExperience.enterXRAsync('immersive-vr', 'local-floor');
    }
    return false;
  }

  async enterAR() {
    if (!this.xrExperience) {
      await this.initializeXR({ sessionMode: 'immersive-ar' });
    }
    
    if (this.xrExperience && this.xrExperience.baseExperience) {
      return await this.xrExperience.baseExperience.enterXRAsync('immersive-ar', 'local-floor');
    }
    return false;
  }

  exitXR() {
    if (this.xrExperience && this.xrExperience.baseExperience) {
      this.xrExperience.baseExperience.exitXRAsync();
      return true;
    }
    return false;
  }

  isInXR() {
    return this.xrExperience && this.xrExperience.baseExperience && this.xrExperience.baseExperience.state === 'IN_XR';
  }

  // === CONTROLLER TRACKING ===

  getControllerPosition(controllerId = 0) {
    if (!this.xrExperience || !this.xrExperience.input) return [0, 0, 0];
    
    const controllers = this.xrExperience.input.controllers;
    if (controllers[controllerId] && controllers[controllerId].grip) {
      const pos = controllers[controllerId].grip.position;
      return [pos.x, pos.y, pos.z];
    }
    return [0, 0, 0];
  }

  getControllerRotation(controllerId = 0) {
    if (!this.xrExperience || !this.xrExperience.input) return [0, 0, 0, 1];
    
    const controllers = this.xrExperience.input.controllers;
    if (controllers[controllerId] && controllers[controllerId].grip) {
      const rot = controllers[controllerId].grip.rotationQuaternion;
      return [rot.x, rot.y, rot.z, rot.w];
    }
    return [0, 0, 0, 1];
  }

  isControllerConnected(controllerId = 0) {
    if (!this.xrExperience || !this.xrExperience.input) return false;
    
    const controllers = this.xrExperience.input.controllers;
    return controllers[controllerId] && controllers[controllerId].inputSource;
  }

  getControllerButtonState(controllerId = 0, buttonIndex = 0) {
    if (!this.xrExperience || !this.xrExperience.input) return false;
    
    const controllers = this.xrExperience.input.controllers;
    const controller = controllers[controllerId];
    
    if (controller && controller.motionController && controller.motionController.components) {
      const components = Object.values(controller.motionController.components);
      const button = components[buttonIndex];
      return button && button.pressed;
    }
    return false;
  }

  getControllerTriggerValue(controllerId = 0) {
    if (!this.xrExperience || !this.xrExperience.input) return 0;
    
    const controllers = this.xrExperience.input.controllers;
    const controller = controllers[controllerId];
    
    if (controller && controller.motionController) {
      const trigger = controller.motionController.getComponent('xr-standard-trigger');
      return trigger ? (trigger.value || 0) : 0;
    }
    return 0;
  }

  getControllerThumbstick(controllerId = 0) {
    if (!this.xrExperience || !this.xrExperience.input) return [0, 0];
    
    const controllers = this.xrExperience.input.controllers;
    const controller = controllers[controllerId];
    
    if (controller && controller.motionController) {
      const thumbstick = controller.motionController.getComponent('xr-standard-thumbstick');
      if (thumbstick && thumbstick.axes) {
        return [thumbstick.axes.x || 0, thumbstick.axes.y || 0];
      }
    }
    return [0, 0];
  }

  // === HEAD TRACKING ===

  getHeadPosition() {
    if (!this.xrExperience || !this.xrExperience.camera) return [0, 1.6, 0];
    
    const pos = this.xrExperience.camera.position;
    return [pos.x, pos.y, pos.z];
  }

  getHeadRotation() {
    if (!this.xrExperience || !this.xrExperience.camera) return [0, 0, 0, 1];
    
    const rot = this.xrExperience.camera.rotationQuaternion;
    return [rot.x, rot.y, rot.z, rot.w];
  }

  getHeadForward() {
    if (!this.xrExperience || !this.xrExperience.camera) return [0, 0, -1];
    
    const forward = this.xrExperience.camera.getForwardRay().direction;
    return [forward.x, forward.y, forward.z];
  }

  // === XR INTERACTION ===

  enableHandTracking() {
    if (!this.xrExperience) return false;
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const handTracking = features.enableFeature(WebXRFeatureName.HAND_TRACKING, 'latest');
    return !!handTracking;
  }

  enableEyeTracking() {
    if (!this.xrExperience) return false;
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const eyeTracking = features.enableFeature(WebXRFeatureName.EYE_TRACKING, 'latest');
    return !!eyeTracking;
  }

  enablePlaneDetection() {
    if (!this.xrExperience) return false;
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const planeDetection = features.enableFeature(WebXRFeatureName.PLANE_DETECTION, 'latest');
    return !!planeDetection;
  }

  enableAnchorSystem() {
    if (!this.xrExperience) return false;
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const anchors = features.enableFeature(WebXRFeatureName.ANCHOR_SYSTEM, 'latest');
    return !!anchors;
  }

  // === XR TELEPORTATION ===

  setTeleportationFloor(meshes) {
    if (!this.xrExperience || !this.xrExperience.teleportation) return false;
    
    if (Array.isArray(meshes)) {
      this.xrExperience.teleportation.addFloorMeshes(meshes);
    } else {
      this.xrExperience.teleportation.addFloorMesh(meshes);
    }
    return true;
  }

  setTeleportationBlocker(meshes) {
    if (!this.xrExperience || !this.xrExperience.teleportation) return false;
    
    if (Array.isArray(meshes)) {
      this.xrExperience.teleportation.addBlockerMeshes(meshes);
    } else {
      this.xrExperience.teleportation.addBlockerMesh(meshes);
    }
    return true;
  }

  teleportToPosition(position) {
    if (!this.xrExperience || !this.xrExperience.camera) return false;
    
    this.xrExperience.camera.position = new Vector3(...position);
    return true;
  }

  // === XR POINTER SELECTION ===

  enablePointerSelection(meshes = []) {
    if (!this.xrExperience) return false;
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const pointerSelection = features.enableFeature(WebXRFeatureName.POINTER_SELECTION, 'stable', {
      xrInput: this.xrExperience.input,
      enablePointerSelectionOnAllControllers: true
    });
    
    if (meshes.length > 0) {
      meshes.forEach(mesh => {
        if (mesh.actionManager) {
          mesh.isPickable = true;
        }
      });
    }
    
    return !!pointerSelection;
  }

  getRayFromController(controllerId = 0) {
    if (!this.xrExperience || !this.xrExperience.input) return null;
    
    const controllers = this.xrExperience.input.controllers;
    const controller = controllers[controllerId];
    
    if (controller && controller.pointer) {
      const origin = controller.pointer.position;
      const direction = controller.pointer.forward;
      return {
        origin: [origin.x, origin.y, origin.z],
        direction: [direction.x, direction.y, direction.z]
      };
    }
    return null;
  }

  // === XR HAPTIC FEEDBACK ===

  triggerHapticFeedback(controllerId = 0, intensity = 1.0, duration = 100) {
    if (!this.xrExperience || !this.xrExperience.input) return false;
    
    const controllers = this.xrExperience.input.controllers;
    const controller = controllers[controllerId];
    
    if (controller && controller.motionController) {
      const haptic = controller.motionController.getComponent('haptic');
      if (haptic) {
        haptic.pulse(Math.max(0, Math.min(1, intensity)), Math.max(1, duration));
        return true;
      }
    }
    return false;
  }

  // === XR MOVEMENT ===

  enableWalkingLocomotion() {
    if (!this.xrExperience) return false;
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const movement = features.enableFeature(WebXRFeatureName.MOVEMENT, 'latest', {
      xrInput: this.xrExperience.input,
      movementEnabled: true,
      rotationEnabled: true
    });
    
    return !!movement;
  }

  setXRMovementSpeed(speed = 5.0) {
    if (!this.xrExperience) return false;
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const movement = features.getEnabledFeature(WebXRFeatureName.MOVEMENT);
    
    if (movement) {
      movement.movementSpeed = speed;
      return true;
    }
    return false;
  }

  setXRRotationSpeed(speed = 60) {
    if (!this.xrExperience) return false;
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const movement = features.getEnabledFeature(WebXRFeatureName.MOVEMENT);
    
    if (movement) {
      movement.rotationAngle = speed;
      return true;
    }
    return false;
  }

  // === XR UI ===

  createXRUI(width = 1024, height = 1024) {
    if (!this.scene) return null;
    
    // Create a 3D UI plane for XR
    const plane = MeshBuilder.CreatePlane('xr_ui_plane', { width: 2, height: 1.5 }, this.scene);
    
    const advancedTexture = AdvancedDynamicTexture.CreateForMesh(plane, width, height);
    advancedTexture.hasAlpha = true;
    
    return {
      plane,
      ui: advancedTexture,
      setPosition: (x, y, z) => {
        plane.position = new Vector3(x, y, z);
      },
      lookAtCamera: () => {
        if (this.xrExperience && this.xrExperience.camera) {
          plane.lookAt(this.xrExperience.camera.position);
        }
      }
    };
  }

  attachUIToController(uiPlane, controllerId = 0, offset = [0, 0, 0.5]) {
    if (!uiPlane || !this.xrExperience || !this.xrExperience.input) return false;
    
    const controllers = this.xrExperience.input.controllers;
    const controller = controllers[controllerId];
    
    if (controller && controller.grip) {
      uiPlane.parent = controller.grip;
      uiPlane.position = new Vector3(...offset);
      return true;
    }
    return false;
  }

  // === HAND TRACKING ===

  getHandJointPosition(hand = 'right', joint = 'wrist') {
    if (!this.xrExperience) return [0, 0, 0];
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const handTracking = features.getEnabledFeature(WebXRFeatureName.HAND_TRACKING);
    
    if (handTracking) {
      const handData = handTracking.getHandByHandedness(hand);
      if (handData && handData.joints[joint]) {
        const pos = handData.joints[joint].position;
        return [pos.x, pos.y, pos.z];
      }
    }
    return [0, 0, 0];
  }

  getHandJointRotation(hand = 'right', joint = 'wrist') {
    if (!this.xrExperience) return [0, 0, 0, 1];
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const handTracking = features.getEnabledFeature(WebXRFeatureName.HAND_TRACKING);
    
    if (handTracking) {
      const handData = handTracking.getHandByHandedness(hand);
      if (handData && handData.joints[joint]) {
        const rot = handData.joints[joint].rotationQuaternion;
        return [rot.x, rot.y, rot.z, rot.w];
      }
    }
    return [0, 0, 0, 1];
  }

  isHandTracked(hand = 'right') {
    if (!this.xrExperience) return false;
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const handTracking = features.getEnabledFeature(WebXRFeatureName.HAND_TRACKING);
    
    if (handTracking) {
      const handData = handTracking.getHandByHandedness(hand);
      return !!handData;
    }
    return false;
  }

  // === AR PLANE DETECTION ===

  getDetectedPlanes() {
    if (!this.xrExperience) return [];
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const planeDetection = features.getEnabledFeature(WebXRFeatureName.PLANE_DETECTION);
    
    if (planeDetection) {
      return Array.from(planeDetection.detectedPlanes.values()).map(plane => ({
        id: plane.id,
        position: [plane.position.x, plane.position.y, plane.position.z],
        rotation: [plane.rotationQuaternion.x, plane.rotationQuaternion.y, plane.rotationQuaternion.z, plane.rotationQuaternion.w],
        polygon: plane.polygon.map(point => [point.x, point.y, point.z])
      }));
    }
    return [];
  }

  createMeshOnPlane(planeId, meshFactory) {
    if (!this.xrExperience || !meshFactory) return null;
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const planeDetection = features.getEnabledFeature(WebXRFeatureName.PLANE_DETECTION);
    
    if (planeDetection) {
      const plane = planeDetection.detectedPlanes.get(planeId);
      if (plane) {
        const mesh = meshFactory();
        mesh.position = plane.position.clone();
        mesh.rotationQuaternion = plane.rotationQuaternion.clone();
        return mesh;
      }
    }
    return null;
  }

  // === XR ANCHORS ===

  createAnchor(position, rotation = [0, 0, 0, 1]) {
    if (!this.xrExperience) return null;
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const anchors = features.getEnabledFeature(WebXRFeatureName.ANCHOR_SYSTEM);
    
    if (anchors) {
      const pos = new Vector3(...position);
      const rot = new Quaternion(...rotation);
      
      return anchors.addAnchorPointUsingHitTestResultAsync({
        position: pos,
        rotationQuaternion: rot
      });
    }
    return null;
  }

  removeAnchor(anchor) {
    if (!anchor || !this.xrExperience) return false;
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const anchors = features.getEnabledFeature(WebXRFeatureName.ANCHOR_SYSTEM);
    
    if (anchors) {
      anchors.removeAnchor(anchor);
      return true;
    }
    return false;
  }

  getAllAnchors() {
    if (!this.xrExperience) return [];
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const anchors = features.getEnabledFeature(WebXRFeatureName.ANCHOR_SYSTEM);
    
    if (anchors) {
      return Array.from(anchors.anchors.values()).map(anchor => ({
        id: anchor.id,
        position: [anchor.transformationMatrix.getTranslation().x, anchor.transformationMatrix.getTranslation().y, anchor.transformationMatrix.getTranslation().z]
      }));
    }
    return [];
  }

  // === XR HIT TESTING ===

  performHitTest(screenX = 0.5, screenY = 0.5) {
    if (!this.xrExperience) return null;
    
    const features = this.xrExperience.baseExperience.featuresManager;
    const hitTest = features.getEnabledFeature(WebXRFeatureName.HIT_TEST);
    
    if (hitTest) {
      return hitTest.doHitTestFromCoordinatesAsync(screenX, screenY);
    }
    return null;
  }

  // === XR SESSION INFO ===

  getXRSessionMode() {
    if (!this.xrExperience || !this.xrExperience.baseExperience) return 'none';
    
    const session = this.xrExperience.baseExperience.session;
    return session ? session.mode : 'none';
  }

  getXRReferenceSpace() {
    if (!this.xrExperience || !this.xrExperience.baseExperience) return 'none';
    
    return this.xrExperience.baseExperience.referenceSpace || 'none';
  }

  isXRSupported() {
    return navigator.xr !== undefined;
  }

  async checkXRSupport(mode = 'immersive-vr') {
    if (!navigator.xr) return false;
    
    try {
      return await navigator.xr.isSessionSupported(mode);
    } catch (error) {
      return false;
    }
  }

  // === XR FEATURES ===

  getEnabledFeatures() {
    if (!this.xrExperience) return [];
    
    const features = this.xrExperience.baseExperience.featuresManager;
    return features.getEnabledFeatures().map(feature => ({
      name: feature.featureName,
      version: feature.xrNativeFeatureName
    }));
  }

  getAvailableFeatures() {
    if (!this.xrExperience) return [];
    
    const features = this.xrExperience.baseExperience.featuresManager;
    return features.getAvailableFeatures();
  }

  // === XR EVENTS ===

  onControllerConnect(callback) {
    if (!this.xrExperience || !callback) return false;
    
    this.xrExperience.input.onControllerAddedObservable.add((controller) => {
      callback({
        controllerId: controller.uniqueId,
        hand: controller.inputSource.handedness,
        type: controller.inputSource.targetRayMode
      });
    });
    return true;
  }

  onControllerDisconnect(callback) {
    if (!this.xrExperience || !callback) return false;
    
    this.xrExperience.input.onControllerRemovedObservable.add((controller) => {
      callback({
        controllerId: controller.uniqueId,
        hand: controller.inputSource.handedness
      });
    });
    return true;
  }

  onXRSessionStart(callback) {
    if (!this.xrExperience || !callback) return false;
    
    this.xrExperience.baseExperience.onStateChangedObservable.add((state) => {
      if (state === 'IN_XR') {
        callback();
      }
    });
    return true;
  }

  onXRSessionEnd(callback) {
    if (!this.xrExperience || !callback) return false;
    
    this.xrExperience.baseExperience.onStateChangedObservable.add((state) => {
      if (state === 'NOT_IN_XR') {
        callback();
      }
    });
    return true;
  }

  // === XR UTILITIES ===

  convertControllerSpaceToWorldSpace(position, controllerId = 0) {
    if (!this.xrExperience || !this.xrExperience.input) return position;
    
    const controllers = this.xrExperience.input.controllers;
    const controller = controllers[controllerId];
    
    if (controller && controller.grip) {
      const worldMatrix = controller.grip.getWorldMatrix();
      const localPos = new Vector3(...position);
      const worldPos = Vector3.TransformCoordinates(localPos, worldMatrix);
      return [worldPos.x, worldPos.y, worldPos.z];
    }
    return position;
  }

  getXRInfo() {
    if (!this.xrExperience) return null;
    
    return {
      inSession: this.isInXR(),
      sessionMode: this.getXRSessionMode(),
      referenceSpace: this.getXRReferenceSpace(),
      controllersCount: this.xrExperience.input ? this.xrExperience.input.controllers.length : 0,
      enabledFeatures: this.getEnabledFeatures(),
      headPosition: this.getHeadPosition(),
      headRotation: this.getHeadRotation()
    };
  }

  // === XR PERFORMANCE ===

  setXRFrameRate(rate = 90) {
    if (!this.xrExperience || !this.xrExperience.baseExperience) return false;
    
    const session = this.xrExperience.baseExperience.session;
    if (session && session.updateRenderState) {
      session.updateRenderState({
        baseLayer: session.renderState.baseLayer,
        depthFar: 1000,
        depthNear: 0.1,
        inlineVerticalFieldOfView: Math.PI / 4
      });
      return true;
    }
    return false;
  }

  getXRFrameRate() {
    if (!this.xrExperience || !this.xrExperience.baseExperience) return 60;
    
    const session = this.xrExperience.baseExperience.session;
    return session ? (session.frameRate || 90) : 60;
  }
}