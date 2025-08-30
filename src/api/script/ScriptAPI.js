import { CoreAPI } from './modules/CoreAPI.js';
import { MaterialAPI } from './modules/MaterialAPI.js';
import { MeshAPI } from './modules/MeshAPI.js';
import { AnimationAPI } from './modules/AnimationAPI.js';
import { SceneAPI } from './modules/SceneAPI.js';
import { PhysicsAPI } from './modules/PhysicsAPI.js';
import { InputAPI } from './modules/InputAPI.js';
import { TextureAPI } from './modules/TextureAPI.js';
import { ParticleAPI } from './modules/ParticleAPI.js';
import { AudioAPI } from './modules/AudioAPI.js';
import { GUIAPI } from './modules/GUIAPI.js';
import { PostProcessAPI } from './modules/PostProcessAPI.js';
import { XRAPI } from './modules/XRAPI.js';
import { DebugAPI } from './modules/DebugAPI.js';
import { AssetAPI } from './modules/AssetAPI.js';
import { UtilityAPI } from './modules/UtilityAPI.js';
import { CameraAPI } from './modules/CameraAPI.js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';

/**
 * ScriptAPIModular - Complete modular Babylon.js scripting interface
 * Combines all feature modules into a single comprehensive API
 */
export class ScriptAPI {
  constructor(scene, babylonObject) {
    this.scene = scene;
    this.babylonObject = babylonObject;
    
    // Initialize all API modules
    this.core = new CoreAPI(scene, babylonObject);
    this.material = new MaterialAPI(scene);
    this.mesh = new MeshAPI(scene);
    this.animation = new AnimationAPI(scene, babylonObject);
    this.sceneQuery = new SceneAPI(scene, babylonObject);
    this.physics = new PhysicsAPI(scene, babylonObject);
    this.input = new InputAPI(scene, babylonObject);
    this.texture = new TextureAPI(scene);
    this.particle = new ParticleAPI(scene);
    this.audio = new AudioAPI(scene);
    this.gui = new GUIAPI(scene);
    this.postProcess = new PostProcessAPI(scene);
    this.xr = new XRAPI(scene);
    this.debug = new DebugAPI(scene);
    this.asset = new AssetAPI(scene);
    this.utility = new UtilityAPI(scene);
    this.camera = new CameraAPI(scene, babylonObject);
    
    // Bind all methods to this instance for direct access
    this._bindAllMethods();
  }

  // === CORE API METHODS (HIGHEST PRIORITY) ===
  
  // Transform methods
  getPosition() { return this.core.getPosition(); }
  setPosition(x, y, z) { return this.core.setPosition(x, y, z); }
  getWorldPosition() { return this.core.getWorldPosition(); }
  getRotation() { return this.core.getRotation(); }
  setRotation(x, y, z) { return this.core.setRotation(x, y, z); }
  getWorldRotation() { return this.core.getWorldRotation(); }
  getWorldRotationQuaternion() { return this.core.getWorldRotationQuaternion(); }
  getScale() { return this.core.getScale(); }
  setScale(x, y, z) { return this.core.setScale(x, y, z); }
  moveBy(x, y, z) { return this.core.moveBy(x, y, z); }
  rotateBy(x, y, z) { return this.core.rotateBy(x, y, z); }
  lookAt(x, y, z) { return this.core.lookAt(x, y, z); }

  // Visibility
  isVisible() { return this.core.isVisible(); }
  setVisible(visible) { return this.core.setVisible(visible); }
  setEnabled(enabled) { return this.core.setEnabled(enabled); }
  isEnabled() { return this.core.isEnabled(); }

  // Tags
  addTag(tag) { return this.core.addTag(tag); }
  removeTag(tag) { return this.core.removeTag(tag); }
  hasTag(tag) { return this.core.hasTag(tag); }
  getTags() { return this.core.getTags(); }

  // Time and utility
  getDeltaTime() { return this.core.getDeltaTime(); }
  getTime() { return this.core.getTime(); }
  log(...args) { return this.core.log(...args); }

  // Math utilities
  random() { return this.core.random(); }
  randomRange(min, max) { return this.core.randomRange(min, max); }
  clamp(value, min, max) { return this.core.clamp(value, min, max); }
  lerp(start, end, t) { return this.core.lerp(start, end, t); }
  distance(x1, y1, z1, x2, y2, z2) { return this.core.distance(x1, y1, z1, x2, y2, z2); }
  normalize(x, y, z) { return this.core.normalize(x, y, z); }
  dot(x1, y1, z1, x2, y2, z2) { return this.core.dot(x1, y1, z1, x2, y2, z2); }
  cross(x1, y1, z1, x2, y2, z2) { return this.core.cross(x1, y1, z1, x2, y2, z2); }
  toRadians(degrees) { return this.core.toRadians(degrees); }
  toDegrees(radians) { return this.core.toDegrees(radians); }

  // === MATERIAL API METHODS ===
  
  setColor(r, g, b, a) { return this.material.setColor(r, g, b, a); }
  getColor() { return this.material.getColor(); }
  setAlpha(alpha) { return this.material.setAlpha(alpha); }
  getAlpha() { return this.material.getAlpha(); }
  setDiffuseColor(r, g, b) { return this.material.setDiffuseColor(r, g, b); }
  setSpecularColor(r, g, b) { return this.material.setSpecularColor(r, g, b); }
  setEmissiveColor(r, g, b) { return this.material.setEmissiveColor(r, g, b); }
  setAmbientColor(r, g, b) { return this.material.setAmbientColor(r, g, b); }
  getEmissiveColor() { return this.material.getEmissiveColor(); }
  setSpecularPower(power) { return this.material.setSpecularPower(power); }
  setMaterialProperty(property, value) { return this.material.setMaterialProperty(property, value); }
  getMaterialProperty(property) { return this.material.getMaterialProperty(property); }

  // Texture methods
  setTexture(texturePath) { return this.material.setTexture(texturePath); }
  setDiffuseTexture(texturePath) { return this.material.setDiffuseTexture(texturePath); }
  setNormalTexture(texturePath) { return this.material.setNormalTexture(texturePath); }
  setSpecularTexture(texturePath) { return this.material.setSpecularTexture(texturePath); }
  setEmissiveTexture(texturePath) { return this.material.setEmissiveTexture(texturePath); }
  setAmbientTexture(texturePath) { return this.material.setAmbientTexture(texturePath); }
  setOpacityTexture(texturePath) { return this.material.setOpacityTexture(texturePath); }
  setReflectionTexture(texturePath) { return this.material.setReflectionTexture(texturePath); }
  setRefractionTexture(texturePath) { return this.material.setRefractionTexture(texturePath); }
  setLightmapTexture(texturePath) { return this.material.setLightmapTexture(texturePath); }
  setMetallicTexture(texturePath) { return this.material.setMetallicTexture(texturePath); }
  setRoughnessTexture(texturePath) { return this.material.setRoughnessTexture(texturePath); }
  setMicroRoughnessTexture(texturePath) { return this.material.setMicroRoughnessTexture(texturePath); }
  setDisplacementTexture(texturePath) { return this.material.setDisplacementTexture(texturePath); }
  setDetailTexture(texturePath) { return this.material.setDetailTexture(texturePath); }

  // Material rendering
  setBackFaceCulling(enabled) { return this.material.setBackFaceCulling(enabled); }
  setDisableLighting(disabled) { return this.material.setDisableLighting(disabled); }
  setWireframe(enabled) { return this.material.setWireframe(enabled); }
  setPointsCloud(enabled) { return this.material.setPointsCloud(enabled); }
  setFillMode(mode) { return this.material.setFillMode(mode); }
  setInvertNormalMapX(invert) { return this.material.setInvertNormalMapX(invert); }
  setInvertNormalMapY(invert) { return this.material.setInvertNormalMapY(invert); }
  setBumpLevel(level) { return this.material.setBumpLevel(level); }
  setParallaxScaleBias(scale, bias) { return this.material.setParallaxScaleBias(scale, bias); }
  setIndexOfRefraction(ior) { return this.material.setIndexOfRefraction(ior); }
  setFresnelParameters(bias, power, left, right) { return this.material.setFresnelParameters(bias, power, left, right); }

  // === ANIMATION API METHODS ===
  
  animate(property, targetValue, duration, loop) { return this.animation.animate(property, targetValue, duration, loop); }
  stopAnimation(property) { return this.animation.stopAnimation(property); }
  pauseAnimation(property) { return this.animation.pauseAnimation(property); }
  resumeAnimation(property) { return this.animation.resumeAnimation(property); }
  animatePosition(x, y, z, duration, easing) { return this.animation.animatePosition(x, y, z, duration, easing); }
  animateRotation(x, y, z, duration, easing) { return this.animation.animateRotation(x, y, z, duration, easing); }
  animateScale(x, y, z, duration, easing) { return this.animation.animateScale(x, y, z, duration, easing); }
  animateColor(r, g, b, duration, easing) { return this.animation.animateColor(r, g, b, duration, easing); }
  animateAlpha(alpha, duration, easing) { return this.animation.animateAlpha(alpha, duration, easing); }
  animateTo(property, targetValue, duration, easing) { return this.animation.animateTo(property, targetValue, duration, easing); }

  // === SCENE API METHODS ===
  
  findObjectByName(name) { return this.sceneQuery.findObjectByName(name); }
  findObjectsByName(name) { return this.sceneQuery.findObjectsByName(name); }
  findObjectsByTag(tag) { return this.sceneQuery.findObjectsByTag(tag); }
  findObjectsWithTag(tag) { return this.sceneQuery.findObjectsWithTag(tag); }
  raycast(ox, oy, oz, dx, dy, dz, maxDist) { return this.sceneQuery.raycast(ox, oy, oz, dx, dy, dz, maxDist); }
  raycastFromCamera(x, y, camera) { return this.sceneQuery.raycastFromCamera(x, y, camera); }
  multiRaycast(ox, oy, oz, dx, dy, dz, maxDist) { return this.sceneQuery.multiRaycast(ox, oy, oz, dx, dy, dz, maxDist); }
  pickObject(x, y) { return this.sceneQuery.pickObject(x, y); }
  pickObjects(x, y) { return this.sceneQuery.pickObjects(x, y); }
  getObjectsInRadius(x, y, z, radius) { return this.sceneQuery.getObjectsInRadius(x, y, z, radius); }
  getObjectsInBox(minX, minY, minZ, maxX, maxY, maxZ) { return this.sceneQuery.getObjectsInBox(minX, minY, minZ, maxX, maxY, maxZ); }
  getClosestObject(x, y, z, tag) { return this.sceneQuery.getClosestObject(x, y, z, tag); }
  intersectsMesh(mesh) { return this.sceneQuery.intersectsMesh(mesh); }
  intersectsPoint(x, y, z) { return this.sceneQuery.intersectsPoint(x, y, z); }
  getBoundingInfo() { return this.sceneQuery.getBoundingInfo(); }
  cloneObject(name, parent) { return this.sceneQuery.cloneObject(name, parent); }

  // === PHYSICS API METHODS ===
  
  enablePhysics(engine, gx, gy, gz) { return this.physics.enablePhysics(engine, gx, gy, gz); }
  disablePhysics() { return this.physics.disablePhysics(); }
  setPhysicsImpostor(type, mass, options) { return this.physics.setPhysicsImpostor(type, mass, options); }
  removePhysicsImpostor() { return this.physics.removePhysicsImpostor(); }
  hasPhysicsImpostor() { return this.physics.hasPhysicsImpostor(); }
  havokUpdate() { return this.physics.havok_update(); }
  applyImpulse(fx, fy, fz, px, py, pz) { return this.physics.applyImpulse(fx, fy, fz, px, py, pz); }
  applyForce(fx, fy, fz, px, py, pz) { return this.physics.applyForce(fx, fy, fz, px, py, pz); }
  setLinearVelocity(vx, vy, vz) { return this.physics.setLinearVelocity(vx, vy, vz); }
  getLinearVelocity() { return this.physics.getLinearVelocity(); }
  setAngularVelocity(vx, vy, vz) { return this.physics.setAngularVelocity(vx, vy, vz); }
  getAngularVelocity() { return this.physics.getAngularVelocity(); }
  setMass(mass) { return this.physics.setMass(mass); }
  getMass() { return this.physics.getMass(); }
  setFriction(friction) { return this.physics.setFriction(friction); }
  getFriction() { return this.physics.getFriction(); }
  setRestitution(restitution) { return this.physics.setRestitution(restitution); }
  getRestitution() { return this.physics.getRestitution(); }

  // === INPUT API METHODS ===
  
  isKeyPressed(key) { return this.input.isKeyPressed(key); }
  isKeyDown(key) { return this.input.isKeyDown(key); }
  isMouseButtonPressed(button) { return this.input.isMouseButtonPressed(button); }
  getMousePosition() { return this.input.getMousePosition(); }
  getMouseX() { return this.input.getMouseX(); }
  getMouseY() { return this.input.getMouseY(); }
  getGamepads() { return this.input.getGamepads(); }
  getGamepad(index) { return this.input.getGamepad(index); }
  isGamepadConnected(index) { return this.input.isGamepadConnected(index); }
  isGamepadButtonPressed(button, gamepadIndex) { return this.input.isGamepadButtonPressed(button, gamepadIndex); }
  getLeftStick(gamepadIndex) { return this.input.getLeftStick(gamepadIndex); }
  getRightStick(gamepadIndex) { return this.input.getRightStick(gamepadIndex); }
  getLeftStickX(gamepadIndex) { return this.input.getLeftStickX(gamepadIndex); }
  getLeftStickY(gamepadIndex) { return this.input.getLeftStickY(gamepadIndex); }
  getRightStickX(gamepadIndex) { return this.input.getRightStickX(gamepadIndex); }
  getRightStickY(gamepadIndex) { return this.input.getRightStickY(gamepadIndex); }
  getGamepadTrigger(trigger, gamepadIndex) { return this.input.getGamepadTrigger(trigger, gamepadIndex); }
  isTouching() { return this.input.isTouching(); }
  getTouches() { return this.input.getTouches(); }

  // === SKELETON ANIMATION FUNCTIONS ===
  // Essential for RenScript skeleton scripts
  
  has_skeleton() {
    return this.animation.hasSkeletonAPI(this.babylonObject);
  }

  play_idle_animation() {
    return this.animation.playIdleAnimation(this.babylonObject);
  }

  play_walk_animation() {
    return this.animation.playWalkAnimation(this.babylonObject);
  }

  play_run_animation() {
    return this.animation.playRunAnimation(this.babylonObject);
  }

  play_jump_animation() {
    return this.animation.playJumpAnimation(this.babylonObject);
  }


  stop_animation() {
    return this.animation.stopAnimationAPI(this.babylonObject);
  }

  set_animation_speed(speed) {
    return this.animation.setAnimationSpeedAPI(this.babylonObject, speed);
  }

  // === CAMELCASE ALIASES FOR SKELETON ANIMATIONS ===
  // For RenScript compiler compatibility
  
  hasSkeleton() {
    return this.has_skeleton();
  }

  playIdleAnimation() {
    return this.play_idle_animation();
  }

  playWalkAnimation() {
    return this.play_walk_animation();
  }

  playRunAnimation() {
    return this.play_run_animation();
  }

  playJumpAnimation() {
    return this.play_jump_animation();
  }

  stopAnimation() {
    return this.stop_animation();
  }

  setAnimationSpeed(speed) {
    return this.set_animation_speed(speed);
  }

  // === CAMERA API METHODS ===
  
  getActiveCamera() { return this.camera.getActiveCamera(); }
  setCameraPosition(x, y, z) { return this.camera.setCameraPosition(x, y, z); }
  getCameraPosition() { return this.camera.getCameraPosition(); }
  setCameraTarget(x, y, z) { return this.camera.setCameraTarget(x, y, z); }
  getCameraTarget() { return this.camera.getCameraTarget(); }
  setCameraRotation(x, y, z) { return this.camera.setCameraRotation(x, y, z); }
  getCameraRotation() { return this.camera.getCameraRotation(); }
  setCameraFOV(fov) { return this.camera.setCameraFOV(fov); }
  getCameraFOV() { return this.camera.getCameraFOV(); }
  setCameraRadius(radius) { return this.camera.setCameraRadius(radius); }
  getCameraRadius() { return this.camera.getCameraRadius(); }
  setCameraType(type) { return this.camera.setCameraType(type); }
  orbitCamera(speed, direction) { return this.camera.orbitCamera(speed, direction); }

  // === COMPATIBILITY LAYER ===
  
  // RenScript snake_case compatibility
  get_position() { return this.getPosition(); }
  set_position(x, y, z) { return this.setPosition(x, y, z); }
  get_world_position() { return this.getWorldPosition(); }
  get_rotation() { return this.getRotation(); }
  set_rotation(x, y, z) { return this.setRotation(x, y, z); }
  get_world_rotation() { return this.getWorldRotation(); }
  get_scale() { return this.getScale(); }
  set_scale(x, y, z) { return this.setScale(x, y, z); }
  move_by(x, y, z) { return this.moveBy(x, y, z); }
  rotate_by(x, y, z) { return this.rotateBy(x, y, z); }
  look_at(x, y, z) { return this.lookAt(x, y, z); }
  is_visible() { return this.isVisible(); }
  set_visible(visible) { return this.setVisible(visible); }
  add_tag(tag) { return this.addTag(tag); }
  remove_tag(tag) { return this.removeTag(tag); }
  has_tag(tag) { return this.hasTag(tag); }
  set_color(r, g, b, a) { return this.setColor(r, g, b, a); }
  get_color() { return this.getColor(); }
  set_diffuse_color(r, g, b) { return this.setDiffuseColor(r, g, b); }
  set_specular_color(r, g, b) { return this.setSpecularColor(r, g, b); }
  set_emissive_color(r, g, b) { return this.setEmissiveColor(r, g, b); }
  set_ambient_color(r, g, b) { return this.setAmbientColor(r, g, b); }
  get_emissive_color() { return this.getEmissiveColor(); }
  set_alpha(alpha) { return this.setAlpha(alpha); }
  set_specular_power(power) { return this.setSpecularPower(power); }
  set_material_property(prop, value) { return this.setMaterialProperty(prop, value); }
  get_material_property(prop) { return this.getMaterialProperty(prop); }
  set_texture(path) { return this.setTexture(path); }
  set_diffuse_texture(path) { return this.setDiffuseTexture(path); }
  set_normal_texture(path) { return this.setNormalTexture(path); }
  set_specular_texture(path) { return this.setSpecularTexture(path); }
  set_emissive_texture(path) { return this.setEmissiveTexture(path); }
  set_ambient_texture(path) { return this.setAmbientTexture(path); }
  set_opacity_texture(path) { return this.setOpacityTexture(path); }
  set_reflection_texture(path) { return this.setReflectionTexture(path); }
  set_refraction_texture(path) { return this.setRefractionTexture(path); }
  set_lightmap_texture(path) { return this.setLightmapTexture(path); }
  set_metallic_texture(path) { return this.setMetallicTexture(path); }
  set_roughness_texture(path) { return this.setRoughnessTexture(path); }
  set_micro_roughness_texture(path) { return this.setMicroRoughnessTexture(path); }
  set_displacement_texture(path) { return this.setDisplacementTexture(path); }
  set_detail_texture(path) { return this.setDetailTexture(path); }
  set_back_face_culling(enabled) { return this.setBackFaceCulling(enabled); }
  set_disable_lighting(disabled) { return this.setDisableLighting(disabled); }
  set_wireframe(enabled) { return this.setWireframe(enabled); }
  set_points_cloud(enabled) { return this.setPointsCloud(enabled); }
  set_fill_mode(mode) { return this.setFillMode(mode); }
  set_invert_normal_map_x(invert) { return this.setInvertNormalMapX(invert); }
  set_invert_normal_map_y(invert) { return this.setInvertNormalMapY(invert); }
  set_bump_level(level) { return this.setBumpLevel(level); }
  set_parallax_scale_bias(scale, bias) { return this.setParallaxScaleBias(scale, bias); }
  set_index_of_refraction(ior) { return this.setIndexOfRefraction(ior); }
  set_fresnel_parameters(bias, power, left, right) { return this.setFresnelParameters(bias, power, left, right); }
  find_object_by_name(name) { return this.findObjectByName(name); }
  find_objects_by_tag(tag) { return this.findObjectsByTag(tag); }
  get_objects_in_radius(x, y, z, radius) { return this.getObjectsInRadius(x, y, z, radius); }
  is_key_pressed(key) { return this.isKeyPressed(key); }
  is_mouse_button_pressed(button) { return this.isMouseButtonPressed(button); }
  get_mouse_position() { return this.getMousePosition(); }
  get_left_stick(gamepadIndex) { return this.getLeftStick(gamepadIndex); }
  get_right_stick(gamepadIndex) { return this.getRightStick(gamepadIndex); }
  get_left_stick_x(gamepadIndex) { return this.getLeftStickX(gamepadIndex); }
  get_left_stick_y(gamepadIndex) { return this.getLeftStickY(gamepadIndex); }
  get_right_stick_x(gamepadIndex) { return this.getRightStickX(gamepadIndex); }
  get_right_stick_y(gamepadIndex) { return this.getRightStickY(gamepadIndex); }
  is_gamepad_button_pressed(button, gamepadIndex) { return this.isGamepadButtonPressed(button, gamepadIndex); }
  get_gamepad_trigger(trigger, gamepadIndex) { return this.getGamepadTrigger(trigger, gamepadIndex); }
  get_delta_time() { return this.getDeltaTime(); }
  get_time() { return this.getTime(); }
  
  // Camera snake_case compatibility
  get_active_camera() { return this.getActiveCamera(); }
  set_camera_position(x, y, z) { return this.setCameraPosition(x, y, z); }
  get_camera_position() { return this.getCameraPosition(); }
  set_camera_target(x, y, z) { return this.setCameraTarget(x, y, z); }
  get_camera_target() { return this.getCameraTarget(); }
  set_camera_rotation(x, y, z) { return this.setCameraRotation(x, y, z); }
  get_camera_rotation() { return this.getCameraRotation(); }
  set_camera_fov(fov) { return this.setCameraFOV(fov); }
  get_camera_fov() { return this.getCameraFOV(); }
  set_camera_radius(radius) { return this.setCameraRadius(radius); }
  get_camera_radius() { return this.getCameraRadius(); }
  set_camera_type(type) { return this.setCameraType(type); }
  orbit_camera(speed, direction) { return this.orbitCamera(speed, direction); }

  // === INTERNAL METHODS ===
  
  _bindAllMethods() {
    // Bind all methods for proper context
    const bindMethods = (obj, prefix = '') => {
      Object.getOwnPropertyNames(Object.getPrototypeOf(obj))
        .filter(name => name !== 'constructor' && typeof obj[name] === 'function')
        .forEach(name => {
          if (prefix && this[name]) {
            // Don't override if already exists
            return;
          }
          const boundMethod = obj[name].bind(obj);
          if (prefix) {
            this[`${prefix}_${name}`] = boundMethod;
          } else {
            this[name] = boundMethod;
          }
        });
    };

    // Bind all module methods
    bindMethods(this.core);
    bindMethods(this.material, 'material');
    bindMethods(this.mesh, 'mesh');
    bindMethods(this.animation, 'animation');
    bindMethods(this.sceneQuery, 'scene');
    bindMethods(this.physics, 'physics');
    bindMethods(this.input, 'input');
    bindMethods(this.texture, 'texture');
    bindMethods(this.particle, 'particle');
    bindMethods(this.audio, 'audio');
    bindMethods(this.gui, 'gui');
    bindMethods(this.postProcess, 'postProcess');
    bindMethods(this.xr, 'xr');
    bindMethods(this.debug, 'debug');
    bindMethods(this.asset, 'asset');
    bindMethods(this.utility, 'utility');
    bindMethods(this.camera, 'camera');
  }

  // === SCRIPT PROPERTY METHODS ===
  
  initializeScriptProperties() {
    if (this._scriptProperties) {
      if (Array.isArray(this._scriptProperties)) {
        // Handle array format from RenScript compiler
        this._scriptProperties.forEach(property => {
          if (property.defaultValue !== undefined) {
            this[property.name] = property.defaultValue;
          }
        });
      } else {
        // Handle Map format (legacy)
        this._scriptProperties.forEach((property, key) => {
          if (property.default !== undefined) {
            this[key] = property.default;
          }
        });
      }
    }
  }
  
  initializeScriptInstanceProperties(scriptInstance) {
    if (this._scriptProperties && scriptInstance) {
      if (Array.isArray(this._scriptProperties)) {
        // Handle array format from RenScript compiler
        this._scriptProperties.forEach(property => {
          if (this[property.name] !== undefined) {
            scriptInstance[property.name] = this[property.name];
          }
        });
      } else {
        // Handle Map format (legacy)
        this._scriptProperties.forEach((property, key) => {
          if (this[key] !== undefined) {
            scriptInstance[key] = this[key];
          }
        });
      }
    }
  }
  
  updateScriptProperty(propertyName, value) {
    console.log(`🔧 updateScriptProperty called: ${propertyName} = ${value}`);
    
    const hasProperty = Array.isArray(this._scriptProperties) ? 
      this._scriptProperties.some(prop => prop.name === propertyName) :
      this._scriptProperties && this._scriptProperties.has(propertyName);
      
    if (hasProperty) {
      console.log(`🔧 Property ${propertyName} found in script properties`);
      this[propertyName] = value;
      
      // Update the script instance if it exists
      if (this._scriptInstance) {
        this._scriptInstance[propertyName] = value;
        console.log(`🔧 Updated script instance property: ${propertyName}`);
      }
      
      // Check if this property has once: true and trigger onOnce if it does
      if (Array.isArray(this._scriptProperties)) {
        const property = this._scriptProperties.find(prop => prop.name === propertyName);
        console.log(`🔧 Found property definition:`, property);
        if (property && property.triggerOnce === true) {
          console.log(`🔄 Property ${propertyName} has triggerOnce: true, triggering onOnce`);
          if (this._scriptInstance && typeof this._scriptInstance.onOnce === 'function') {
            try {
              console.log(`🔄 Calling onOnce() method...`);
              this._scriptInstance.onOnce();
              console.log(`✅ onOnce() called successfully`);
            } catch (error) {
              console.error(`Error calling onOnce for property ${propertyName}:`, error);
            }
          } else {
            console.log(`❌ No onOnce method found on script instance`);
          }
        } else {
          console.log(`🔧 Property ${propertyName} does not have triggerOnce: true (triggerOnce = ${property?.triggerOnce})`);
        }
      }
      
      return true;
    } else {
      console.log(`❌ Property ${propertyName} not found in script properties`);
    }
    return false;
  }
  
  getScriptProperty(propertyName) {
    const hasProperty = Array.isArray(this._scriptProperties) ? 
      this._scriptProperties.some(prop => prop.name === propertyName) :
      this._scriptProperties && this._scriptProperties.has(propertyName);
      
    if (hasProperty) {
      return this[propertyName];
    }
    return undefined;
  }
  
  getAllScriptProperties() {
    const props = {};
    if (this._scriptProperties) {
      if (Array.isArray(this._scriptProperties)) {
        this._scriptProperties.forEach(property => {
          props[property.name] = {
            value: this[property.name],
            ...property
          };
        });
      } else {
        this._scriptProperties.forEach((property, key) => {
          props[key] = {
            value: this[key],
            ...property
          };
        });
      }
    }
    return props;
  }
  
  getScriptProperties() {
    const props = [];
    if (this._scriptProperties) {
      if (Array.isArray(this._scriptProperties)) {
        // Already in the correct array format
        return this._scriptProperties.map(property => ({
          name: property.name,
          type: property.type,
          defaultValue: property.defaultValue,
          min: property.min,
          max: property.max,
          options: property.options,
          description: property.description,
          section: property.section || 'General'
        }));
      } else {
        // Handle Map format (legacy)
        this._scriptProperties.forEach((property, key) => {
          props.push({
            name: key,
            type: property.type,
            defaultValue: property.default,
            min: property.min,
            max: property.max,
            options: property.options,
            description: property.description,
            section: property.section || 'General'
          });
        });
      }
    }
    return props;
  }
  
  getScriptPropertiesBySection() {
    const sections = {};
    if (this._scriptProperties) {
      if (Array.isArray(this._scriptProperties)) {
        this._scriptProperties.forEach(property => {
          const section = property.section || 'General';
          if (!sections[section]) {
            sections[section] = [];
          }
          sections[section].push({
            name: property.name,
            type: property.type,
            defaultValue: property.defaultValue,
            min: property.min,
            max: property.max,
            options: property.options,
            description: property.description
          });
        });
      } else {
        // Handle Map format (legacy)
        this._scriptProperties.forEach((property, key) => {
          const section = property.section || 'General';
          if (!sections[section]) {
            sections[section] = [];
          }
          sections[section].push({
            name: key,
            type: property.type,
            defaultValue: property.default,
            min: property.min,
            max: property.max,
            options: property.options,
            description: property.description
          });
        });
      }
    }
    return sections;
  }
  
  setScriptProperty(propertyName, value) {
    console.log(`🔧 setScriptProperty called: ${propertyName} = ${value}`);
    return this.updateScriptProperty(propertyName, value);
  }

  // === DYNAMIC PROPERTIES ===
  
  addDynamicProperty(name, type, options = {}) {
    console.log(`🔧 Adding dynamic property: ${name} (${type})`);
    if (!this._scriptInstance._scriptProperties) {
      this._scriptInstance._scriptProperties = [];
    }
    
    const property = {
      name: name,
      type: type,
      section: options.section || 'Dynamic',
      defaultValue: options.default || (type === 'boolean' ? false : type === 'select' ? 'none' : 0),
      min: options.min || null,
      max: options.max || null,
      options: options.options || (type === 'select' ? ['none'] : null),
      description: options.description || `Dynamic ${type} property`,
      triggerOnce: options.once || false
    };
    
    this._scriptInstance._scriptProperties.push(property);
    
    // Initialize the property value
    this._scriptInstance[name] = property.defaultValue;
    
    // Trigger UI update
    this.updateScriptPropertyMetadata();
    return true;
  }
  
  updatePropertyOptions(propertyName, newOptions) {
    console.log(`🔧 Updating property options for: ${propertyName}`, newOptions);
    if (!this._scriptInstance || !this._scriptInstance._scriptProperties) return false;
    
    // Find and update the property
    const property = this._scriptInstance._scriptProperties.find(p => p.name === propertyName);
    if (property && property.type === 'select') {
      property.options = ['none', ...newOptions];
      this.updateScriptPropertyMetadata();
      return true;
    }
    return false;
  }
  
  removeDynamicProperty(propertyName) {
    console.log(`🔧 Removing dynamic property: ${propertyName}`);
    if (!this._scriptInstance || !this._scriptInstance._scriptProperties) return false;
    
    const index = this._scriptInstance._scriptProperties.findIndex(p => p.name === propertyName);
    if (index >= 0) {
      this._scriptInstance._scriptProperties.splice(index, 1);
      delete this._scriptInstance[propertyName];
      this.updateScriptPropertyMetadata();
      return true;
    }
    return false;
  }
  
  getPropertyValue(propertyName) {
    return this._scriptInstance ? this._scriptInstance[propertyName] : undefined;
  }
  
  setPropertyValue(propertyName, value) {
    if (this._scriptInstance) {
      this._scriptInstance[propertyName] = value;
      return this.updateScriptProperty(propertyName, value);
    }
    return false;
  }
  
  updateScriptPropertyMetadata() {
    // Trigger a UI refresh for script properties
    if (this.mesh && this.mesh.metadata && this.mesh.metadata.entityId) {
      const event = new CustomEvent('engine:script-properties-updated', {
        detail: { entityId: this.mesh.metadata.entityId }
      });
      document.dispatchEvent(event);
    }
  }

  // === UPDATE METHODS ===
  
  _updateDeltaTime(deltaTime) {
    this.core._updateDeltaTime(deltaTime);
  }

  // === DISPOSAL ===
  
  dispose() {
    this.physics?.disposePhysics?.();
    this.input?.dispose?.();
  }
}