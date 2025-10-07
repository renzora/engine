import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';
import { PhysicsAggregate } from '@babylonjs/core/Physics/v2/physicsAggregate.js';
import { PhysicsShapeType } from '@babylonjs/core/Physics/v2/IPhysicsEnginePlugin.js';
import { HavokPlugin } from '@babylonjs/core/Physics/v2/Plugins/havokPlugin.js';
import HavokPhysics from '@babylonjs/havok';
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
  
  async enablePhysics(engine = 'havok', gravityX = 0, gravityY = -9.81, gravityZ = 0) {
    if (!this.scene) return false;
    
    try {
      const gravity = new Vector3(gravityX, gravityY, gravityZ);
      
      if (engine.toLowerCase() === 'havok') {
        try {
          const havokInstance = await HavokPhysics();
          const hk = new HavokPlugin(true, havokInstance);
          const enableResult = this.scene.enablePhysics(gravity, hk);
          console.log('✅ Havok physics enabled, result:', enableResult);
          return true;
        } catch (error) {
          console.error('Failed to initialize Havok:', error);
          return false;
        }
      } else {
        console.warn('Only Havok physics engine is supported');
        return false;
      }
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

  // === PHYSICS AGGREGATES (V2 API) ===
  
  setPhysicsImpostor(type = 'box', mass = 1, options = {}) {
    if (!this.babylonObject) return;
    
    const physicsEngine = this.scene.getPhysicsEngine();
    if (!physicsEngine) {
      console.warn('No physics engine available');
      return;
    }
    
    // Map aggregate types to PhysicsShapeType
    let shapeType;
    
    // Auto-detect shape for common objects
    if (this.babylonObject.getClassName) {
      const className = this.babylonObject.getClassName().toLowerCase();
      const objectName = this.babylonObject.name.toLowerCase();
      
      if (className.includes('ground') || objectName.includes('ground') || objectName.includes('plane')) {
        shapeType = PhysicsShapeType.BOX;
        console.log('Auto-detected ground/plane object, using BOX shape');
      } else if (className.includes('sphere') || objectName.includes('sphere')) {
        shapeType = PhysicsShapeType.SPHERE;
        console.log('Auto-detected sphere object, using SPHERE shape');
      } else {
        // Use provided type
        switch (type.toLowerCase()) {
          case 'sphere':
            shapeType = PhysicsShapeType.SPHERE;
            break;
          case 'cylinder':
            shapeType = PhysicsShapeType.CYLINDER;
            break;
          case 'mesh':
            shapeType = PhysicsShapeType.MESH;
            break;
          case 'convex_hull':
            shapeType = PhysicsShapeType.CONVEX_HULL;
            break;
          case 'plane':
            shapeType = PhysicsShapeType.BOX;
            break;
          case 'box':
          default:
            shapeType = PhysicsShapeType.BOX;
            break;
        }
      }
    } else {
      // Use provided type
      switch (type.toLowerCase()) {
        case 'sphere':
          shapeType = PhysicsShapeType.SPHERE;
          break;
        case 'cylinder':
          shapeType = PhysicsShapeType.CYLINDER;
          break;
        case 'mesh':
          shapeType = PhysicsShapeType.MESH;
          break;
        case 'convex_hull':
          shapeType = PhysicsShapeType.CONVEX_HULL;
          break;
        case 'plane':
          shapeType = PhysicsShapeType.BOX;
          break;
        case 'box':
        default:
          shapeType = PhysicsShapeType.BOX;
          break;
      }
    }
    
    console.log(`Creating physics aggregate: ${type} for object: ${this.babylonObject.name}`);
    console.log(`Object position: ${this.babylonObject.position.x}, ${this.babylonObject.position.y}, ${this.babylonObject.position.z}`);
    console.log(`Object scaling: ${this.babylonObject.scaling.x}, ${this.babylonObject.scaling.y}, ${this.babylonObject.scaling.z}`);
    
    try {
      // Dispose existing physics aggregate if it exists
      if (this.babylonObject.aggregate) {
        console.log(`🗑️ Disposing existing physics aggregate for ${this.babylonObject.name}`);
        this.babylonObject.aggregate.dispose();
        this.babylonObject.aggregate = null;
      }
      
      // Create PhysicsAggregate (Physics v2 API)
      const aggregate = new PhysicsAggregate(this.babylonObject, shapeType, {
        mass: mass,
        restitution: options.restitution || 0.3,
        friction: options.friction || 0.5,
        ...options
      }, this.scene);
      
      // Store aggregate reference
      this.babylonObject.aggregate = aggregate;
      
      console.log(`✅ Physics aggregate created for ${this.babylonObject.name} with ${type} shape, mass: ${mass}`);
      console.log(`Final object position after physics: ${this.babylonObject.position.x}, ${this.babylonObject.position.y}, ${this.babylonObject.position.z}`);
      
      // For dynamic objects, log initial state
      if (mass > 0 && aggregate.body) {
        const velocity = aggregate.body.getLinearVelocity();
        console.log(`Initial velocity: ${velocity.x}, ${velocity.y}, ${velocity.z}`);
      }
      
      return aggregate;
    } catch (error) {
      console.error('Failed to create physics aggregate:', error);
      return null;
    }
  }

  removePhysicsImpostor() {
    if (this.babylonObject?.aggregate) {
      this.babylonObject.aggregate.dispose();
      this.babylonObject.aggregate = null;
    }
  }

  hasPhysicsImpostor() {
    return !!this.babylonObject?.aggregate;
  }

  // === PHYSICS FORCES ===
  
  applyImpulse(forceX, forceY, forceZ, contactPointX = 0, contactPointY = 0, contactPointZ = 0) {
    if (!this.babylonObject?.aggregate?.body) return;
    
    const force = new Vector3(forceX, forceY, forceZ);
    const contactPoint = new Vector3(contactPointX, contactPointY, contactPointZ);
    
    this.babylonObject.aggregate.body.applyImpulse(force, contactPoint);
  }

  applyForce(forceX, forceY, forceZ, _contactPointX = 0, _contactPointY = 0, _contactPointZ = 0) {
    if (!this.babylonObject?.aggregate?.body) return;
    
    const force = new Vector3(forceX, forceY, forceZ);
    
    this.babylonObject.aggregate.body.applyForce(force);
  }

  setLinearVelocity(velocityX, velocityY, velocityZ) {
    if (!this.babylonObject?.aggregate?.body) return;
    
    const velocity = new Vector3(velocityX, velocityY, velocityZ);
    this.babylonObject.aggregate.body.setLinearVelocity(velocity);
  }

  getLinearVelocity() {
    if (!this.babylonObject?.aggregate?.body) return [0, 0, 0];
    
    const velocity = this.babylonObject.aggregate.body.getLinearVelocity();
    return [velocity.x, velocity.y, velocity.z];
  }

  setAngularVelocity(velocityX, velocityY, velocityZ) {
    if (!this.babylonObject?.aggregate?.body) return;
    
    const velocity = new Vector3(velocityX, velocityY, velocityZ);
    this.babylonObject.aggregate.body.setAngularVelocity(velocity);
  }

  getAngularVelocity() {
    if (!this.babylonObject?.aggregate?.body) return [0, 0, 0];
    
    const velocity = this.babylonObject.aggregate.body.getAngularVelocity();
    return [velocity.x, velocity.y, velocity.z];
  }

  // === PHYSICS BODY SYNC ===
  
  updatePhysics(options = {}) {
    if (!this.babylonObject?.aggregate) {
      console.warn('No physics aggregate found, cannot update physics properties');
      return;
    }
    
    // For V2 API, we need to recreate the aggregate with new properties
    // Store current aggregate type
    const currentType = this.babylonObject.aggregate.shape?.type || PhysicsShapeType.BOX;
    
    // Get current options and merge with new ones
    const currentOptions = {
      mass: this.getMass(),
      friction: 0.5, // Default fallback
      restitution: 0.3, // Default fallback
      ...options // Override with new values
    };
    
    // Dispose current aggregate
    this.babylonObject.aggregate.dispose();
    this.babylonObject.aggregate = null;
    
    // Recreate with updated properties
    const aggregate = new PhysicsAggregate(this.babylonObject, currentType, currentOptions, this.scene);
    this.babylonObject.aggregate = aggregate;
    
    console.log(`Physics properties updated:`, options);
  }
  

  // === PHYSICS PROPERTIES ===
  
  setMass(mass) {
    if (!this.babylonObject?.aggregate?.body) return;
    
    this.babylonObject.aggregate.body.setMassProperties({ mass: mass });
  }

  getMass() {
    if (!this.babylonObject?.aggregate?.body) return 0;
    
    const massProps = this.babylonObject.aggregate.body.getMassProperties();
    return massProps ? massProps.mass : 0;
  }

  setFriction(_friction) {
    console.warn('setFriction: Use updatePhysics() instead for V2 API');
  }

  getFriction() {
    return 0.5; // Default fallback
  }

  setRestitution(_restitution) {
    console.warn('setRestitution: Use updatePhysics() instead for V2 API');
  }

  getRestitution() {
    return 0.3; // Default fallback
  }

  // === SHORT NAME ALIASES ===
  
  mass() {
    return this.getMass();
  }
  
  friction() {
    return this.getFriction();
  }
  
  restitution() {
    return this.getRestitution();
  }
  
  gravity() {
    return this.getGravity();
  }
  
  linearVelocity() {
    return this.getLinearVelocity();
  }
  
  angularVelocity() {
    return this.getAngularVelocity();
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
        
        mesh.physicsImpostor.registerOnPhysicsCollide(mesh.physicsImpostor, (main, _collided) => {
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
    
    const _from = new Vector3(originX, originY, originZ);
    const _to = new Vector3(
      originX + directionX * maxDistance,
      originY + directionY * maxDistance,
      originZ + directionZ * maxDistance
    );
    
    // This would need proper raycast implementation based on physics engine
    console.warn('Physics raycast not fully implemented - depends on physics engine');
    return null;
  }

  // === CHARACTER CONTROLLER (HAVOK) ===
  
  createCharacterController(_options = {}) {
    console.warn('Character controller requires Havok physics engine');
    return null;
  }

  moveCharacter(_velocityX, _velocityY, _velocityZ) {
    console.warn('Character movement requires Havok character controller');
  }

  jumpCharacter(_force = 5) {
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
  
  enableSoftBody(_options = {}) {
    console.warn('Soft body physics requires advanced physics engine');
    return false;
  }

  setSoftBodyProperties(_stiffness, _damping) {
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
    if (this.babylonObject?.aggregate) {
      this.babylonObject.aggregate.dispose();
      this.babylonObject.aggregate = null;
    }
    
    if (this.babylonObject?.physicsJoints) {
      this.babylonObject.physicsJoints = [];
    }
  }
}