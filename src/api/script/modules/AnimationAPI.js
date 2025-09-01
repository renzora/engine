// === ANIMATION API MODULE ===

import {
  Animation,
  AnimationGroup,
  AnimationRange,
  Skeleton,
  Bone,
  AnimationPropertiesOverride,
  EasingFunction,
  BezierCurveEase,
  CircleEase,
  BackEase,
  BounceEase,
  CubicEase,
  ElasticEase,
  ExponentialEase,
  PowerEase,
  QuadraticEase,
  QuarticEase,
  QuinticEase,
  SineEase,
  Vector3,
  Color3,
  Quaternion,
  Matrix
} from '@babylonjs/core';

import { MorphTarget, MorphTargetManager } from '@babylonjs/core/Morph/index.js';

export class AnimationAPI {
  constructor(scene, babylonObject = null) {
    this.scene = scene;
    this.mesh = babylonObject; // Set the current object as the default mesh
  }

  // === BASIC ANIMATION CREATION ===

  createAnimation(name, targetProperty, frameRate = 60, dataType = Animation.ANIMATIONTYPE_FLOAT) {
    return new Animation(name, targetProperty, frameRate, dataType, Animation.ANIMATIONLOOPMODE_CYCLE);
  }

  createVectorAnimation(name, targetProperty, frameRate = 60) {
    return new Animation(name, targetProperty, frameRate, Animation.ANIMATIONTYPE_VECTOR3, Animation.ANIMATIONLOOPMODE_CYCLE);
  }

  createColorAnimation(name, targetProperty, frameRate = 60) {
    return new Animation(name, targetProperty, frameRate, Animation.ANIMATIONTYPE_COLOR3, Animation.ANIMATIONLOOPMODE_CYCLE);
  }

  createQuaternionAnimation(name, targetProperty, frameRate = 60) {
    return new Animation(name, targetProperty, frameRate, Animation.ANIMATIONTYPE_QUATERNION, Animation.ANIMATIONLOOPMODE_CYCLE);
  }

  // === ANIMATION KEYFRAMES ===

  addAnimationKeys(animation, keyframes) {
    if (!animation || !keyframes) return false;
    
    const keys = keyframes.map(keyframe => ({
      frame: keyframe.frame,
      value: this.parseAnimationValue(keyframe.value, animation.dataType),
      inTangent: keyframe.inTangent,
      outTangent: keyframe.outTangent
    }));
    
    animation.setKeys(keys);
    return true;
  }

  parseAnimationValue(value, dataType) {
    switch (dataType) {
      case Animation.ANIMATIONTYPE_VECTOR3:
        return Array.isArray(value) ? new Vector3(...value) : value;
      case Animation.ANIMATIONTYPE_COLOR3:
        return Array.isArray(value) ? new Color3(...value) : value;
      case Animation.ANIMATIONTYPE_QUATERNION:
        return Array.isArray(value) ? new Quaternion(...value) : value;
      default:
        return value;
    }
  }

  // === ANIMATION PLAYBACK ===

  playAnimation(target, animations, from = 0, to = 60, loop = true, speedRatio = 1.0) {
    if (!target || !animations) return null;
    
    const animationArray = Array.isArray(animations) ? animations : [animations];
    
    return this.scene.beginAnimation(
      target, 
      from, 
      to, 
      loop, 
      speedRatio,
      null, // onAnimationEnd
      animationArray
    );
  }

  stopAnimation(target) {
    if (!target) return false;
    this.scene.stopAnimation(target);
    return true;
  }

  pauseAnimation(target) {
    if (!target) return false;
    this.scene.pauseAnimation(target);
    return true;
  }

  restartAnimation(target) {
    if (!target) return false;
    this.scene.restartAnimation(target);
    return true;
  }

  // === EASING FUNCTIONS ===

  createBezierEase(x1 = 0, y1 = 0, x2 = 1, y2 = 1) {
    return new BezierCurveEase(x1, y1, x2, y2);
  }

  createCircleEase() {
    return new CircleEase();
  }

  createBackEase(amplitude = 1) {
    return new BackEase(amplitude);
  }

  createBounceEase(bounces = 3, bounciness = 2) {
    return new BounceEase(bounces, bounciness);
  }

  createElasticEase(oscillations = 3, springiness = 3) {
    return new ElasticEase(oscillations, springiness);
  }

  createExponentialEase(exponent = 2) {
    return new ExponentialEase(exponent);
  }

  createPowerEase(power = 2) {
    return new PowerEase(power);
  }

  setEasingMode(easingFunction, mode) {
    if (!easingFunction) return false;
    // EasingFunction.EASINGMODE_EASEIN = 0
    // EasingFunction.EASINGMODE_EASEOUT = 1  
    // EasingFunction.EASINGMODE_EASEINOUT = 2
    easingFunction.setEasingMode(mode);
    return true;
  }

  // === ANIMATION GROUPS ===

  createAnimationGroup(name, scene = null) {
    return new AnimationGroup(name, scene || this.scene);
  }

  addAnimationToGroup(animationGroup, animation) {
    if (!animationGroup || !animation) return false;
    animationGroup.addTargetedAnimation(animation.animation, animation.target);
    return true;
  }

  playAnimationGroup(animationGroup, loop = false, speedRatio = 1.0) {
    if (!animationGroup) return false;
    animationGroup.play(loop);
    if (speedRatio !== 1.0) {
      animationGroup.speedRatio = speedRatio;
    }
    return true;
  }

  stopAnimationGroup(animationGroup) {
    if (!animationGroup) return false;
    animationGroup.stop();
    return true;
  }

  pauseAnimationGroup(animationGroup) {
    if (!animationGroup) return false;
    animationGroup.pause();
    return true;
  }

  resetAnimationGroup(animationGroup) {
    if (!animationGroup) return false;
    animationGroup.reset();
    return true;
  }

  // === SKELETON ANIMATION ===

  createSkeleton(name, bones, scene = null) {
    const skeleton = new Skeleton(name, '', scene || this.scene);
    bones.forEach(boneData => {
      const bone = new Bone(boneData.name, skeleton, boneData.parent || null, boneData.matrix);
      if (boneData.rest) {
        bone.setRestPose(boneData.rest);
      }
    });
    return skeleton;
  }

  playSkeletonAnimation(skeleton, name, loop = true, speedRatio = 1.0, onAnimationEnd = null) {
    if (!skeleton) return null;
    
    const animationRange = skeleton.getAnimationRange(name);
    if (!animationRange) {
      console.warn(`Animation range '${name}' not found in skeleton`);
      return null;
    }
    
    return this.scene.beginAnimation(
      skeleton,
      animationRange.from,
      animationRange.to,
      loop,
      speedRatio,
      onAnimationEnd
    );
  }

  stopSkeletonAnimation(skeleton) {
    if (!skeleton) return false;
    this.scene.stopAnimation(skeleton);
    return true;
  }

  createAnimationRange(skeleton, name, from, to) {
    if (!skeleton) return false;
    skeleton.createAnimationRange(name, from, to);
    return true;
  }

  deleteAnimationRange(skeleton, name) {
    if (!skeleton) return false;
    skeleton.deleteAnimationRange(name);
    return true;
  }

  getSkeletonAnimationRanges(skeleton) {
    if (!skeleton) return [];
    return skeleton.getAnimationRanges();
  }

  // === BONE MANIPULATION ===

  getBoneByName(skeleton, name) {
    if (!skeleton) return null;
    return skeleton.bones.find(bone => bone.name === name);
  }

  setBoneTransform(bone, position, rotation, scaling) {
    if (!bone) return false;
    
    const matrix = Matrix.Compose(
      new Vector3(...scaling),
      Quaternion.RotationYawPitchRoll(...rotation),
      new Vector3(...position)
    );
    
    bone.setLocalMatrix(matrix);
    return true;
  }

  getBoneWorldMatrix(bone) {
    if (!bone) return null;
    const matrix = bone.getWorldMatrix();
    return matrix.asArray();
  }

  attachMeshToBone(mesh, bone, skeleton) {
    if (!mesh || !bone || !skeleton) return false;
    mesh.attachToBone(bone, skeleton);
    return true;
  }

  // === MORPH TARGET ANIMATION ===

  createMorphTargetManager(mesh) {
    if (!mesh) return null;
    
    if (!mesh.morphTargetManager) {
      mesh.morphTargetManager = new MorphTargetManager(this.scene);
    }
    return mesh.morphTargetManager;
  }

  addMorphTarget(mesh, name, positions, normals = null, uvs = null) {
    if (!mesh || !positions) return null;
    
    const manager = this.createMorphTargetManager(mesh);
    const target = MorphTarget.FromMesh(mesh, name);
    
    if (positions) target.setPositions(positions);
    if (normals) target.setNormals(normals);
    if (uvs) target.setUVs(uvs);
    
    manager.addTarget(target);
    return target;
  }

  setMorphTargetInfluence(mesh, targetIndex, influence) {
    if (!mesh || !mesh.morphTargetManager) return false;
    
    const target = mesh.morphTargetManager.getTarget(targetIndex);
    if (target) {
      target.influence = Math.max(0, Math.min(1, influence));
      return true;
    }
    return false;
  }

  animateMorphTarget(mesh, targetIndex, influence, duration = 1000) {
    if (!mesh || !mesh.morphTargetManager) return null;
    
    const target = mesh.morphTargetManager.getTarget(targetIndex);
    if (!target) return null;
    
    const animation = this.createAnimation(
      `morph_${targetIndex}_influence`,
      'influence',
      60,
      Animation.ANIMATIONTYPE_FLOAT
    );
    
    this.addAnimationKeys(animation, [
      { frame: 0, value: target.influence },
      { frame: Math.floor((duration / 1000) * 60), value: influence }
    ]);
    
    target.animations = [animation];
    return this.playAnimation(target, animation);
  }

  // === ANIMATION BLENDING ===

  blendAnimations(target, fromAnimation, toAnimation, blendTime = 1.0) {
    if (!target || !fromAnimation || !toAnimation) return false;
    
    // Stop current animation
    this.scene.stopAnimation(target);
    
    // Create blend group
    const blendGroup = new AnimationGroup(`blend_${target.name}`, this.scene);
    
    // Add animations with weight
    blendGroup.addTargetedAnimation(fromAnimation, target);
    blendGroup.addTargetedAnimation(toAnimation, target);
    
    // Set blend weights
    blendGroup.setWeightForAllAnimatables(target, 1.0 - (1.0 / blendTime));
    
    blendGroup.play(false);
    return true;
  }

  // === PROCEDURAL ANIMATION ===

  animateAlongPath(mesh, path, duration = 5000, loop = false) {
    if (!mesh || !path || path.length === 0) return null;
    
    const pathVectors = path.map(p => new Vector3(...p));
    const curve = new Curve3(pathVectors);
    const frameRate = 60;
    const totalFrames = Math.floor((duration / 1000) * frameRate);
    
    const animation = this.createVectorAnimation(`${mesh.name}_path`, 'position', frameRate);
    
    const keys = [];
    for (let i = 0; i <= totalFrames; i++) {
      const t = i / totalFrames;
      const point = curve.getPointAt(t);
      keys.push({ frame: i, value: point });
    }
    
    this.addAnimationKeys(animation, keys);
    mesh.animations = [animation];
    
    return this.scene.beginAnimation(
      mesh, 
      0, 
      totalFrames, 
      loop, 
      1.0
    );
  }

  animateRotationAroundAxis(mesh, axis, angle, duration = 2000, loop = true) {
    if (!mesh || !axis) return null;
    
    const frameRate = 60;
    const totalFrames = Math.floor((duration / 1000) * frameRate);
    const axisVector = new Vector3(...axis);
    
    const animation = this.createQuaternionAnimation(`${mesh.name}_rotation`, 'rotationQuaternion', frameRate);
    
    const keys = [];
    for (let i = 0; i <= totalFrames; i++) {
      const t = (i / totalFrames) * angle;
      const quaternion = Quaternion.RotationAxis(axisVector, t);
      keys.push({ frame: i, value: quaternion });
    }
    
    this.addAnimationKeys(animation, keys);
    mesh.animations = [animation];
    
    return this.scene.beginAnimation(mesh, 0, totalFrames, loop, 1.0);
  }

  animateScale(mesh, fromScale, toScale, duration = 1000, loop = false) {
    if (!mesh || !fromScale || !toScale) return null;
    
    const frameRate = 60;
    const totalFrames = Math.floor((duration / 1000) * frameRate);
    
    const animation = this.createVectorAnimation(`${mesh.name}_scale`, 'scaling', frameRate);
    
    this.addAnimationKeys(animation, [
      { frame: 0, value: fromScale },
      { frame: totalFrames, value: toScale }
    ]);
    
    mesh.animations = [animation];
    return this.scene.beginAnimation(mesh, 0, totalFrames, loop, 1.0);
  }

  animateOpacity(mesh, fromAlpha, toAlpha, duration = 1000, loop = false) {
    if (!mesh) return null;
    
    const frameRate = 60;
    const totalFrames = Math.floor((duration / 1000) * frameRate);
    
    const animation = this.createAnimation(`${mesh.name}_alpha`, 'visibility', frameRate);
    
    this.addAnimationKeys(animation, [
      { frame: 0, value: fromAlpha },
      { frame: totalFrames, value: toAlpha }
    ]);
    
    mesh.animations = [animation];
    return this.scene.beginAnimation(mesh, 0, totalFrames, loop, 1.0);
  }

  // === SKELETON FUNCTIONS FOR RENSCRIPT ===

  hasSkeletonAPI(mesh) {
    return mesh && mesh.skeleton;
  }

  playIdleAnimation(mesh) {
    if (!mesh || !mesh.skeleton) return false;
    
    const idle = mesh.skeleton.getAnimationRange('idle');
    if (idle) {
      this.scene.beginAnimation(mesh.skeleton, idle.from, idle.to, true, 1.0);
      return true;
    }
    
    // Try common idle animation names
    const commonIdles = ['Idle', 'idle', 'IDLE', 'Armature|Idle', 'mixamo.com'];
    for (const name of commonIdles) {
      const range = mesh.skeleton.getAnimationRange(name);
      if (range) {
        this.scene.beginAnimation(mesh.skeleton, range.from, range.to, true, 1.0);
        return true;
      }
    }
    
    console.warn('No idle animation found');
    return false;
  }

  playWalkAnimation(mesh) {
    if (!mesh || !mesh.skeleton) return false;
    
    const walk = mesh.skeleton.getAnimationRange('walk');
    if (walk) {
      this.scene.beginAnimation(mesh.skeleton, walk.from, walk.to, true, 1.0);
      return true;
    }
    
    // Try common walk animation names
    const commonWalks = ['Walk', 'walk', 'WALK', 'Walking', 'Armature|Walk'];
    for (const name of commonWalks) {
      const range = mesh.skeleton.getAnimationRange(name);
      if (range) {
        this.scene.beginAnimation(mesh.skeleton, range.from, range.to, true, 1.0);
        return true;
      }
    }
    
    console.warn('No walk animation found');
    return false;
  }

  playRunAnimation(mesh) {
    if (!mesh || !mesh.skeleton) return false;
    
    const run = mesh.skeleton.getAnimationRange('run');
    if (run) {
      this.scene.beginAnimation(mesh.skeleton, run.from, run.to, true, 1.0);
      return true;
    }
    
    // Try common run animation names
    const commonRuns = ['Run', 'run', 'RUN', 'Running', 'Armature|Run', 'Sprint'];
    for (const name of commonRuns) {
      const range = mesh.skeleton.getAnimationRange(name);
      if (range) {
        this.scene.beginAnimation(mesh.skeleton, range.from, range.to, true, 1.0);
        return true;
      }
    }
    
    console.warn('No run animation found');
    return false;
  }

  playJumpAnimation(mesh) {
    if (!mesh || !mesh.skeleton) return false;
    
    const jump = mesh.skeleton.getAnimationRange('jump');
    if (jump) {
      this.scene.beginAnimation(mesh.skeleton, jump.from, jump.to, false, 1.0);
      return true;
    }
    
    // Try common jump animation names
    const commonJumps = ['Jump', 'jump', 'JUMP', 'Armature|Jump', 'Leap'];
    for (const name of commonJumps) {
      const range = mesh.skeleton.getAnimationRange(name);
      if (range) {
        this.scene.beginAnimation(mesh.skeleton, range.from, range.to, false, 1.0);
        return true;
      }
    }
    
    console.warn('No jump animation found');
    return false;
  }

  stopAnimationAPI(mesh) {
    if (!mesh) return false;
    
    if (mesh.skeleton) {
      this.scene.stopAnimation(mesh.skeleton);
    }
    this.scene.stopAnimation(mesh);
    return true;
  }

  setAnimationSpeedAPI(mesh, speed) {
    if (!mesh) return false;
    
    // Set speed for skeleton animations
    if (mesh.skeleton) {
      const skeletonAnimatable = this.scene.getAnimatableByTarget(mesh.skeleton);
      if (skeletonAnimatable) {
        skeletonAnimatable.speedRatio = speed;
      }
    }
    
    // Set speed for mesh animations
    const meshAnimatable = this.scene.getAnimatableByTarget(mesh);
    if (meshAnimatable) {
      meshAnimatable.speedRatio = speed;
    }
    
    return true;
  }

  // === ANIMATION WEIGHT AND BLENDING ===

  setAnimationWeight(target, weight) {
    if (!target) return false;
    
    const animatable = this.scene.getAnimatableByTarget(target);
    if (animatable) {
      animatable.weight = Math.max(0, Math.min(1, weight));
      return true;
    }
    return false;
  }

  blendToAnimation(target, animationName, blendTime = 0.3) {
    if (!target) return false;
    
    // For skeletons
    if (target.getAnimationRange) {
      const range = target.getAnimationRange(animationName);
      if (range) {
        const current = this.scene.getAnimatableByTarget(target);
        if (current) {
          current.goToFrame(range.from);
          return this.scene.beginWeightedAnimation(target, range.from, range.to, 1.0, true, 1.0, null, null, blendTime);
        }
      }
    }
    
    return false;
  }

  // === ANIMATION EVENTS ===

  addAnimationEvent(animation, frame, action) {
    if (!animation || !action) return false;
    
    animation.addEvent(new AnimationEvent(frame, action));
    return true;
  }

  removeAnimationEvents(animation) {
    if (!animation) return false;
    animation.removeEvents();
    return true;
  }

  // === ANIMATION UTILITIES ===

  getAnimationProgress(target) {
    if (!target) return 0;
    
    const animatable = this.scene.getAnimatableByTarget(target);
    if (animatable && animatable._animations.length > 0) {
      const anim = animatable._animations[0];
      const progress = (anim.currentFrame - anim.fromFrame) / (anim.toFrame - anim.fromFrame);
      return Math.max(0, Math.min(1, progress));
    }
    return 0;
  }

  isAnimationPlaying(target) {
    if (!target) return false;
    
    const animatable = this.scene.getAnimatableByTarget(target);
    return animatable && !animatable.paused;
  }

  getAllAnimations(target) {
    if (!target) target = this.mesh;
    if (!target) return [];
    
    const animations = [];
    
    // Get mesh animations
    if (target.animations) {
      animations.push(...target.animations.map(anim => anim.name));
    }
    
    // Get skeleton animation ranges
    if (target.skeleton && target.skeleton.getAnimationRanges) {
      const ranges = target.skeleton.getAnimationRanges();
      animations.push(...ranges.map(range => range.name));
    }
    
    // Check for animation groups (common in GLTF/GLB files)
    if (target.metadata && target.metadata.animationGroups) {
      target.metadata.animationGroups.forEach(group => {
        animations.push(group.name);
      });
    }
    
    // Also check scene animation groups
    if (this.scene && this.scene.animationGroups) {
      this.scene.animationGroups.forEach(group => {
        // Check if this animation group targets our mesh
        const targetsOurMesh = group.targetedAnimations.some(ta => ta.target === target || ta.target === target.skeleton);
        if (targetsOurMesh) {
          animations.push(group.name);
        }
      });
    }
    
    return animations;
  }

  // === PHYSICS ANIMATION ===

  animateWithPhysics(mesh, force, torque = null) {
    if (!mesh || !mesh.physicsImpostor) return false;
    
    const forceVector = new Vector3(...force);
    mesh.physicsImpostor.applyImpulse(forceVector, mesh.getAbsolutePosition());
    
    if (torque) {
      const torqueVector = new Vector3(...torque);
      mesh.physicsImpostor.setAngularVelocity(torqueVector);
    }
    
    return true;
  }

  // === ANIMATION CURVES ===

  createAnimationCurve(points) {
    if (!points || points.length < 2) return null;
    
    const pathPoints = points.map(p => new Vector3(...p));
    return new Curve3(pathPoints);
  }

  getCurvePoint(curve, t) {
    if (!curve) return null;
    const point = curve.getPointAt(Math.max(0, Math.min(1, t)));
    return [point.x, point.y, point.z];
  }

  getCurveTangent(curve, t) {
    if (!curve) return null;
    const tangent = curve.getTangentAt(Math.max(0, Math.min(1, t)));
    return [tangent.x, tangent.y, tangent.z];
  }

  // === SMART ANIMATION PLAYER ===

  playAnimationByName(animationName, loop = true, speedRatio = 1.0) {
    if (!animationName || animationName === "none") return false;
    
    // First try to find it as an animation group in the scene
    if (this.scene && this.scene.animationGroups) {
      const animationGroup = this.scene.animationGroups.find(group => group.name === animationName);
      if (animationGroup) {
        console.log(`Found animation group: ${animationName}, checking targets...`);
        console.log(`this.mesh:`, this.mesh);
        console.log(`Animation group targeted animations:`, animationGroup.targetedAnimations.map(ta => ({target: ta.target, targetName: ta.target?.name})));
        
        // Check if this animation group targets our mesh or skeleton
        const targetsOurMesh = animationGroup.targetedAnimations.some(ta => 
          ta.target === this.mesh || 
          (this.mesh && ta.target === this.mesh.skeleton) ||
          // Also check by name if direct reference fails
          (this.mesh && ta.target && ta.target.name === this.mesh.name) ||
          // Check if any child of our TransformNode is targeted
          (this.mesh && this.mesh.getChildren && this.mesh.getChildren().includes(ta.target)) ||
          // Check if target is a skeleton of any child mesh
          (this.mesh && this.mesh.getChildren && this.mesh.getChildren().some(child => 
            child.skeleton && child.skeleton === ta.target
          ))
        );
        
        // For TransformNodes with skeletal animations, be less restrictive
        if (targetsOurMesh || animationGroup.targetedAnimations.length === 0 || 
            (this.mesh && this.mesh.getClassName && this.mesh.getClassName() === 'TransformNode')) {
          console.log(`Playing animation group: ${animationName}`);
          animationGroup.play(loop);
          if (speedRatio !== 1.0) {
            animationGroup.speedRatio = speedRatio;
          }
          return true;
        } else {
          console.log(`Animation group ${animationName} doesn't target our mesh`);
        }
      } else {
        console.log(`Animation group ${animationName} not found in scene.animationGroups`);
      }
    }
    
    // Then try as skeleton animation range
    if (this.mesh && this.mesh.skeleton) {
      console.log(`Checking skeleton animation ranges for: ${animationName}`);
      const allRanges = this.mesh.skeleton.getAnimationRanges();
      console.log(`Available skeleton ranges:`, allRanges.map(r => r.name));
      
      const animationRange = this.mesh.skeleton.getAnimationRange(animationName);
      if (animationRange) {
        console.log(`Playing skeleton animation range: ${animationName}`);
        this.scene.beginAnimation(
          this.mesh.skeleton,
          animationRange.from,
          animationRange.to,
          loop,
          speedRatio
        );
        return true;
      } else {
        console.log(`Skeleton animation range ${animationName} not found`);
      }
    } else {
      console.log(`No mesh or skeleton available for animation: ${animationName}`);
    }
    
    // Finally try as mesh animation
    if (this.mesh && this.mesh.animations) {
      const meshAnimation = this.mesh.animations.find(anim => anim.name === animationName);
      if (meshAnimation) {
        console.log(`Playing mesh animation: ${animationName}`);
        this.scene.beginAnimation(this.mesh, 0, 100, loop, speedRatio, null, [meshAnimation]);
        return true;
      }
    }
    
    console.warn(`Animation '${animationName}' not found in any format`);
    return false;
  }

  // === ANIMATION INFO ===

  getAnimationInfo(target) {
    if (!target) return null;
    
    const animatable = this.scene.getAnimatableByTarget(target);
    if (!animatable) return null;
    
    return {
      isPlaying: !animatable.paused,
      currentFrame: animatable._animations[0] ? animatable._animations[0].currentFrame : 0,
      totalFrames: animatable._animations[0] ? animatable._animations[0].toFrame - animatable._animations[0].fromFrame : 0,
      speedRatio: animatable.speedRatio,
      weight: animatable.weight || 1.0,
      looping: animatable.loopAnimation
    };
  }

  // === SIMPLE ANIMATION HELPERS ===

  animatePosition(x, y, z, duration = 1000, easing = null) {
    const target = this.mesh;
    if (!target) return null;

    const animation = this.createVectorAnimation(`${target.name}_position`, 'position');
    const keys = [
      { frame: 0, value: target.position.clone() },
      { frame: 60, value: new Vector3(x, y, z) }
    ];
    
    this.addAnimationKeys(animation, keys);
    if (easing) animation.setEasingFunction(easing);
    
    return this.scene.beginAnimation(target, 0, 60, false, 60000 / duration);
  }

  animateRotation(x, y, z, duration = 1000, easing = null) {
    const target = this.mesh;
    if (!target) return null;

    const animation = this.createVectorAnimation(`${target.name}_rotation`, 'rotation');
    const keys = [
      { frame: 0, value: target.rotation.clone() },
      { frame: 60, value: new Vector3(x, y, z) }
    ];
    
    this.addAnimationKeys(animation, keys);
    if (easing) animation.setEasingFunction(easing);
    
    return this.scene.beginAnimation(target, 0, 60, false, 60000 / duration);
  }

  animateScale(x, y, z, duration = 1000, easing = null) {
    const target = this.mesh;
    if (!target) return null;

    const animation = this.createVectorAnimation(`${target.name}_scale`, 'scaling');
    const keys = [
      { frame: 0, value: target.scaling.clone() },
      { frame: 60, value: new Vector3(x, y, z) }
    ];
    
    this.addAnimationKeys(animation, keys);
    if (easing) animation.setEasingFunction(easing);
    
    return this.scene.beginAnimation(target, 0, 60, false, 60000 / duration);
  }

  animateColor(r, g, b, duration = 1000, easing = null) {
    const target = this.mesh;
    if (!target || !target.material) return null;

    const animation = this.createColorAnimation(`${target.name}_color`, 'material.diffuseColor');
    const keys = [
      { frame: 0, value: target.material.diffuseColor.clone() },
      { frame: 60, value: new Color3(r, g, b) }
    ];
    
    this.addAnimationKeys(animation, keys);
    if (easing) animation.setEasingFunction(easing);
    
    return this.scene.beginAnimation(target, 0, 60, false, 60000 / duration);
  }

  animateAlpha(alpha, duration = 1000, easing = null) {
    const target = this.mesh;
    if (!target || !target.material) return null;

    const animation = this.createAnimation(`${target.name}_alpha`, 'material.alpha');
    const keys = [
      { frame: 0, value: target.material.alpha },
      { frame: 60, value: alpha }
    ];
    
    this.addAnimationKeys(animation, keys);
    if (easing) animation.setEasingFunction(easing);
    
    return this.scene.beginAnimation(target, 0, 60, false, 60000 / duration);
  }

  // === SHORT NAME ALIASES ===
  
  skeletonAnimationRanges(skeleton) {
    return this.getSkeletonAnimationRanges(skeleton);
  }
  
  boneByName(skeleton, name) {
    return this.getBoneByName(skeleton, name);
  }
  
  boneWorldMatrix(bone) {
    return this.getBoneWorldMatrix(bone);
  }
  
  animationProgress(target) {
    return this.getAnimationProgress(target);
  }
  
  allAnimations(target) {
    return this.getAllAnimations(target);
  }
  
  curvePoint(curve, t) {
    return this.getCurvePoint(curve, t);
  }
  
  curveTangent(curve, t) {
    return this.getCurveTangent(curve, t);
  }
  
  animationInfo(target) {
    return this.getAnimationInfo(target);
  }
}