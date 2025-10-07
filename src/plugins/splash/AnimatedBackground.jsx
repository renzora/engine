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
import '@babylonjs/core/Meshes/Builders/torusBuilder';
import '@babylonjs/core/Meshes/Builders/boxBuilder';
import '@babylonjs/core/Meshes/Builders/sphereBuilder';
import '@babylonjs/core/Meshes/Builders/polyhedronBuilder';
import '@babylonjs/core/Meshes/Builders/linesBuilder';
import '@babylonjs/core/Meshes/Builders/cylinderBuilder';
import '@babylonjs/core/Meshes/Builders/capsuleBuilder';

export default function AnimatedBackground() {
  let canvasRef = null;
  let engine;
  let scene;
  let camera;
  let movingShapes = [];

  // Helper function to create shapes with consistent vertex counts for morphing
  const _createMorphableShape = (type, size, name) => {
    const segments = 16; // Consistent tessellation for all shapes
    
    switch (type) {
      case 0: // Box
        return MeshBuilder.CreateBox(name, { size, segments }, scene);
      case 1: // Sphere  
        return MeshBuilder.CreateSphere(name, { diameter: size, segments }, scene);
      case 2: // Torus
        return MeshBuilder.CreateTorus(name, { 
          diameter: size * 1.5, 
          thickness: size * 0.3, 
          tessellation: segments 
        }, scene);
      case 3: // Cylinder
        return MeshBuilder.CreateCylinder(name, { 
          height: size * 1.2, 
          diameter: size * 0.6,
          tessellation: segments
        }, scene);
      case 4: // Capsule
        return MeshBuilder.CreateCapsule(name, { 
          radius: size * 0.4, 
          height: size * 1.2,
          tessellation: segments
        }, scene);
      default:
        return MeshBuilder.CreateBox(name, { size, segments }, scene);
    }
  };





  const createScene = () => {
    scene = new Scene(engine);
    // Dark background
    scene.clearColor = new Color4(0.05, 0.05, 0.1, 1.0);
    camera = new ArcRotateCamera("camera", -Math.PI / 4, Math.PI / 3, 100, Vector3.Zero(), scene);
    camera.setTarget(Vector3.Zero());

    
    const hemisphericLight = new HemisphericLight("hemiLight", new Vector3(0, 1, 0), scene);
    hemisphericLight.intensity = 0.2; // Reduced for better shadow contrast
    // Dark ambient lighting
    hemisphericLight.diffuse = new Color3(0.1, 0.15, 0.2);
    hemisphericLight.specular = new Color3(0, 0, 0);
    
    const keyLight = new DirectionalLight("keyLight", new Vector3(-0.5, -1, -0.5), scene);
    keyLight.position = new Vector3(100, 150, 100);
    keyLight.intensity = 0.8; // Increased for better shadows
    // Primary blue lighting
    keyLight.diffuse = new Color3(0.15, 0.3, 0.6);
    keyLight.specular = new Color3(0.2, 0.4, 0.8);
    
    createNeonGrid();
    createMovingShapes();
    setupCameraAnimation();
    
    return scene;
  };


  const createNeonGrid = () => {
    const gridSpacing = 20;
    const renderDistance = 500;
    const gridLines = [];
    
    const createGridLines = () => {
      gridLines.forEach(line => line.dispose());
      gridLines.length = 0;
      const camPos = camera.position;
      const centerX = Math.round(camPos.x / gridSpacing) * gridSpacing;
      const centerZ = Math.round(camPos.z / gridSpacing) * gridSpacing;
      
      for (let i = -renderDistance; i <= renderDistance; i += gridSpacing) {
        const z = centerZ + i;
        const points = [
          new Vector3(centerX - renderDistance, 0, z),
          new Vector3(centerX + renderDistance, 0, z)
        ];
        const line = MeshBuilder.CreateLines(`gridLineH${i}`, { points }, scene);
        const distanceFromCenter = Math.abs(i);
        const fadeStart = renderDistance * 0.3;
        const alpha = distanceFromCenter < fadeStart ? 0.3 : 
                     Math.max(0.02, 0.3 * (1 - (distanceFromCenter - fadeStart) / (renderDistance * 0.7)));
        
        // Rainbow wave effect
        const time = performance.now() * 0.001;
        const distanceFromOrigin = Math.sqrt(i * i);
        const hue = (time + distanceFromOrigin * 0.02) % (Math.PI * 2);
        
        // Create rainbow colors using HSV to RGB conversion
        const h = (hue / (Math.PI * 2)) * 360;
        const s = 0.8;
        const v = 0.7;
        
        const c = v * s;
        const x1 = c * (1 - Math.abs((h / 60) % 2 - 1));
        const m = v - c;
        
        let r, g, b;
        if (h < 60) { r = c; g = x1; b = 0; }
        else if (h < 120) { r = x1; g = c; b = 0; }
        else if (h < 180) { r = 0; g = c; b = x1; }
        else if (h < 240) { r = 0; g = x1; b = c; }
        else if (h < 300) { r = x1; g = 0; b = c; }
        else { r = c; g = 0; b = x1; }
        
        line.color = new Color3(r + m, g + m, b + m);
        line.alpha = alpha;
        
        gridLines.push(line);
      }
      
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
        
        // Rainbow wave effect
        const time = performance.now() * 0.001;
        const distanceFromOrigin = Math.sqrt(i * i);
        const hue = (time + distanceFromOrigin * 0.02) % (Math.PI * 2);
        
        // Create rainbow colors using HSV to RGB conversion
        const h = (hue / (Math.PI * 2)) * 360;
        const s = 0.8;
        const v = 0.7;
        
        const c = v * s;
        const x1 = c * (1 - Math.abs((h / 60) % 2 - 1));
        const m = v - c;
        
        let r, g, b;
        if (h < 60) { r = c; g = x1; b = 0; }
        else if (h < 120) { r = x1; g = c; b = 0; }
        else if (h < 180) { r = 0; g = c; b = x1; }
        else if (h < 240) { r = 0; g = x1; b = c; }
        else if (h < 300) { r = x1; g = 0; b = c; }
        else { r = c; g = 0; b = x1; }
        
        line.color = new Color3(r + m, g + m, b + m);
        line.alpha = alpha;
        
        gridLines.push(line);
      }
    };
    
    createGridLines();
    
    let lastUpdatePos = camera.position.clone();
    let updateTimeout = null;
    
    scene.registerBeforeRender(() => {
      const currentPos = camera.position;
      const deltaX = Math.abs(currentPos.x - lastUpdatePos.x);
      const deltaZ = Math.abs(currentPos.z - lastUpdatePos.z);
      
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
      new Color3(0.235, 0.506, 0.957), // Primary blue
      new Color3(0.545, 0.365, 0.957), // Secondary purple  
      new Color3(0.024, 0.714, 0.831), // Accent cyan
      new Color3(1, 0.3, 0.8),         // Hot pink
      new Color3(0.8, 1, 0.3),         // Electric lime
      new Color3(0.3, 0.8, 1),         // Sky blue
      new Color3(1, 0.6, 0.2),         // Orange glow
      new Color3(0.6, 0.2, 1),         // Deep purple
      new Color3(0.2, 1, 0.6),         // Mint green
      new Color3(1, 0.2, 0.6),         // Magenta
      new Color3(0.4, 0.9, 0.9),       // Aqua
      new Color3(0.9, 0.9, 0.4)        // Electric yellow
    ];
    
    for (let i = 0; i < shapeCount; i++) {
      const shapeType = i % 6;
      const size = 3 + Math.random() * 5;
      
      let shape;
      switch (shapeType) {
        case 0:
          shape = MeshBuilder.CreateBox(`movingCube${i}`, { size }, scene);
          break;
        case 1:
          shape = MeshBuilder.CreateSphere(`movingSphere${i}`, { diameter: size, segments: 16 }, scene);
          break;
        case 2:
          shape = MeshBuilder.CreateTorus(`movingTorus${i}`, { 
            diameter: size * 1.5, 
            thickness: size * 0.3, 
            tessellation: 16 
          }, scene);
          break;
        case 3:
          shape = MeshBuilder.CreateCylinder(`movingCylinder${i}`, { 
            height: size * 1.2, 
            diameter: size * 0.6,
            tessellation: 16
          }, scene);
          break;
        case 4:
          shape = MeshBuilder.CreateCapsule(`movingCapsule${i}`, { 
            radius: size * 0.4, 
            height: size * 1.2
          }, scene);
          break;
        case 5:
          shape = MeshBuilder.CreatePolyhedron(`movingPoly${i}`, { 
            type: Math.floor(Math.random() * 3), 
            size: size * 0.8
          }, scene);
          break;
      }
      
      const material = new StandardMaterial(`movingShapeMaterial${i}`, scene);
      const color = neonColors[i % neonColors.length];
      
      // Wireframe materials
      material.wireframe = true;
      material.diffuseColor = color;
      material.emissiveColor = color.scale(0.3);
      material.specularColor = new Color3(0, 0, 0);
      material.backFaceCulling = false;
      
      shape.material = material;

      shape.position = new Vector3(
        (Math.random() - 0.5) * 300,
        Math.random() * 40 + 5,
        (Math.random() - 0.5) * 300
      );

      movingShapes.push({
        mesh: shape,
        material: material,
        originalColor: color,
        currentColorIndex: i % neonColors.length,
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
      
      // Enhanced R3 pulse animation - more dramatic
      const pulseAnimation = new Animation(
        `shapePulse${i}`, 
        "emissiveColor", 
        60, 
        Animation.ANIMATIONTYPE_COLOR3, 
        Animation.ANIMATIONLOOPMODE_CYCLE
      );
      
      const keys = [];
      keys.push({ frame: 0, value: color.scale(0.3) });
      keys.push({ frame: 60 + i * 5, value: color.scale(0.8) }); // Brighter peak
      keys.push({ frame: 120 + i * 10, value: color.scale(0.3) });
      
      pulseAnimation.setKeys(keys);
      
      // Add intensity scaling animation for extra drama
      const intensityAnimation = new Animation(
        `shapeIntensity${i}`,
        "diffuseColor",
        60,
        Animation.ANIMATIONTYPE_COLOR3,
        Animation.ANIMATIONLOOPMODE_CYCLE
      );
      
      const intensityKeys = [];
      intensityKeys.push({ frame: 0, value: color.scale(0.4) });
      intensityKeys.push({ frame: 90 + i * 8, value: color.scale(0.2) });
      intensityKeys.push({ frame: 180 + i * 16, value: color.scale(0.4) });
      
      intensityAnimation.setKeys(intensityKeys);
      
      if (!material.animations) {
        material.animations = [];
      }
      // Scale animation
      const scaleAnimation = new Animation(
        `shapeScale${i}`,
        "scaling",
        60,
        Animation.ANIMATIONTYPE_VECTOR3,
        Animation.ANIMATIONLOOPMODE_CYCLE
      );
      
      const scaleKeys = [];
      const baseScale = 0.8 + Math.random() * 0.4; // Random base scale
      const scaleVariation = 0.3 + Math.random() * 0.4; // Random scale variation
      
      scaleKeys.push({ frame: 0, value: new Vector3(baseScale, baseScale, baseScale) });
      scaleKeys.push({ frame: 300 + i * 10, value: new Vector3(baseScale + scaleVariation, baseScale + scaleVariation, baseScale + scaleVariation) });
      scaleKeys.push({ frame: 600 + i * 20, value: new Vector3(baseScale, baseScale, baseScale) });
      
      scaleAnimation.setKeys(scaleKeys);
      
      if (!shape.animations) {
        shape.animations = [];
      }
      shape.animations.push(scaleAnimation);
      scene.beginAnimation(shape, 0, 600 + i * 20, true);
      
      material.animations.push(pulseAnimation, intensityAnimation);
      scene.beginAnimation(material, 0, 180 + i * 16, true);
    }
    
    scene.registerBeforeRender(() => {
      movingShapes.forEach((shapeData, index) => {
        const { mesh, material: _material, velocity, rotationSpeed, bounds } = shapeData;
        
        
        
        
        
        for (let i = index + 1; i < movingShapes.length; i++) {
          const otherShape = movingShapes[i];
          const distance = Vector3.Distance(mesh.position, otherShape.mesh.position);
          const collisionDistance = 8;
          
          if (distance < collisionDistance) {
            const collisionNormal = otherShape.mesh.position.subtract(mesh.position).normalize();
            const relativeVelocity = velocity.subtract(otherShape.velocity);
            const velocityAlongNormal = Vector3.Dot(relativeVelocity, collisionNormal);
            
            if (velocityAlongNormal > 0) continue;
    
            const bounceStrength = 0.8;
            const impulse = collisionNormal.scale(velocityAlongNormal * bounceStrength);
            
            velocity.subtractInPlace(impulse);
            otherShape.velocity.addInPlace(impulse);
            const spinFactor = 0.01;
            rotationSpeed.addInPlace(collisionNormal.scale(spinFactor));
            otherShape.rotationSpeed.addInPlace(collisionNormal.scale(-spinFactor));
            const maxRotationSpeed = 0.05;
            if (rotationSpeed.length() > maxRotationSpeed) {
              rotationSpeed.normalize().scaleInPlace(maxRotationSpeed);
            }
            if (otherShape.rotationSpeed.length() > maxRotationSpeed) {
              otherShape.rotationSpeed.normalize().scaleInPlace(maxRotationSpeed);
            }
            
            const separation = collisionNormal.scale((collisionDistance - distance) * 0.5);
            mesh.position.subtractInPlace(separation);
            otherShape.mesh.position.addInPlace(separation);
          }
        }
        
        mesh.position.addInPlace(velocity);
        mesh.rotation.addInPlace(rotationSpeed);
        rotationSpeed.scaleInPlace(0.995);
        
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
    // Set camera position
    camera.alpha = -Math.PI / 4;
    camera.beta = Math.PI / 3;
    camera.radius = 100;
    camera.setTarget(Vector3.Zero());
    
    // Smooth camera rotation around the same central location
    const rotationAnimation = new Animation(
      "cameraRotation", "alpha", 60,
      Animation.ANIMATIONTYPE_FLOAT,
      Animation.ANIMATIONLOOPMODE_CYCLE
    );
    
    rotationAnimation.setKeys([
      { frame: 0, value: camera.alpha },
      { frame: 3600, value: camera.alpha + Math.PI * 2 }
    ]);
    
    // Gentle elevation changes
    const elevationAnimation = new Animation(
      "cameraElevation", "beta", 60,
      Animation.ANIMATIONTYPE_FLOAT,
      Animation.ANIMATIONLOOPMODE_CYCLE
    );
    
    elevationAnimation.setKeys([
      { frame: 0, value: Math.PI / 3 },
      { frame: 1800, value: Math.PI / 4 },
      { frame: 3600, value: Math.PI / 3 }
    ]);
    
    // Subtle zoom in/out
    const zoomAnimation = new Animation(
      "cameraZoom", "radius", 60,
      Animation.ANIMATIONTYPE_FLOAT,
      Animation.ANIMATIONLOOPMODE_CYCLE
    );
    
    zoomAnimation.setKeys([
      { frame: 0, value: 100 },
      { frame: 1200, value: 110 },
      { frame: 2400, value: 90 },
      { frame: 3600, value: 100 }
    ]);
    
    if (!camera.animations) {
      camera.animations = [];
    }
    camera.animations = [rotationAnimation, elevationAnimation, zoomAnimation];
    
    scene.beginAnimation(camera, 0, 3600, true);
    
    // Micro-movements for organic feel
    scene.registerBeforeRender(() => {
      if (movingShapes.length > 0) {
        const time = performance.now() * 0.0003;
        const baseTarget = Vector3.Zero();
        camera.target.x = baseTarget.x + Math.sin(time) * 4;
        camera.target.z = baseTarget.z + Math.cos(time * 0.7) * 4;
        camera.target.y = baseTarget.y + Math.sin(time * 0.5) * 1;
      }
    });
  };

  onMount(() => {
    if (!canvasRef) return;

    try {
      engine = new Engine(canvasRef, true, { preserveDrawingBuffer: true, stencil: true });
      
      createScene();
      
      engine.runRenderLoop(() => {
        if (scene && scene.activeCamera) {
          scene.render();
        }
      });
      
      const handleResize = () => {
        if (engine) {
          engine.resize();
        }
      };
      
      window.addEventListener('resize', handleResize);
      
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