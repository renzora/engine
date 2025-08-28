// === PARTICLE API MODULE ===

import {
  ParticleSystem,
  GPUParticleSystem,
  SubEmitter,
  BoxParticleEmitter,
  ConeParticleEmitter,
  SphereParticleEmitter,
  HemisphereParticleEmitter,
  CylinderParticleEmitter,
  PointParticleEmitter,
  MeshParticleEmitter,
  CustomParticleEmitter,
  Texture,
  Vector3,
  Color3,
  Color4,
  Animation,
  Mesh
} from '@babylonjs/core';

export class ParticleAPI {
  constructor(scene) {
    this.scene = scene;
  }

  // === BASIC PARTICLE SYSTEM CREATION ===

  createParticleSystem(name, capacity = 2000, texture = null, gpu = false) {
    const system = gpu ? 
      new GPUParticleSystem(name, { capacity }, this.scene) :
      new ParticleSystem(name, capacity, this.scene);
    
    if (texture) {
      system.particleTexture = texture;
    }
    
    return system;
  }

  createGPUParticleSystem(name, capacity = 100000, texture = null) {
    return this.createParticleSystem(name, capacity, texture, true);
  }

  // === PARTICLE EMITTERS ===

  setBoxEmitter(system, minEmitBox = [-1, -1, -1], maxEmitBox = [1, 1, 1]) {
    if (!system) return false;
    system.createBoxEmitter(
      new Vector3(...minEmitBox),
      new Vector3(...maxEmitBox)
    );
    return true;
  }

  setSphereEmitter(system, radius = 1, radiusRange = 0) {
    if (!system) return false;
    system.createSphereEmitter(radius, radiusRange);
    return true;
  }

  setConeEmitter(system, radius = 1, angle = Math.PI / 4) {
    if (!system) return false;
    system.createConeEmitter(radius, angle);
    return true;
  }

  setCylinderEmitter(system, radius = 1, height = 1, radiusRange = 0, directionRandomizer = 0) {
    if (!system) return false;
    system.createCylinderEmitter(radius, height, radiusRange, directionRandomizer);
    return true;
  }

  setHemisphereEmitter(system, radius = 1, radiusRange = 0) {
    if (!system) return false;
    system.createHemisphericEmitter(radius, radiusRange);
    return true;
  }

  setPointEmitter(system, direction1 = [0, 1, 0], direction2 = [0, 1, 0]) {
    if (!system) return false;
    system.createPointEmitter(
      new Vector3(...direction1),
      new Vector3(...direction2)
    );
    return true;
  }

  setMeshEmitter(system, mesh) {
    if (!system || !mesh) return false;
    system.emitter = mesh;
    return true;
  }

  // === PARTICLE PROPERTIES ===

  setParticleLifetime(system, minLifetime = 1.0, maxLifetime = 1.0) {
    if (!system) return false;
    system.minLifeTime = minLifetime;
    system.maxLifeTime = maxLifetime;
    return true;
  }

  setParticleSize(system, minSize = 1.0, maxSize = 1.0) {
    if (!system) return false;
    system.minSize = minSize;
    system.maxSize = maxSize;
    return true;
  }

  setParticleScale(system, minScaleX = 1.0, maxScaleX = 1.0, minScaleY = 1.0, maxScaleY = 1.0) {
    if (!system) return false;
    system.minScaleX = minScaleX;
    system.maxScaleX = maxScaleX;
    system.minScaleY = minScaleY;
    system.maxScaleY = maxScaleY;
    return true;
  }

  setParticleSpeed(system, minEmitPower = 1, maxEmitPower = 1, updateSpeed = 0.01) {
    if (!system) return false;
    system.minEmitPower = minEmitPower;
    system.maxEmitPower = maxEmitPower;
    system.updateSpeed = updateSpeed;
    return true;
  }

  setParticleDirection(system, direction1 = [0, 1, 0], direction2 = [0, 1, 0]) {
    if (!system) return false;
    system.direction1 = new Vector3(...direction1);
    system.direction2 = new Vector3(...direction2);
    return true;
  }

  setParticleAngularSpeed(system, minAngularSpeed = 0, maxAngularSpeed = 0) {
    if (!system) return false;
    system.minAngularSpeed = minAngularSpeed;
    system.maxAngularSpeed = maxAngularSpeed;
    return true;
  }

  setParticleRotation(system, minInitialRotation = 0, maxInitialRotation = 0) {
    if (!system) return false;
    system.minInitialRotation = minInitialRotation;
    system.maxInitialRotation = maxInitialRotation;
    return true;
  }

  // === PARTICLE COLORS ===

  setParticleColors(system, color1 = [1, 1, 1, 1], color2 = [1, 1, 1, 1], colorDead = [0, 0, 0, 0]) {
    if (!system) return false;
    system.color1 = new Color4(...color1);
    system.color2 = new Color4(...color2);
    system.colorDead = new Color4(...colorDead);
    return true;
  }

  setParticleGradient(system, gradients) {
    if (!system || !gradients) return false;
    
    // Clear existing gradients
    system.removeColorGradients();
    
    gradients.forEach(gradient => {
      system.addColorGradient(
        gradient.gradient,
        new Color4(...gradient.color1),
        gradient.color2 ? new Color4(...gradient.color2) : undefined
      );
    });
    
    return true;
  }

  setParticleSizeGradient(system, sizeGradients) {
    if (!system || !sizeGradients) return false;
    
    system.removeSizeGradients();
    
    sizeGradients.forEach(gradient => {
      system.addSizeGradient(gradient.gradient, gradient.factor1, gradient.factor2);
    });
    
    return true;
  }

  setParticleVelocityGradient(system, velocityGradients) {
    if (!system || !velocityGradients) return false;
    
    system.removeVelocityGradients();
    
    velocityGradients.forEach(gradient => {
      system.addVelocityGradient(gradient.gradient, gradient.factor1, gradient.factor2);
    });
    
    return true;
  }

  // === PARTICLE FORCES ===

  setGravity(system, gravity = [0, -9.81, 0]) {
    if (!system) return false;
    system.gravity = new Vector3(...gravity);
    return true;
  }

  addParticleForce(system, force) {
    if (!system || !force) return false;
    
    const forceVector = new Vector3(...force);
    if (system.addForce) {
      system.addForce(forceVector);
    } else {
      // Fallback for CPU particle systems
      system.gravity = system.gravity.add(forceVector);
    }
    return true;
  }

  setWindForce(system, windVector = [1, 0, 0], strength = 1.0) {
    if (!system) return false;
    
    const wind = new Vector3(...windVector).scale(strength);
    system.gravity = system.gravity.add(wind);
    return true;
  }

  // === PARTICLE EMISSION ===

  setEmissionRate(system, rate = 10) {
    if (!system) return false;
    system.emitRate = rate;
    return true;
  }

  setBurstMode(system, count1 = 50, count2 = 50, minEmitTime = 0.1, maxEmitTime = 0.2) {
    if (!system) return false;
    system.manualEmitCount = count1;
    system.maxEmitCount = count2;
    system.minEmitTime = minEmitTime;
    system.maxEmitTime = maxEmitTime;
    return true;
  }

  emitParticleBurst(system, count = 50) {
    if (!system) return false;
    
    if (system.burst) {
      system.burst(count);
    } else {
      // Fallback for systems without burst
      const oldRate = system.emitRate;
      system.emitRate = count * 60; // Emit for 1 frame at 60fps
      setTimeout(() => {
        system.emitRate = oldRate;
      }, 16); // ~1 frame
    }
    return true;
  }

  // === PARTICLE ANIMATION ===

  animateParticleProperty(system, property, targetValue, duration = 1000) {
    if (!system || !system[property]) return null;
    
    const frameRate = 60;
    const totalFrames = Math.floor((duration / 1000) * frameRate);
    
    const animation = new Animation(
      `${system.name}_${property}`,
      property,
      frameRate,
      Animation.ANIMATIONTYPE_FLOAT,
      Animation.ANIMATIONLOOPMODE_CONSTANT
    );
    
    animation.setKeys([
      { frame: 0, value: system[property] },
      { frame: totalFrames, value: targetValue }
    ]);
    
    system.animations = system.animations || [];
    system.animations.push(animation);
    
    return this.scene.beginAnimation(system, 0, totalFrames, false, 1.0);
  }

  // === PARTICLE TEXTURES ===

  setParticleTexture(system, texture) {
    if (!system) return false;
    system.particleTexture = texture;
    return true;
  }

  setParticleTextureSheet(system, spriteColumns = 1, spriteRows = 1, startSprite = 0, endSprite = 0) {
    if (!system) return false;
    system.spriteCellHeight = 1.0 / spriteRows;
    system.spriteCellWidth = 1.0 / spriteColumns;
    system.startSpriteCellID = startSprite;
    system.endSpriteCellID = endSprite || (spriteColumns * spriteRows - 1);
    return true;
  }

  // === PARTICLE BLENDING ===

  setParticleBlendMode(system, mode) {
    if (!system) return false;
    // ParticleSystem.BLENDMODE_ONEONE = 0, BLENDMODE_STANDARD = 1, etc.
    system.blendMode = mode;
    return true;
  }

  setParticleAlphaBlend(system, enabled = true) {
    if (!system) return false;
    system.useAlphaFromTexture = enabled;
    return true;
  }

  // === PARTICLE SYSTEM CONTROL ===

  startParticleSystem(system) {
    if (!system) return false;
    system.start();
    return true;
  }

  stopParticleSystem(system) {
    if (!system) return false;
    system.stop();
    return true;
  }

  pauseParticleSystem(system) {
    if (!system) return false;
    if (system.pause) {
      system.pause();
    } else {
      system.updateSpeed = 0;
    }
    return true;
  }

  resumeParticleSystem(system, updateSpeed = 0.01) {
    if (!system) return false;
    if (system.resume) {
      system.resume();
    } else {
      system.updateSpeed = updateSpeed;
    }
    return true;
  }

  resetParticleSystem(system) {
    if (!system) return false;
    system.reset();
    return true;
  }

  disposeParticleSystem(system) {
    if (!system) return false;
    system.dispose();
    return true;
  }

  // === SUB EMITTERS ===

  createSubEmitter(system, type = 0, inheritDirection = false, inheritedVelocityAmount = 1.0) {
    if (!system) return null;
    
    const subEmitter = new SubEmitter(system);
    subEmitter.type = type; // 0=END, 1=BIRTH, 2=DEATH
    subEmitter.inheritDirection = inheritDirection;
    subEmitter.inheritedVelocityAmount = inheritedVelocityAmount;
    
    return subEmitter;
  }

  addSubEmitter(system, subEmitter) {
    if (!system || !subEmitter) return false;
    system.subEmitters.push(subEmitter);
    return true;
  }

  // === PARTICLE NOISE ===

  setParticleNoise(system, noiseTexture, strength = [10, 10, 10]) {
    if (!system || !noiseTexture) return false;
    system.noiseTexture = noiseTexture;
    system.noiseStrength = new Vector3(...strength);
    return true;
  }

  // === PARTICLE COLLISION ===

  enableParticleCollisions(system, mesh, bounciness = 0.7, friction = 0.2) {
    if (!system || !mesh) return false;
    
    system.enableCollision = true;
    system.collisionMesh = mesh;
    system.bounciness = bounciness;
    system.friction = friction;
    return true;
  }

  disableParticleCollisions(system) {
    if (!system) return false;
    system.enableCollision = false;
    system.collisionMesh = null;
    return true;
  }

  // === PARTICLE SHAPE ===

  setParticleShape(system, shape = 0) {
    if (!system) return false;
    // 0=BOX, 1=SPHERE, 2=CONE, 3=CYLINDER, 4=HEMISPHERE, 5=POINT, 6=MESH
    system.particleShape = shape;
    return true;
  }

  setParticleTextureAnimation(system, startFrame = 0, endFrame = 1, speed = 1.0, loop = true) {
    if (!system) return false;
    system.startSpriteCellID = startFrame;
    system.endSpriteCellID = endFrame;
    system.spriteCellChangeSpeed = speed;
    system.spriteCellLoop = loop;
    return true;
  }

  // === PARTICLE PRESETS ===

  createFireParticles(name, position = [0, 0, 0], capacity = 2000) {
    const system = this.createParticleSystem(name, capacity);
    
    // Fire properties
    system.emitter = new Vector3(...position);
    this.setSphereEmitter(system, 0.5, 0.5);
    
    system.minLifeTime = 0.3;
    system.maxLifeTime = 1.5;
    
    system.minSize = 0.5;
    system.maxSize = 2.0;
    
    system.emitRate = 300;
    
    // Fire colors (red/orange/yellow)
    this.setParticleColors(system, [1, 0.5, 0, 1], [1, 1, 0, 1], [0.5, 0, 0, 0]);
    
    // Upward movement with spread
    this.setParticleDirection(system, [0, 1, 0], [0.2, 1, 0.2]);
    this.setParticleSpeed(system, 2, 8, 0.01);
    
    // Gravity and forces
    this.setGravity(system, [0, -2, 0]);
    
    return system;
  }

  createSmokeParticles(name, position = [0, 0, 0], capacity = 1000) {
    const system = this.createParticleSystem(name, capacity);
    
    system.emitter = new Vector3(...position);
    this.setSphereEmitter(system, 0.2, 0.8);
    
    system.minLifeTime = 2.0;
    system.maxLifeTime = 4.0;
    
    system.minSize = 1.0;
    system.maxSize = 3.0;
    
    system.emitRate = 50;
    
    // Smoke colors (gray shades)
    this.setParticleColors(system, [0.8, 0.8, 0.8, 0.8], [0.4, 0.4, 0.4, 0.3], [0.2, 0.2, 0.2, 0]);
    
    // Slow upward drift
    this.setParticleDirection(system, [0, 1, 0], [0.5, 1, 0.5]);
    this.setParticleSpeed(system, 0.5, 2, 0.005);
    
    return system;
  }

  createRainParticles(name, position = [0, 10, 0], capacity = 5000) {
    const system = this.createParticleSystem(name, capacity);
    
    system.emitter = new Vector3(...position);
    this.setBoxEmitter(system, [-10, 0, -10], [10, 0, 10]);
    
    system.minLifeTime = 1.0;
    system.maxLifeTime = 3.0;
    
    system.minSize = 0.1;
    system.maxSize = 0.2;
    
    system.emitRate = 1000;
    
    // Blue water colors
    this.setParticleColors(system, [0.3, 0.6, 1, 0.8], [0.1, 0.4, 0.8, 0.6]);
    
    // Downward rain
    this.setParticleDirection(system, [0, -1, 0], [0.1, -1, 0.1]);
    this.setParticleSpeed(system, 10, 15, 0.02);
    
    this.setGravity(system, [0, -20, 0]);
    
    return system;
  }

  createSnowParticles(name, position = [0, 10, 0], capacity = 2000) {
    const system = this.createParticleSystem(name, capacity);
    
    system.emitter = new Vector3(...position);
    this.setBoxEmitter(system, [-15, 0, -15], [15, 0, 15]);
    
    system.minLifeTime = 5.0;
    system.maxLifeTime = 10.0;
    
    system.minSize = 0.3;
    system.maxSize = 0.8;
    
    system.emitRate = 200;
    
    // White snow
    this.setParticleColors(system, [1, 1, 1, 1], [0.9, 0.9, 1, 0.8]);
    
    // Gentle falling with wind
    this.setParticleDirection(system, [-0.2, -1, 0], [0.2, -1, 0.2]);
    this.setParticleSpeed(system, 1, 3, 0.005);
    
    this.setGravity(system, [0.5, -2, 0]); // Light gravity with wind
    
    return system;
  }

  createSparkParticles(name, position = [0, 0, 0], capacity = 500) {
    const system = this.createParticleSystem(name, capacity);
    
    system.emitter = new Vector3(...position);
    this.setPointEmitter(system, [0, 1, 0], [1, 1, 1]);
    
    system.minLifeTime = 0.2;
    system.maxLifeTime = 0.8;
    
    system.minSize = 0.1;
    system.maxSize = 0.3;
    
    system.emitRate = 500;
    
    // Bright spark colors
    this.setParticleColors(system, [1, 1, 0.5, 1], [1, 0.5, 0, 1], [0.5, 0, 0, 0]);
    
    // Explosive outward movement
    this.setParticleDirection(system, [-1, -1, -1], [1, 1, 1]);
    this.setParticleSpeed(system, 5, 15, 0.02);
    
    this.setGravity(system, [0, -10, 0]);
    
    return system;
  }

  // === PARTICLE BEHAVIORS ===

  setParticleRotationBehavior(system, minAngularSpeed = 0, maxAngularSpeed = 6.28) {
    if (!system) return false;
    system.minAngularSpeed = minAngularSpeed;
    system.maxAngularSpeed = maxAngularSpeed;
    return true;
  }

  setParticleGrowthBehavior(system, growthRate = 0) {
    if (!system) return false;
    
    // Add size gradient for growth
    system.removeSizeGradients();
    system.addSizeGradient(0, 0.1);
    system.addSizeGradient(0.5, 1.0);
    system.addSizeGradient(1.0, 0.1 + growthRate);
    
    return true;
  }

  setParticleFadeBehavior(system, fadeIn = true, fadeOut = true) {
    if (!system) return false;
    
    system.removeColorGradients();
    
    if (fadeIn && fadeOut) {
      system.addColorGradient(0, new Color4(1, 1, 1, 0));
      system.addColorGradient(0.2, new Color4(1, 1, 1, 1));
      system.addColorGradient(0.8, new Color4(1, 1, 1, 1));
      system.addColorGradient(1, new Color4(1, 1, 1, 0));
    } else if (fadeIn) {
      system.addColorGradient(0, new Color4(1, 1, 1, 0));
      system.addColorGradient(0.3, new Color4(1, 1, 1, 1));
    } else if (fadeOut) {
      system.addColorGradient(0.7, new Color4(1, 1, 1, 1));
      system.addColorGradient(1, new Color4(1, 1, 1, 0));
    }
    
    return true;
  }

  // === PARTICLE SYSTEM INFO ===

  getParticleSystemInfo(system) {
    if (!system) return null;
    
    return {
      name: system.name,
      capacity: system.getCapacity(),
      isStarted: system.isStarted(),
      isStopped: system.isStopped(),
      emitRate: system.emitRate,
      activeParticleCount: system.getActiveCount ? system.getActiveCount() : 'unknown',
      minLifeTime: system.minLifeTime,
      maxLifeTime: system.maxLifeTime,
      gravity: [system.gravity.x, system.gravity.y, system.gravity.z],
      blendMode: system.blendMode,
      updateSpeed: system.updateSpeed,
      isGPU: system instanceof GPUParticleSystem
    };
  }

  getAllParticleSystems() {
    return this.scene.particleSystems.map(system => ({
      name: system.name,
      capacity: system.getCapacity(),
      isStarted: system.isStarted(),
      isGPU: system instanceof GPUParticleSystem
    }));
  }

  // === PARTICLE OPTIMIZATION ===

  convertToGPUParticles(cpuSystem) {
    if (!cpuSystem || cpuSystem instanceof GPUParticleSystem) return null;
    
    // Create new GPU system with same properties
    const gpuSystem = new GPUParticleSystem(cpuSystem.name, { capacity: cpuSystem.getCapacity() }, this.scene);
    
    // Copy properties
    gpuSystem.particleTexture = cpuSystem.particleTexture;
    gpuSystem.emitter = cpuSystem.emitter;
    gpuSystem.minLifeTime = cpuSystem.minLifeTime;
    gpuSystem.maxLifeTime = cpuSystem.maxLifeTime;
    gpuSystem.minSize = cpuSystem.minSize;
    gpuSystem.maxSize = cpuSystem.maxSize;
    gpuSystem.emitRate = cpuSystem.emitRate;
    gpuSystem.gravity = cpuSystem.gravity.clone();
    gpuSystem.direction1 = cpuSystem.direction1.clone();
    gpuSystem.direction2 = cpuSystem.direction2.clone();
    gpuSystem.color1 = cpuSystem.color1.clone();
    gpuSystem.color2 = cpuSystem.color2.clone();
    gpuSystem.colorDead = cpuSystem.colorDead.clone();
    gpuSystem.minEmitPower = cpuSystem.minEmitPower;
    gpuSystem.maxEmitPower = cpuSystem.maxEmitPower;
    
    // Dispose old system
    cpuSystem.dispose();
    
    return gpuSystem;
  }

  // === PARTICLE EFFECTS COMBINATIONS ===

  createExplosionEffect(position = [0, 0, 0], intensity = 1.0) {
    const systems = [];
    
    // Main explosion flash
    const flash = this.createSparkParticles(`explosion_flash_${Date.now()}`, position, 300 * intensity);
    this.setParticleLifetime(flash, 0.1, 0.3);
    this.setParticleSpeed(flash, 20 * intensity, 40 * intensity, 0.05);
    systems.push(flash);
    
    // Smoke cloud
    const smoke = this.createSmokeParticles(`explosion_smoke_${Date.now()}`, position, 500 * intensity);
    this.setParticleLifetime(smoke, 3.0, 8.0);
    systems.push(smoke);
    
    // Debris
    const debris = this.createParticleSystem(`explosion_debris_${Date.now()}`, 200 * intensity);
    this.setBoxEmitter(debris, [-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]);
    this.setParticleColors(debris, [0.5, 0.3, 0.1, 1], [0.3, 0.2, 0.1, 1]);
    this.setParticleLifetime(debris, 2.0, 5.0);
    this.setParticleSpeed(debris, 5 * intensity, 25 * intensity, 0.02);
    this.setGravity(debris, [0, -15, 0]);
    systems.push(debris);
    
    // Start all systems
    systems.forEach(system => this.startParticleSystem(system));
    
    return systems;
  }

  createMagicEffect(position = [0, 0, 0], color = [0.5, 0, 1]) {
    const system = this.createParticleSystem(`magic_${Date.now()}`, 800);
    
    system.emitter = new Vector3(...position);
    this.setSphereEmitter(system, 2, 1);
    
    system.minLifeTime = 1.0;
    system.maxLifeTime = 3.0;
    
    system.minSize = 0.2;
    system.maxSize = 1.0;
    
    system.emitRate = 150;
    
    // Magic sparkle colors
    const magicColor1 = [...color, 1.0];
    const magicColor2 = [color[0] * 0.5, color[1] * 0.5, color[2] * 0.5, 0.5];
    this.setParticleColors(system, magicColor1, magicColor2, [0, 0, 0, 0]);
    
    // Swirling upward movement
    this.setParticleDirection(system, [-0.5, 1, -0.5], [0.5, 1, 0.5]);
    this.setParticleSpeed(system, 1, 5, 0.01);
    
    // Add rotation
    this.setParticleRotationBehavior(system, -3.14, 3.14);
    
    // Gentle upward force
    this.setGravity(system, [0, 2, 0]);
    
    this.startParticleSystem(system);
    return system;
  }

  createWaterfallEffect(position = [0, 5, 0], width = 2) {
    const system = this.createParticleSystem(`waterfall_${Date.now()}`, 3000);
    
    system.emitter = new Vector3(...position);
    this.setBoxEmitter(system, [-width, 0, -0.1], [width, 0, 0.1]);
    
    system.minLifeTime = 1.0;
    system.maxLifeTime = 2.5;
    
    system.minSize = 0.1;
    system.maxSize = 0.3;
    
    system.emitRate = 1000;
    
    // Water colors
    this.setParticleColors(system, [0.6, 0.8, 1, 0.8], [0.3, 0.6, 1, 0.4]);
    
    // Downward flow
    this.setParticleDirection(system, [0, -1, 0], [0.1, -1, 0.1]);
    this.setParticleSpeed(system, 8, 12, 0.02);
    
    this.setGravity(system, [0, -25, 0]);
    
    this.startParticleSystem(system);
    return system;
  }

  // === PARTICLE SYSTEM MANAGEMENT ===

  findParticleSystemByName(name) {
    return this.scene.particleSystems.find(system => system.name === name) || null;
  }

  getAllParticleSystemsInfo() {
    return this.scene.particleSystems.map(system => this.getParticleSystemInfo(system));
  }

  disposeAllParticleSystems() {
    const systems = [...this.scene.particleSystems];
    systems.forEach(system => system.dispose());
    return true;
  }
}