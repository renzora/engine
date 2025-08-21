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
  let colorCache = {};
  let lastTheme = null;

  // Helper function to convert OKLCH to RGB using CSS Color Module 4 conversion
  const oklchToRgb = (l, c, h) => {
    // Convert OKLCH to OKLAB
    const hRad = h * Math.PI / 180;
    const a = c * Math.cos(hRad);
    const b = c * Math.sin(hRad);
    
    // Convert OKLAB to linear RGB using matrices from CSS Color Module 4 spec
    const l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    const m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    const s_ = l - 0.0894841775 * a - 1.2914855480 * b;
    
    const l3 = l_ * l_ * l_;
    const m3 = m_ * m_ * m_;
    const s3 = s_ * s_ * s_;
    
    let r = +4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3;
    let g = -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3;
    let bl = -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3;
    
    // Gamma correction for sRGB
    r = r > 0.0031308 ? 1.055 * Math.pow(r, 1/2.4) - 0.055 : 12.92 * r;
    g = g > 0.0031308 ? 1.055 * Math.pow(g, 1/2.4) - 0.055 : 12.92 * g;
    bl = bl > 0.0031308 ? 1.055 * Math.pow(bl, 1/2.4) - 0.055 : 12.92 * bl;
    
    return {
      r: Math.max(0, Math.min(1, r)),
      g: Math.max(0, Math.min(1, g)),
      b: Math.max(0, Math.min(1, bl))
    };
  };

  // Helper function to parse color string and convert to RGB
  const parseColorToRgb = (colorStr) => {
    if (colorStr.startsWith('oklch(')) {
      const match = colorStr.match(/oklch\(([\d.%]+)\s+([\d.]+)\s+([\d.]+)\)/);
      if (match) {
        let l = parseFloat(match[1]);
        const c = parseFloat(match[2]);
        const h = parseFloat(match[3]);
        
        // Convert percentage lightness to decimal
        if (match[1].includes('%')) {
          l = l / 100;
        }
        
        return oklchToRgb(l, c, h);
      }
    }
    
    if (colorStr.startsWith('rgb(')) {
      const match = colorStr.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
      if (match) {
        return {
          r: parseInt(match[1]) / 255,
          g: parseInt(match[2]) / 255,
          b: parseInt(match[3]) / 255
        };
      }
    }
    
    if (colorStr.startsWith('#')) {
      const hex = colorStr.slice(1);
      return {
        r: parseInt(hex.slice(0, 2), 16) / 255,
        g: parseInt(hex.slice(2, 4), 16) / 255,
        b: parseInt(hex.slice(4, 6), 16) / 255
      };
    }
    
    return null;
  };

  // Helper function to get DaisyUI color from CSS custom properties with caching
  const getDaisyUIColor = (colorName) => {
    const currentTheme = document.documentElement.getAttribute('data-theme') || 'default';
    const cacheKey = `${currentTheme}-${colorName}`;
    
    // Check if colors need to be recalculated
    if (lastTheme !== currentTheme) {
      colorCache = {}; // Clear cache when theme changes
      lastTheme = currentTheme;
    }
    
    // Return cached color if available
    if (colorCache[cacheKey]) {
      return colorCache[cacheKey];
    }
    
    const style = getComputedStyle(document.documentElement);
    // Map short names to actual DaisyUI CSS custom property names
    const colorPropertyMap = {
      'p': 'color-primary',
      's': 'color-secondary', 
      'a': 'color-accent',
      'b1': 'color-base-100',
      'b2': 'color-base-200',
      'b3': 'color-base-300',
      'bc': 'color-base-content',
      'n': 'color-neutral'
    };
    
    const propertyName = colorPropertyMap[colorName] || colorName;
    const colorValue = style.getPropertyValue(`--${propertyName}`).trim();
    
    let color;
    if (colorValue) {
      const rgb = parseColorToRgb(colorValue);
      if (rgb) {
        color = new Color3(rgb.r, rgb.g, rgb.b);
      }
    }
    
    // Fallback colors that match common DaisyUI themes
    if (!color) {
      switch (colorName) {
        case 'p': color = new Color3(0.235, 0.506, 0.957); break; // primary blue
        case 's': color = new Color3(0.545, 0.365, 0.957); break; // secondary purple
        case 'a': color = new Color3(0.024, 0.714, 0.831); break; // accent cyan
        case 'b1': color = new Color3(0.067, 0.094, 0.149); break; // base-100 dark
        case 'b2': color = new Color3(0.122, 0.161, 0.216); break; // base-200
        case 'b3': color = new Color3(0.220, 0.255, 0.318); break; // base-300
        case 'bc': color = new Color3(0.9, 0.9, 0.9); break; // base-content light
        default: color = new Color3(0.235, 0.506, 0.957); break; // fallback to primary
      }
    }
    
    // Cache the color
    colorCache[cacheKey] = color;
    return color;
  };

  // Function to update scene colors when theme changes
  const updateSceneColors = () => {
    if (!scene) return;

    // Update background color
    const bgColor = getDaisyUIColor('b1');
    scene.clearColor = new Color4(bgColor.r, bgColor.g, bgColor.b, 1.0);

    // Update lighting colors
    const ambientColor = getDaisyUIColor('b3');
    const primaryColor = getDaisyUIColor('p');
    
    scene.lights.forEach(light => {
      if (light.name === 'hemiLight') {
        light.diffuse = ambientColor.scale(0.4);
      } else if (light.name === 'keyLight') {
        light.diffuse = primaryColor.scale(0.8);
        light.specular = primaryColor.scale(1.2);
      }
    });

    // Update grid line colors
    const gridColor = getDaisyUIColor('p');
    scene.meshes.forEach(mesh => {
      if (mesh.name.startsWith('gridLine')) {
        mesh.color = gridColor;
      }
    });

  };

  const createScene = () => {
    scene = new Scene(engine);
    // Use DaisyUI base-100 color for background with transparency
    const bgColor = getDaisyUIColor('b1');
    scene.clearColor = new Color4(bgColor.r, bgColor.g, bgColor.b, 1.0);
    camera = new ArcRotateCamera("camera", -Math.PI / 4, Math.PI / 3, 100, Vector3.Zero(), scene);
    camera.setTarget(Vector3.Zero());

    glowLayer = new GlowLayer("glow", scene, {
      mainTextureFixedSize: 1024,
      blurKernelSize: 32
    });
    glowLayer.intensity = 1.2;
    
    const hemisphericLight = new HemisphericLight("hemiLight", new Vector3(0, 1, 0), scene);
    hemisphericLight.intensity = 0.35;
    // Use DaisyUI base-300 color for ambient lighting
    const ambientColor = getDaisyUIColor('b3');
    hemisphericLight.diffuse = ambientColor.scale(0.4);
    hemisphericLight.specular = new Color3(0, 0, 0);
    
    const keyLight = new DirectionalLight("keyLight", new Vector3(-0.5, -1, -0.5), scene);
    keyLight.position = new Vector3(50, 100, 50);
    keyLight.intensity = 0.6;
    // Use DaisyUI primary color for key lighting
    const primaryColor = getDaisyUIColor('p');
    keyLight.diffuse = primaryColor.scale(0.8);
    keyLight.specular = primaryColor.scale(1.2);
    
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
        
        // Use DaisyUI text color for grid lines
        const gridColor = getDaisyUIColor('bc');
        line.color = gridColor;
        line.alpha = alpha;
        
        gridLines.push(line);
        glowLayer.addIncludedOnlyMesh(line);
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
        
        // Use DaisyUI text color for grid lines
        const gridColor = getDaisyUIColor('bc');
        line.color = gridColor;
        line.alpha = alpha;
        
        gridLines.push(line);
        glowLayer.addIncludedOnlyMesh(line);
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
      new Color3(1, 0, 1),
      new Color3(1, 1, 0),
      new Color3(1, 0.3, 0),
      new Color3(0, 1, 0.3),
      new Color3(0.3, 0, 1),
      new Color3(1, 0, 0.3),
      new Color3(0.3, 1, 0),
      new Color3(1, 0.5, 0.8)
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
      
      const material = new StandardMaterial(`movingShapeMaterial${i}`, scene);
      const color = neonColors[i % neonColors.length];
      
      material.diffuseColor = color.scale(0.6);
      material.emissiveColor = color.scale(0.4);
      material.specularColor = new Color3(0.5, 0.5, 0.5);
      material.specularPower = 64;
      
      shape.material = material;
      glowLayer.addIncludedOnlyMesh(shape);

      shape.position = new Vector3(
        (Math.random() - 0.5) * 300,
        Math.random() * 40 + 5,
        (Math.random() - 0.5) * 300
      );
      
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
    
    scene.registerBeforeRender(() => {
      movingShapes.forEach((shapeData, index) => {
        const { mesh, velocity, rotationSpeed, bounds } = shapeData;
        
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
      new Vector3(0, 0, 0),
      new Vector3(80, 0, 80),
      new Vector3(-80, 0, 80),
      new Vector3(-80, 0, -80),
      new Vector3(80, 0, -80),
      new Vector3(0, 0, 120),
      new Vector3(120, 0, 0),
      new Vector3(0, 0, -120),
      new Vector3(-120, 0, 0)
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
    
    scene.registerBeforeRender(() => {
      if (movingShapes.length > 0) {
        const time = performance.now() * 0.0005;
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
      
      // Watch for theme changes on the document element
      const themeObserver = new MutationObserver((mutations) => {
        mutations.forEach((mutation) => {
          if (mutation.type === 'attributes' && mutation.attributeName === 'data-theme') {
            // Small delay to ensure CSS variables are updated
            setTimeout(() => {
              updateSceneColors();
            }, 50);
          }
        });
      });
      
      themeObserver.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['data-theme']
      });
      
      onCleanup(() => {
        window.removeEventListener('resize', handleResize);
        themeObserver.disconnect();
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