import { onMount, onCleanup } from 'solid-js';
import { Engine } from '@babylonjs/core/Engines/engine';
import { Scene } from '@babylonjs/core/scene';
import { ArcRotateCamera } from '@babylonjs/core/Cameras/arcRotateCamera';
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight';
import { DirectionalLight } from '@babylonjs/core/Lights/directionalLight';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { Color3, Color4 } from '@babylonjs/core/Maths/math.color';
import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { Animation } from '@babylonjs/core/Animations/animation';
import { GlowLayer } from '@babylonjs/core/Layers/glowLayer';
// Import mesh builders
import '@babylonjs/core/Meshes/Builders/torusBuilder';
import '@babylonjs/core/Meshes/Builders/boxBuilder';
import '@babylonjs/core/Meshes/Builders/sphereBuilder';
import '@babylonjs/core/Meshes/Builders/polyhedronBuilder';
import '@babylonjs/core/Meshes/Builders/linesBuilder';
import '@babylonjs/core/Meshes/Builders/cylinderBuilder';
import '@babylonjs/core/Meshes/Builders/capsuleBuilder';

export default function AnimatedBackground() {
  let canvasRef;
  let engine;
  let scene;
  let camera;
  let glowLayer;
  let movingShapes = [];

  const createScene = () => {
    // Create scene with deep space background
    scene = new Scene(engine);
    scene.clearColor = new Color4(0.01, 0.02, 0.05, 1.0); // Deep space black
    
    // Create camera with better positioning for grid view
    camera = new ArcRotateCamera("camera", -Math.PI / 4, Math.PI / 3, 100, Vector3.Zero(), scene);
    camera.setTarget(Vector3.Zero());
    
    // Create glow layer with reduced intensity
    glowLayer = new GlowLayer("glow", scene, {
      mainTextureFixedSize: 1024,
      blurKernelSize: 32
    });
    glowLayer.intensity = 1.2;
    
    // Proper lighting setup
    const hemisphericLight = new HemisphericLight("hemiLight", new Vector3(0, 1, 0), scene);
    hemisphericLight.intensity = 0.35;
    hemisphericLight.diffuse = new Color3(0.1, 0.15, 0.3);
    hemisphericLight.specular = new Color3(0, 0, 0);
    
    // Key light
    const keyLight = new DirectionalLight("keyLight", new Vector3(-0.5, -1, -0.5), scene);
    keyLight.position = new Vector3(50, 100, 50);
    keyLight.intensity = 0.6;
    keyLight.diffuse = new Color3(0.3, 0.7, 1.0);
    keyLight.specular = new Color3(0.5, 0.8, 1.0);
    
    // Create the neon grid
    createNeonGrid();
    
    // Create moving shapes
    createMovingShapes();
    
    // Setup camera animation
    setupCameraAnimation();
    
    return scene;
  };

  const createNeonGrid = () => {
    const gridSpacing = 20;
    const renderDistance = 500;
    const gridLines = [];
    
    // Function to create infinite grid lines
    const createGridLines = () => {
      // Clear existing lines
      gridLines.forEach(line => line.dispose());
      gridLines.length = 0;
      
      // Get camera position for centering
      const camPos = camera.position;
      const centerX = Math.round(camPos.x / gridSpacing) * gridSpacing;
      const centerZ = Math.round(camPos.z / gridSpacing) * gridSpacing;
      
      // Create horizontal lines extending to horizon
      for (let i = -renderDistance; i <= renderDistance; i += gridSpacing) {
        const z = centerZ + i;
        const points = [
          new Vector3(centerX - renderDistance, 0, z),
          new Vector3(centerX + renderDistance, 0, z)
        ];
        const line = MeshBuilder.CreateLines(`gridLineH${i}`, { points }, scene);
        
        // Distance-based fade for infinite appearance
        const distanceFromCenter = Math.abs(i);
        const fadeStart = renderDistance * 0.3;
        const alpha = distanceFromCenter < fadeStart ? 0.3 : 
                     Math.max(0.02, 0.3 * (1 - (distanceFromCenter - fadeStart) / (renderDistance * 0.7)));
        
        line.color = new Color3(0.2, 0.4, 0.6);
        line.alpha = alpha;
        
        gridLines.push(line);
        glowLayer.addIncludedOnlyMesh(line);
      }
      
      // Create vertical lines extending to horizon
      for (let i = -renderDistance; i <= renderDistance; i += gridSpacing) {
        const x = centerX + i;
        const points = [
          new Vector3(x, 0, centerZ - renderDistance),
          new Vector3(x, 0, centerZ + renderDistance)
        ];
        const line = MeshBuilder.CreateLines(`gridLineV${i}`, { points }, scene);
        
        const distanceFromCenter = Math.abs(i);
        const fadeStart = renderDistance * 0.3;
        const alpha = distanceFromCenter < fadeStart ? 0.3 : 
                     Math.max(0.02, 0.3 * (1 - (distanceFromCenter - fadeStart) / (renderDistance * 0.7)));
        
        line.color = new Color3(0.2, 0.4, 0.6);
        line.alpha = alpha;
        
        gridLines.push(line);
        glowLayer.addIncludedOnlyMesh(line);
      }
    };
    
    // Initial grid creation
    createGridLines();
    
    // Update grid when camera moves significantly
    let lastUpdatePos = camera.position.clone();
    let updateTimeout = null;
    
    scene.registerBeforeRender(() => {
      const currentPos = camera.position;
      const deltaX = Math.abs(currentPos.x - lastUpdatePos.x);
      const deltaZ = Math.abs(currentPos.z - lastUpdatePos.z);
      
      // Recreate grid when camera moves significantly, with debouncing
      if (deltaX > gridSpacing * 1.5 || deltaZ > gridSpacing * 1.5) {
        if (updateTimeout) {
          clearTimeout(updateTimeout);
        }
        
        updateTimeout = setTimeout(() => {
          createGridLines();
          lastUpdatePos = currentPos.clone();
          updateTimeout = null;
        }, 50);
      }
    });
  };

  const createMovingShapes = () => {
    const shapeCount = 60;
    const neonColors = [
      new Color3(1, 0, 1),    // magenta
      new Color3(1, 1, 0),    // yellow
      new Color3(1, 0.3, 0),  // orange
      new Color3(0, 1, 0.3),  // green
      new Color3(0.3, 0, 1),  // blue
      new Color3(1, 0, 0.3),  // red
      new Color3(0.3, 1, 0),  // lime
      new Color3(1, 0.5, 0.8) // pink
    ];
    
    for (let i = 0; i < shapeCount; i++) {
      let shape;
      const shapeType = i % 6;
      
      switch (shapeType) {
        case 0:
          shape = MeshBuilder.CreateBox(`movingCube${i}`, { size: 4 + Math.random() * 4 }, scene);
          break;
        case 1:
          shape = MeshBuilder.CreateSphere(`movingSphere${i}`, { diameter: 4 + Math.random() * 4, segments: 16 }, scene);
          break;
        case 2:
          shape = MeshBuilder.CreateTorus(`movingTorus${i}`, { 
            diameter: 6 + Math.random() * 3, 
            thickness: 1 + Math.random() * 0.8, 
            tessellation: 16 
          }, scene);
          break;
        case 3:
          shape = MeshBuilder.CreatePolyhedron(`movingPoly${i}`, { 
            type: Math.floor(Math.random() * 3), 
            size: 3 + Math.random() * 3 
          }, scene);
          break;
        case 4:
          shape = MeshBuilder.CreateCylinder(`movingCylinder${i}`, { 
            height: 5 + Math.random() * 3, 
            diameter: 3 + Math.random() * 2 
          }, scene);
          break;
        case 5:
          shape = MeshBuilder.CreateCapsule(`movingCapsule${i}`, { 
            radius: 2 + Math.random() * 2, 
            height: 4 + Math.random() * 3 
          }, scene);
          break;
      }
      
      // Create material
      const material = new StandardMaterial(`movingShapeMaterial${i}`, scene);
      const color = neonColors[i % neonColors.length];
      
      material.diffuseColor = color.scale(0.6);
      material.emissiveColor = color.scale(0.4);
      material.specularColor = new Color3(0.5, 0.5, 0.5);
      material.specularPower = 64;
      
      shape.material = material;
      
      // Add to glow layer
      glowLayer.addIncludedOnlyMesh(shape);
      
      // Random starting position - spread them out more
      shape.position = new Vector3(
        (Math.random() - 0.5) * 300,
        Math.random() * 40 + 5,
        (Math.random() - 0.5) * 300
      );
      
      // Store shape data for animation
      movingShapes.push({
        mesh: shape,
        velocity: new Vector3(
          (Math.random() - 0.5) * 0.4,
          (Math.random() - 0.5) * 0.25,
          (Math.random() - 0.5) * 0.4
        ),
        rotationSpeed: new Vector3(
          (Math.random() - 0.5) * 0.03,
          (Math.random() - 0.5) * 0.03,
          (Math.random() - 0.5) * 0.03
        ),
        bounds: 180
      });
      
      // Pulsing glow animation
      const pulseAnimation = new Animation(
        `shapePulse${i}`, 
        "emissiveColor", 
        60, 
        Animation.ANIMATIONTYPE_COLOR3, 
        Animation.ANIMATIONLOOPMODE_CYCLE
      );
      
      const keys = [];
      keys.push({ frame: 0, value: color.scale(0.15) });
      keys.push({ frame: 75 + i * 7, value: color.scale(0.25) });
      keys.push({ frame: 150 + i * 15, value: color.scale(0.15) });
      
      pulseAnimation.setKeys(keys);
      
      if (!material.animations) {
        material.animations = [];
      }
      material.animations.push(pulseAnimation);
      scene.beginAnimation(material, 0, 150 + i * 15, true);
    }
    
    // Setup movement animation with collision detection
    scene.registerBeforeRender(() => {
      movingShapes.forEach((shapeData, index) => {
        const { mesh, velocity, rotationSpeed, bounds } = shapeData;
        
        // Check collisions with other shapes
        for (let i = index + 1; i < movingShapes.length; i++) {
          const otherShape = movingShapes[i];
          const distance = Vector3.Distance(mesh.position, otherShape.mesh.position);
          const collisionDistance = 8; // Collision threshold
          
          if (distance < collisionDistance) {
            // Calculate collision normal
            const collisionNormal = otherShape.mesh.position.subtract(mesh.position).normalize();
            
            // Reflect velocities
            const relativeVelocity = velocity.subtract(otherShape.velocity);
            const velocityAlongNormal = Vector3.Dot(relativeVelocity, collisionNormal);
            
            if (velocityAlongNormal > 0) continue; // Objects separating
            
            // Apply collision response with some bounce
            const bounceStrength = 0.8;
            const impulse = collisionNormal.scale(velocityAlongNormal * bounceStrength);
            
            velocity.subtractInPlace(impulse);
            otherShape.velocity.addInPlace(impulse);
            
            // Add some spin on collision with damping
            const spinFactor = 0.01;
            rotationSpeed.addInPlace(collisionNormal.scale(spinFactor));
            otherShape.rotationSpeed.addInPlace(collisionNormal.scale(-spinFactor));
            
            // Apply rotation damping to prevent spinning out of control
            const maxRotationSpeed = 0.05;
            if (rotationSpeed.length() > maxRotationSpeed) {
              rotationSpeed.normalize().scaleInPlace(maxRotationSpeed);
            }
            if (otherShape.rotationSpeed.length() > maxRotationSpeed) {
              otherShape.rotationSpeed.normalize().scaleInPlace(maxRotationSpeed);
            }
            
            // Separate overlapping objects
            const separation = collisionNormal.scale((collisionDistance - distance) * 0.5);
            mesh.position.subtractInPlace(separation);
            otherShape.mesh.position.addInPlace(separation);
          }
        }
        
        // Update position
        mesh.position.addInPlace(velocity);
        
        // Update rotation with gradual damping
        mesh.rotation.addInPlace(rotationSpeed);
        
        // Apply gradual rotation damping to naturally slow down spinning
        rotationSpeed.scaleInPlace(0.995);
        
        // Bounce off boundaries
        if (Math.abs(mesh.position.x) > bounds) {
          velocity.x *= -1;
        }
        if (mesh.position.y > 35 || mesh.position.y < 2) {
          velocity.y *= -1;
        }
        if (Math.abs(mesh.position.z) > bounds) {
          velocity.z *= -1;
        }
      });
    });
  };

  const setupCameraAnimation = () => {
    let currentAngleSet = 0;
    let currentLocationSet = 0;
    
    const angleSets = [
      { alpha: -Math.PI / 4, beta: Math.PI / 3, radius: 100 },
      { alpha: Math.PI / 6, beta: Math.PI / 4, radius: 120 },
      { alpha: Math.PI / 2, beta: Math.PI / 6, radius: 80 },
      { alpha: -Math.PI / 2, beta: Math.PI / 2.5, radius: 110 },
      { alpha: Math.PI, beta: Math.PI / 3.5, radius: 90 }
    ];
    
    const locationSets = [
      new Vector3(0, 0, 0),      // Center
      new Vector3(80, 0, 80),    // Corner 1
      new Vector3(-80, 0, 80),   // Corner 2
      new Vector3(-80, 0, -80),  // Corner 3
      new Vector3(80, 0, -80),   // Corner 4
      new Vector3(0, 0, 120),    // Far side
      new Vector3(120, 0, 0),    // Right side
      new Vector3(0, 0, -120),   // Near side
      new Vector3(-120, 0, 0)    // Left side
    ];
    
    const animateToAngle = (targetAngle, targetLocation) => {
      const rotationAnim = new Animation(
        "mediumRotation", "alpha", 60,
        Animation.ANIMATIONTYPE_FLOAT,
        Animation.ANIMATIONLOOPMODE_CONSTANT
      );
      rotationAnim.setKeys([
        { frame: 0, value: camera.alpha },
        { frame: 2700, value: targetAngle.alpha + Math.PI * 2 }
      ]);
      
      const elevationAnim = new Animation(
        "elevation", "beta", 60,
        Animation.ANIMATIONTYPE_FLOAT,
        Animation.ANIMATIONLOOPMODE_CONSTANT
      );
      elevationAnim.setKeys([
        { frame: 0, value: camera.beta },
        { frame: 1350, value: targetAngle.beta },
        { frame: 2700, value: targetAngle.beta }
      ]);
      
      const zoomAnim = new Animation(
        "zoom", "radius", 60,
        Animation.ANIMATIONTYPE_FLOAT,
        Animation.ANIMATIONLOOPMODE_CONSTANT
      );
      zoomAnim.setKeys([
        { frame: 0, value: camera.radius },
        { frame: 900, value: targetAngle.radius + 20 },
        { frame: 1800, value: targetAngle.radius - 10 },
        { frame: 2700, value: targetAngle.radius }
      ]);
      
      // Animate camera target position (physical location change)
      const targetXAnim = new Animation(
        "targetX", "target.x", 60,
        Animation.ANIMATIONTYPE_FLOAT,
        Animation.ANIMATIONLOOPMODE_CONSTANT
      );
      targetXAnim.setKeys([
        { frame: 0, value: camera.target.x },
        { frame: 1350, value: targetLocation.x },
        { frame: 2700, value: targetLocation.x }
      ]);
      
      const targetZAnim = new Animation(
        "targetZ", "target.z", 60,
        Animation.ANIMATIONTYPE_FLOAT,
        Animation.ANIMATIONLOOPMODE_CONSTANT
      );
      targetZAnim.setKeys([
        { frame: 0, value: camera.target.z },
        { frame: 1350, value: targetLocation.z },
        { frame: 2700, value: targetLocation.z }
      ]);
      
      if (!camera.animations) {
        camera.animations = [];
      }
      camera.animations = [rotationAnim, elevationAnim, zoomAnim, targetXAnim, targetZAnim];
      
      const animGroup = scene.beginAnimation(camera, 0, 2700, false);
      
      animGroup.onAnimationEndObservable.add(() => {
        currentAngleSet = (currentAngleSet + 1) % angleSets.length;
        currentLocationSet = (currentLocationSet + 1) % locationSets.length;
        animateToAngle(angleSets[currentAngleSet], locationSets[currentLocationSet]);
      });
    };
    
    animateToAngle(angleSets[currentAngleSet], locationSets[currentLocationSet]);
    
    // Dynamic target movement - smaller since we now have location changes
    scene.registerBeforeRender(() => {
      if (movingShapes.length > 0) {
        const time = performance.now() * 0.0005;
        // Smaller movements since we have bigger location changes
        const baseTarget = locationSets[currentLocationSet] || Vector3.Zero();
        camera.target.x = baseTarget.x + Math.sin(time) * 4;
        camera.target.z = baseTarget.z + Math.cos(time * 0.7) * 4;
        camera.target.y = baseTarget.y + Math.sin(time * 0.5) * 1;
      }
    });
  };

  onMount(() => {
    if (!canvasRef) return;

    try {
      // Create engine
      engine = new Engine(canvasRef, true, { preserveDrawingBuffer: true, stencil: true });
      
      // Create scene
      createScene();
      
      // Start render loop
      engine.runRenderLoop(() => {
        if (scene && scene.activeCamera) {
          scene.render();
        }
      });
      
      // Handle window resize
      const handleResize = () => {
        if (engine) {
          engine.resize();
        }
      };
      
      window.addEventListener('resize', handleResize);
      
      // Cleanup function
      onCleanup(() => {
        window.removeEventListener('resize', handleResize);
        if (scene) {
          scene.dispose();
        }
        if (engine) {
          engine.dispose();
        }
      });
      
    } catch (error) {
      console.error('Failed to create Babylon.js scene:', error);
    }
  });

  return (
    <canvas
      ref={canvasRef}
      class="absolute inset-0 w-full h-full"
      style={{ 
        "z-index": "1",
        "pointer-events": "none",
        "display": "block"
      }}
    />
  );
}