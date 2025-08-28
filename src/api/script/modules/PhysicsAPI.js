import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';
import { PhysicsImpostor } from '@babylonjs/core/Physics/physicsImpostor.js';
import { StandardMaterial, Color3 } from '@babylonjs/core';

/**
 * PhysicsAPI - Physics engine integration (Havok, Cannon.js, Ammo.js)
 * Priority: HIGH - Essential for realistic object behavior
 */
export class PhysicsAPI {
  constructor(scene, babylonObject) {
    this.scene = scene;
    this.babylonObject = babylonObject;
  }

  // === PHYSICS ENGINE SETUP ===
  
  async enablePhysics(engine = 'cannon', gravityX = 0, gravityY = -9.81, gravityZ = 0) {
    if (!this.scene) return false;
    
    try {
      const gravity = new Vector3(gravityX, gravityY, gravityZ);
      
      // Import appropriate physics engine
      let physicsPlugin;
      switch (engine.toLowerCase()) {
        case 'havok':
          try {
            // Try to enable Havok if available
            if (typeof HavokPlugin !== 'undefined' && typeof HavokPhysics !== 'undefined') {
              physicsPlugin = new HavokPlugin(true, await HavokPhysics());
            } else {
              console.warn('Havok physics not available, using basic physics simulation');
              this.scene.gravity = gravity;
              return true;
            }
          } catch (error) {
            console.warn('Failed to initialize Havok, using basic physics');
            this.scene.gravity = gravity;
            return true;
          }
          break;
        case 'cannon':
          try {
            if (typeof CannonJSPlugin !== 'undefined' && typeof CANNON !== 'undefined') {
              physicsPlugin = new CannonJSPlugin(true, 10, CANNON);
            } else {
              console.warn('Cannon.js not available, using basic physics simulation');
              this.scene.gravity = gravity;
              return true;
            }
          } catch (error) {
            this.scene.gravity = gravity;
            return true;
          }
          break;
        case 'ammo':
          try {
            if (typeof AmmoJSPlugin !== 'undefined' && typeof Ammo !== 'undefined') {
              physicsPlugin = new AmmoJSPlugin(true, Ammo);
            } else {
              console.warn('Ammo.js not available, using basic physics simulation');
              this.scene.gravity = gravity;
              return true;
            }
          } catch (error) {
            this.scene.gravity = gravity;
            return true;
          }
          break;
        default:
          console.warn('Unknown physics engine:', engine, '- using basic simulation');
          this.scene.gravity = gravity;
          return true;
      }
      
      if (physicsPlugin) {
        this.scene.enablePhysics(gravity, physicsPlugin);
      } else {
        this.scene.gravity = gravity;
      }
      
      return true;
    } catch (error) {
      console.error('Failed to enable physics:', error);
      return false;
    }
  }

  disablePhysics() {
    if (!this.scene) return;
    
    if (this.scene.physicsEnabled) {
      this.scene.disablePhysicsEngine();
    }
  }

  isPhysicsEnabled() {
    return this.scene?.physicsEnabled || false;
  }

  setGravity(x, y, z) {
    if (!this.scene?.physicsEnabled) return;
    
    const physicsEngine = this.scene.getPhysicsEngine();
    if (physicsEngine) {
      physicsEngine.setGravity(new Vector3(x, y, z));
    }
  }

  getGravity() {
    if (!this.scene?.physicsEnabled) return [0, -9.81, 0];
    
    const physicsEngine = this.scene.getPhysicsEngine();
    if (physicsEngine) {
      const gravity = physicsEngine.gravity;
      return [gravity.x, gravity.y, gravity.z];
    }
    
    return [0, -9.81, 0];
  }

  // === PHYSICS IMPOSTORS ===
  
  setPhysicsImpostor(type = 'box', mass = 1, options = {}) {
    if (!this.babylonObject) return;
    
    let impostorType;
    switch (type.toLowerCase()) {
      case 'box':
        impostorType = PhysicsImpostor.BoxImpostor;
        break;
      case 'sphere':
        impostorType = PhysicsImpostor.SphereImpostor;
        break;
      case 'cylinder':
        impostorType = PhysicsImpostor.CylinderImpostor;
        break;
      case 'plane':
        impostorType = PhysicsImpostor.PlaneImpostor;
        break;
      case 'mesh':
        impostorType = PhysicsImpostor.MeshImpostor;
        break;
      case 'convex_hull':
        impostorType = PhysicsImpostor.ConvexHullImpostor;
        break;
      default:
        impostorType = PhysicsImpostor.BoxImpostor;
    }
    
    const impostor = new PhysicsImpostor(this.babylonObject, impostorType, { 
      mass: mass,
      ...options 
    }, this.scene);
    
    return impostor;
  }

  removePhysicsImpostor() {
    if (!this.babylonObject?.physicsImpostor) return;
    
    this.babylonObject.physicsImpostor.dispose();
    this.babylonObject.physicsImpostor = null;
  }

  hasPhysicsImpostor() {
    return !!this.babylonObject?.physicsImpostor;
  }

  // === PHYSICS FORCES ===
  
  applyImpulse(forceX, forceY, forceZ, contactPointX = 0, contactPointY = 0, contactPointZ = 0) {
    if (!this.babylonObject?.physicsImpostor) return;
    
    const force = new Vector3(forceX, forceY, forceZ);
    const contactPoint = new Vector3(contactPointX, contactPointY, contactPointZ);
    
    this.babylonObject.physicsImpostor.applyImpulse(force, contactPoint);
  }

  applyForce(forceX, forceY, forceZ, contactPointX = 0, contactPointY = 0, contactPointZ = 0) {
    if (!this.babylonObject?.physicsImpostor) return;
    
    const force = new Vector3(forceX, forceY, forceZ);
    const contactPoint = new Vector3(contactPointX, contactPointY, contactPointZ);
    
    // Apply continuous force (would need to be called each frame)
    this.babylonObject.physicsImpostor.setLinearVelocity(
      this.babylonObject.physicsImpostor.getLinearVelocity().add(force)
    );
  }

  setLinearVelocity(velocityX, velocityY, velocityZ) {
    if (!this.babylonObject?.physicsImpostor) return;
    
    const velocity = new Vector3(velocityX, velocityY, velocityZ);
    this.babylonObject.physicsImpostor.setLinearVelocity(velocity);
  }

  getLinearVelocity() {
    if (!this.babylonObject?.physicsImpostor) return [0, 0, 0];
    
    const velocity = this.babylonObject.physicsImpostor.getLinearVelocity();
    return [velocity.x, velocity.y, velocity.z];
  }

  setAngularVelocity(velocityX, velocityY, velocityZ) {
    if (!this.babylonObject?.physicsImpostor) return;
    
    const velocity = new Vector3(velocityX, velocityY, velocityZ);
    this.babylonObject.physicsImpostor.setAngularVelocity(velocity);
  }

  getAngularVelocity() {
    if (!this.babylonObject?.physicsImpostor) return [0, 0, 0];
    
    const velocity = this.babylonObject.physicsImpostor.getAngularVelocity();
    return [velocity.x, velocity.y, velocity.z];
  }

  // === PHYSICS PROPERTIES ===
  
  setMass(mass) {
    if (!this.babylonObject?.physicsImpostor) return;
    
    this.babylonObject.physicsImpostor.setMass(mass);
  }

  getMass() {
    if (!this.babylonObject?.physicsImpostor) return 0;
    
    return this.babylonObject.physicsImpostor.mass;
  }

  setFriction(friction) {
    if (!this.babylonObject?.physicsImpostor) return;
    
    this.babylonObject.physicsImpostor.friction = friction;
  }

  getFriction() {
    if (!this.babylonObject?.physicsImpostor) return 0;
    
    return this.babylonObject.physicsImpostor.friction;
  }

  setRestitution(restitution) {
    if (!this.babylonObject?.physicsImpostor) return;
    
    this.babylonObject.physicsImpostor.restitution = restitution;
  }

  getRestitution() {
    if (!this.babylonObject?.physicsImpostor) return 0;
    
    return this.babylonObject.physicsImpostor.restitution;
  }

  // === PHYSICS CONSTRAINTS/JOINTS ===
  
  createPhysicsJoint(type, connectedObject, jointData = {}) {
    if (!this.babylonObject?.physicsImpostor || !connectedObject?.physicsImpostor) return null;
    
    // This would need proper joint implementation based on physics engine
    const joint = {
      type: type,
      mainObject: this.babylonObject,
      connectedObject: connectedObject,
      jointData: jointData
    };
    
    // Store reference for cleanup
    if (!this.babylonObject.physicsJoints) {
      this.babylonObject.physicsJoints = [];
    }
    this.babylonObject.physicsJoints.push(joint);
    
    return joint;
  }

  removePhysicsJoint(joint) {
    if (!joint || !this.babylonObject?.physicsJoints) return;
    
    const index = this.babylonObject.physicsJoints.indexOf(joint);
    if (index > -1) {
      this.babylonObject.physicsJoints.splice(index, 1);
    }
  }

  // === COLLISION DETECTION ===
  
  onCollisionEnter(callback) {
    if (!this.babylonObject?.physicsImpostor || !callback) return;
    
    this.babylonObject.physicsImpostor.registerOnPhysicsCollide(
      this.scene.meshes, // All meshes
      (collider, collidedWith) => {
        callback({
          collider: collider.object,
          collidedWith: collidedWith.object,
          point: null, // Would need collision point calculation
          normal: null // Would need collision normal calculation
        });
      }
    );
  }

  onCollisionExit(callback) {
    // This would need engine-specific implementation
    if (mesh && mesh.physicsImpostor && callback) {
      // Store callback for collision exit detection
      if (!mesh._collisionCallbacks) {
        mesh._collisionCallbacks = { exit: [] };
      }
      mesh._collisionCallbacks.exit.push(callback);
      
      // Set up collision tracking
      if (!mesh.physicsImpostor._collisionTracker) {
        mesh._previousCollisions = new Set();
        
        mesh.physicsImpostor.registerOnPhysicsCollide(mesh.physicsImpostor, (main, collided) => {
          const currentCollisions = new Set();
          
          // Track current collisions
          this.scene.meshes.forEach(otherMesh => {
            if (otherMesh.physicsImpostor && otherMesh !== main.object) {
              const distance = Vector3.Distance(main.object.position, otherMesh.position);
              const combinedRadius = (main.object.getBoundingInfo().boundingSphere.radius + 
                                   otherMesh.getBoundingInfo().boundingSphere.radius) * 1.1;
              
              if (distance < combinedRadius) {
                currentCollisions.add(otherMesh);
              }
            }
          });
          
          // Check for exits (was in previous but not in current)
          main.object._previousCollisions.forEach(prev => {
            if (!currentCollisions.has(prev) && main.object._collisionCallbacks?.exit) {
              main.object._collisionCallbacks.exit.forEach(cb => cb(prev));
            }
          });
          
          main.object._previousCollisions = currentCollisions;
        });
        
        mesh.physicsImpostor._collisionTracker = true;
      }
      return true;
    }
  }

  // === RAYCASTING (PHYSICS) ===
  
  physicsRaycast(originX, originY, originZ, directionX, directionY, directionZ, maxDistance = 100) {
    if (!this.scene?.physicsEnabled) return null;
    
    const physicsEngine = this.scene.getPhysicsEngine();
    if (!physicsEngine) return null;
    
    const from = new Vector3(originX, originY, originZ);
    const to = new Vector3(
      originX + directionX * maxDistance,
      originY + directionY * maxDistance,
      originZ + directionZ * maxDistance
    );
    
    // This would need proper raycast implementation based on physics engine
    console.warn('Physics raycast not fully implemented - depends on physics engine');
    return null;
  }

  // === CHARACTER CONTROLLER (HAVOK) ===
  
  createCharacterController(options = {}) {
    console.warn('Character controller requires Havok physics engine');
    return null;
  }

  moveCharacter(velocityX, velocityY, velocityZ) {
    console.warn('Character movement requires Havok character controller');
  }

  jumpCharacter(force = 5) {
    console.warn('Character jump requires Havok character controller');
  }

  // === RAGDOLL PHYSICS ===
  
  enableRagdoll() {
    console.warn('Ragdoll physics requires Havok physics engine');
    return false;
  }

  disableRagdoll() {
    console.warn('Ragdoll physics requires Havok physics engine');
  }

  // === SOFT BODY PHYSICS ===
  
  enableSoftBody(options = {}) {
    console.warn('Soft body physics requires advanced physics engine');
    return false;
  }

  setSoftBodyProperties(stiffness, damping) {
    console.warn('Soft body properties require advanced physics engine');
  }

  // === PHYSICS MATERIALS ===
  
  createPhysicsMaterial(name, friction = 0.5, restitution = 0.3) {
    return {
      name: name,
      friction: friction,
      restitution: restitution
    };
  }

  setPhysicsMaterial(material) {
    if (!this.babylonObject?.physicsImpostor || !material) return;
    
    this.setFriction(material.friction);
    this.setRestitution(material.restitution);
  }

  // === PHYSICS SIMULATION CONTROL ===
  
  pausePhysics() {
    if (!this.scene?.physicsEnabled) return;
    
    const physicsEngine = this.scene.getPhysicsEngine();
    if (physicsEngine && physicsEngine.setTimeStep) {
      physicsEngine.setTimeStep(0);
    }
  }

  resumePhysics(timeStep = 1/60) {
    if (!this.scene?.physicsEnabled) return;
    
    const physicsEngine = this.scene.getPhysicsEngine();
    if (physicsEngine && physicsEngine.setTimeStep) {
      physicsEngine.setTimeStep(timeStep);
    }
  }

  setPhysicsTimeStep(timeStep) {
    if (!this.scene?.physicsEnabled) return;
    
    const physicsEngine = this.scene.getPhysicsEngine();
    if (physicsEngine && physicsEngine.setTimeStep) {
      physicsEngine.setTimeStep(timeStep);
    }
  }

  // === PHYSICS DEBUG ===
  
  enablePhysicsDebug() {
    if (!this.scene.physicsEngine) {
      console.warn('Physics engine not enabled');
      return false;
    }
    
    try {
      // Try to use PhysicsViewer if available
      if (typeof PhysicsViewer !== 'undefined') {
        this._physicsViewer = new PhysicsViewer(this.scene);
        
        this.scene.meshes.forEach(mesh => {
          if (mesh.physicsImpostor) {
            this._physicsViewer.showImpostor(mesh.physicsImpostor);
          }
        });
      } else {
        // Create custom debug visualization
        this.scene.meshes.forEach(mesh => {
          if (mesh.physicsImpostor && !mesh._physicsDebugMesh) {
            // Create wireframe clone for debug visualization
            const debugMesh = mesh.clone(`${mesh.name}_physics_debug`);
            debugMesh.material = new StandardMaterial(`${mesh.name}_physics_mat`, this.scene);
            debugMesh.material.wireframe = true;
            debugMesh.material.emissiveColor = new Color3(0, 1, 0); // Green wireframe
            debugMesh.material.alpha = 0.6;
            debugMesh.isPickable = false;
            mesh._physicsDebugMesh = debugMesh;
          }
        });
      }
      
      return true;
    } catch (error) {
      console.error('Failed to enable physics debug:', error);
      return false;
    }
  }

  disablePhysicsDebug() {
    try {
      // Dispose PhysicsViewer if exists
      if (this._physicsViewer) {
        this._physicsViewer.dispose();
        this._physicsViewer = null;
      }
      
      // Remove custom debug meshes
      this.scene.meshes.forEach(mesh => {
        if (mesh._physicsDebugMesh) {
          mesh._physicsDebugMesh.dispose();
          mesh._physicsDebugMesh = null;
        }
      });
      
      return true;
    } catch (error) {
      console.error('Failed to disable physics debug:', error);
      return false;
    }
  }

  // === CLEANUP ===
  
  disposePhysics() {
    if (this.babylonObject?.physicsImpostor) {
      this.babylonObject.physicsImpostor.dispose();
      this.babylonObject.physicsImpostor = null;
    }
    
    if (this.babylonObject?.physicsJoints) {
      this.babylonObject.physicsJoints = [];
    }
  }
}