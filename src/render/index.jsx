import { onMount, onCleanup, createSignal, createEffect } from 'solid-js';
import { Engine } from '@babylonjs/core/Engines/engine';
import { Scene } from '@babylonjs/core/scene';
import { UniversalCamera } from '@babylonjs/core/Cameras/universalCamera';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { Ray } from '@babylonjs/core/Culling/ray';
import { DirectionalLight } from '@babylonjs/core/Lights/directionalLight';
import { PointLight } from '@babylonjs/core/Lights/pointLight';
import { ShadowGenerator } from '@babylonjs/core/Lights/Shadows/shadowGenerator';
import { ReflectionProbe } from '@babylonjs/core/Probes/reflectionProbe';
import '@babylonjs/core/Lights/Shadows/shadowGeneratorSceneComponent';
import { Color4 } from '@babylonjs/core/Maths/math.color';
import { CreateBox } from '@babylonjs/core/Meshes/Builders/boxBuilder';
import { CreateGround } from '@babylonjs/core/Meshes/Builders/groundBuilder';
import { CreateLines } from '@babylonjs/core/Meshes/Builders/linesBuilder';
import { CreateSphere } from '@babylonjs/core/Meshes/Builders/sphereBuilder';
import { CubeTexture } from '@babylonjs/core/Materials/Textures/cubeTexture';
import '@babylonjs/core/Materials/Textures/Loaders/envTextureLoader';
import { CreateSkyBox } from '@babylonjs/core/Helpers/environmentHelper';
import { SkyMaterial } from '@babylonjs/materials/sky/skyMaterial';
import '@babylonjs/materials/sky';
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { PBRMaterial } from '@babylonjs/core/Materials/PBR/pbrMaterial';
import { HDRCubeTexture } from '@babylonjs/core/Materials/Textures/hdrCubeTexture';
import { Color3 } from '@babylonjs/core/Maths/math.color';
import { ImageProcessingConfiguration } from '@babylonjs/core/Materials/imageProcessingConfiguration';
import '@babylonjs/core/Materials/Textures/Loaders/hdrTextureLoader';
import { ParticleSystem } from '@babylonjs/core/Particles/particleSystem';
import { Texture } from '@babylonjs/core/Materials/Textures/texture';
import '@babylonjs/core/Cameras/Inputs/arcRotateCameraPointersInput';
import '@babylonjs/core/Cameras/Inputs/arcRotateCameraKeyboardMoveInput';
import '@babylonjs/core/Cameras/Inputs/arcRotateCameraMouseWheelInput';
import { GizmoManager } from '@babylonjs/core/Gizmos/gizmoManager';
import { UtilityLayerRenderer } from '@babylonjs/core/Rendering/utilityLayerRenderer';
import { HighlightLayer } from '@babylonjs/core/Layers/highlightLayer';
import '@babylonjs/core/Layers/effectLayerSceneComponent';
import '@babylonjs/core/Materials/standardMaterial';
import { Mesh } from '@babylonjs/core/Meshes/mesh';
import { CreateCylinder } from '@babylonjs/core/Meshes/Builders/cylinderBuilder';
import { CreateTorus } from '@babylonjs/core/Meshes/Builders/torusBuilder';
import { DynamicTexture } from '@babylonjs/core/Materials/Textures/dynamicTexture';
import { SceneLoader } from '@babylonjs/core/Loading/sceneLoader';
import '@babylonjs/loaders/glTF';
import { HavokPlugin } from '@babylonjs/core/Physics/v2/Plugins/havokPlugin';
import HavokPhysics from '@babylonjs/havok';
import '@babylonjs/core/Physics/physicsEngineComponent';
import { bridgeService } from '@/plugins/core/bridge';
import { renderStore, renderActions } from './store.jsx';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { grid } from './hooks/grid.jsx';
import { renderShortcuts } from './hooks/renderShortcuts';
import { useCameraController } from './hooks/cameraMovement.jsx';
import { GizmoManagerComponent } from './hooks/gizmo.jsx';
import { useAssetLoader } from './hooks/assetLoader.jsx';
import { LoadingTooltip } from './components/LoadingTooltip.jsx';
import { pluginAPI } from '@/api/plugin';
import { viewportStore } from '@/layout/stores/ViewportStore.jsx';



const loadDefaultSceneContent = (scene, canvas) => {
  console.log('🌟 Loading default scene content');
  
  // Create default camera positioned diagonally to show X and Z axis intersection
  const camera = new UniversalCamera(
    'camera',
    new Vector3(7, 5, 7),
    scene
  );
  // Don't attach Babylon's native controls - we use custom camera controller
  camera.setTarget(Vector3.Zero());


  // Unreal Engine style lighting setup
  
  // Configure image processing for realistic tone mapping
  scene.imageProcessingConfiguration.toneMappingEnabled = true;
  scene.imageProcessingConfiguration.toneMappingType = ImageProcessingConfiguration.TONEMAPPING_ACES;
  scene.imageProcessingConfiguration.exposure = 1.0;
  scene.imageProcessingConfiguration.contrast = 1.1;
  scene.imageProcessingConfiguration.vignetteEnabled = false;
  
  // Day/Night Cycle System
  const dayNightCycle = {
    timeOfDay: 12.0, // Current hour (0-24, 12 = noon)
    speed: 0.2, // Hours per minute real time (0.2 = 1 hour per 5 minutes)
    enabled: true,
    // Configurable timing
    sunriseHour: 6.0,    // When sun starts rising
    sunsetHour: 21.0,    // When sun starts setting
    transitionDuration: 1.0 // Hours for sunrise/sunset transitions
  };

  // Sky material
  var skyboxMaterial = new SkyMaterial("skyMaterial", scene);
  skyboxMaterial.backFaceCulling = false;
  
  // Add clouds to the sky
  skyboxMaterial.cloudsEnabled = true;
  skyboxMaterial.cumulusCloudSize = 20;
  skyboxMaterial.cumulusCloudDensity = 0.3;

  // Sky mesh (sphere)
  var skybox = CreateSphere("skyBox", { diameter: 1000.0 }, scene);
  skybox.material = skyboxMaterial;
  skybox.infiniteDistance = true;
  
  // Create ground plane to receive shadows
  const ground = CreateGround("ground", { width: 200, height: 200 }, scene);
  ground.receiveShadows = true;
  ground.position.y = 0;
  
  // Ground material
  const groundMaterial = new PBRMaterial("groundMaterial", scene);
  groundMaterial.baseColor = new Color3(0.4, 0.5, 0.3); // Earthy green
  groundMaterial.metallicFactor = 0.0;
  groundMaterial.roughnessFactor = 0.9;
  groundMaterial.enableSpecularAntiAliasing = true;
  
  // Enable reflections from environment
  groundMaterial.environmentIntensity = 1.0;
  groundMaterial.usePhysicalLightFalloff = true;
  ground.material = groundMaterial;
  ground._isInternalMesh = true;
  
  // Sky light - will be controlled by day/night cycle
  const skyLight = new HemisphericLight('skyLight', new Vector3(0, 1, 0), scene);
  skyLight.intensity = 1.0; // Initial intensity, will be updated by cycle
  skyLight.diffuse = new Color3(0.8, 0.9, 1.0); // Initial color, will be updated
  skyLight.groundColor = new Color3(0.6, 0.55, 0.5); // Initial ground color
  
  // Main directional light - will be controlled by day/night cycle
  const sunLight = new DirectionalLight('sunLight', new Vector3(-0.3, -0.8, -0.5), scene);
  sunLight.intensity = 1.0; // Initial intensity, will be updated by cycle
  sunLight.diffuse = new Color3(1.0, 0.98, 0.9); // Initial color, will be updated
  sunLight.specular = new Color3(1.0, 0.95, 0.85);
  
  // Realistic sun position (45 degree elevation, southeast)
  const sunElevation = 45 * Math.PI / 180;
  const sunAzimuth = 130 * Math.PI / 180;
  sunLight.direction = new Vector3(
    Math.cos(sunElevation) * Math.cos(sunAzimuth),
    -Math.sin(sunElevation),
    Math.cos(sunElevation) * Math.sin(sunAzimuth)
  );
  
  // Rim light - simulates atmospheric scattering
  const rimLight = new DirectionalLight('rimLight', new Vector3(0.8, 0.2, -0.6), scene);
  rimLight.intensity = 0.4;
  rimLight.diffuse = new Color3(0.9, 0.7, 0.5); // Warm rim
  rimLight.specular = new Color3(0.0, 0.0, 0.0); // No specular for rim
  
  // Bounce light - simulates indirect lighting
  const bounceLight = new DirectionalLight('bounceLight', new Vector3(-0.2, 0.3, 0.9), scene);
  bounceLight.intensity = 0.3;
  bounceLight.diffuse = new Color3(0.4, 0.5, 0.7); // Cool bounce from sky
  bounceLight.specular = new Color3(0.0, 0.0, 0.0); // No specular for bounce

  // Enhanced shadow generator with Unreal-style settings
  const shadowGenerator = new ShadowGenerator(4096, sunLight);
  shadowGenerator.usePercentageCloserFiltering = true;
  shadowGenerator.filteringQuality = ShadowGenerator.QUALITY_HIGH;
  shadowGenerator.darkness = 0.3; // Softer shadows for realism
  shadowGenerator.bias = 0.00005; // Reduced bias for cleaner shadows
  
  // Contact hardening for realistic shadow softness
  shadowGenerator.useContactHardeningShadow = true;
  shadowGenerator.contactHardeningLightSizeUVRatio = 0.05; // Tighter contact hardening
  
  // Cascade shadow maps for better distance shadows
  shadowGenerator.useCascades = true;
  shadowGenerator.numCascades = 4;
  shadowGenerator.cascadeBlendPercentage = 0.1;
  
  // Exponential shadow maps for softer shadows
  shadowGenerator.useExponentialShadowMap = true;
  shadowGenerator.blurKernel = 64; // Larger blur for softer edges
  
  // Store shadow generator for access by physics objects
  scene.shadowGenerator = shadowGenerator;
  
  // Set environment intensity for realistic IBL
  scene.environmentIntensity = 1.2; // Higher for brighter environment
  
  // Create reflection probe to capture sky material for reflections
  const reflectionProbe = new ReflectionProbe('skyReflection', 512, scene);
  reflectionProbe.renderList.push(skybox);
  scene.environmentTexture = reflectionProbe.cubeTexture;
  
  // Configure global lighting settings for Unreal-style rendering
  scene.autoClear = true;
  scene.autoClearDepthAndStencil = true;
  
  // Enable realistic fog for depth and atmosphere
  scene.fogEnabled = true;
  scene.fogMode = 2; // FOGMODE_EXP2
  scene.fogDensity = 0.001; // Very light atmospheric fog
  scene.fogColor = new Color3(0.7, 0.8, 0.9); // Light blue-gray fog
  

  // Create snow particle system
  const snowSystem = new ParticleSystem('snow', 2000, scene);
  
  // Create a simple white circle texture for snowflakes
  const snowTexture = new DynamicTexture('snowTexture', { width: 64, height: 64 }, scene);
  const snowContext = snowTexture.getContext();
  snowContext.clearRect(0, 0, 64, 64);
  snowContext.fillStyle = 'white';
  snowContext.beginPath();
  snowContext.arc(32, 32, 24, 0, 2 * Math.PI);
  snowContext.fill();
  
  // Add soft edge
  const snowGradient = snowContext.createRadialGradient(32, 32, 0, 32, 32, 24);
  snowGradient.addColorStop(0, 'rgba(255, 255, 255, 1)');
  snowGradient.addColorStop(0.8, 'rgba(255, 255, 255, 0.8)');
  snowGradient.addColorStop(1, 'rgba(255, 255, 255, 0)');
  snowContext.fillStyle = snowGradient;
  snowContext.beginPath();
  snowContext.arc(32, 32, 24, 0, 2 * Math.PI);
  snowContext.fill();
  snowTexture.update();
  
  snowSystem.particleTexture = snowTexture;
  snowSystem.emitter = new Vector3(0, 20, 0); // Emit from above
  snowSystem.minEmitBox = new Vector3(-25, 0, -25); // Spread area
  snowSystem.maxEmitBox = new Vector3(25, 0, 25);
  
  // Snow particle behavior
  snowSystem.color1 = new Color4(1, 1, 1, 0.8);
  snowSystem.color2 = new Color4(0.9, 0.9, 1, 0.6);
  snowSystem.colorDead = new Color4(1, 1, 1, 0);
  
  snowSystem.minSize = 0.02;
  snowSystem.maxSize = 0.08;
  snowSystem.minLifeTime = 8;
  snowSystem.maxLifeTime = 12;
  snowSystem.emitRate = 100;
  
  // Gentle falling motion
  snowSystem.gravity = new Vector3(0, -1.5, 0);
  snowSystem.direction1 = new Vector3(-0.2, -1, -0.2);
  snowSystem.direction2 = new Vector3(0.2, -1, 0.2);
  snowSystem.minAngularSpeed = 0;
  snowSystem.maxAngularSpeed = Math.PI;
  
  // Add slight wind drift
  snowSystem.minEmitPower = 0.5;
  snowSystem.maxEmitPower = 1.0;
  
  snowSystem.start();

  // Create star particle system for hemisphere coverage
  const starSystem = new ParticleSystem('stars', 800, scene);
  
  // Create small white dot texture for stars
  const starTexture = new DynamicTexture('starTexture', { width: 16, height: 16 }, scene);
  const starContext = starTexture.getContext();
  starContext.clearRect(0, 0, 16, 16);
  starContext.fillStyle = 'white';
  starContext.beginPath();
  starContext.arc(8, 8, 6, 0, 2 * Math.PI);
  starContext.fill();
  starTexture.update();
  
  starSystem.particleTexture = starTexture;
  starSystem.emitter = new Vector3(0, 0, 0);
  starSystem.minEmitBox = new Vector3(-450, 50, -450); // Wide spread, above horizon
  starSystem.maxEmitBox = new Vector3(450, 450, 450); // Full hemisphere coverage
  
  // Star properties
  starSystem.color1 = new Color4(1, 1, 1, 1);
  starSystem.color2 = new Color4(0.9, 0.9, 1, 0.8);
  starSystem.colorDead = new Color4(1, 1, 1, 0);
  
  starSystem.minSize = 0.1;
  starSystem.maxSize = 0.3;
  
  // Make stars emit light
  starSystem.blendMode = ParticleSystem.BLENDMODE_ADD; // Additive blending for glow effect
  starSystem.minLifeTime = 999999; // Very long lifetime
  starSystem.maxLifeTime = 999999;
  starSystem.emitRate = 0; // Don't emit continuously
  
  // No gravity or movement - stars should be stationary
  starSystem.gravity = Vector3.Zero();
  starSystem.direction1 = Vector3.Zero();
  starSystem.direction2 = Vector3.Zero();
  starSystem.minEmitPower = 0;
  starSystem.maxEmitPower = 0;
  
  // Don't start immediately - will be controlled by day/night cycle
  // starSystem.start();

  // Create moon with light
  const moon = CreateSphere('moon', { diameter: 20 }, scene);
  moon.position = new Vector3(100, 300, 200);
  const moonMaterial = new PBRMaterial('moonMaterial', scene);
  moonMaterial.baseColor = new Color3(0.9, 0.9, 0.8);
  moonMaterial.emissiveColor = new Color3(0.3, 0.3, 0.25);
  moonMaterial.metallicFactor = 0.0;
  moonMaterial.roughnessFactor = 0.8;
  moonMaterial.disableLighting = true;
  moon.material = moonMaterial;
  moon._isInternalMesh = true;
  
  // Moon light source
  const moonLight = new PointLight('moonLight', moon.position, scene);
  moonLight.diffuse = new Color3(0.3, 0.3, 0.4);
  moonLight.specular = new Color3(0.2, 0.2, 0.3);
  moonLight.intensity = 0;
  moonLight.range = 1000;


  // Function to update day/night cycle
  const updateDayNightCycle = () => {
    if (!dayNightCycle.enabled) return;
    
    // Advance time (speed is hours per minute)
    dayNightCycle.timeOfDay += dayNightCycle.speed * (1/60); // Convert to hours per frame
    if (dayNightCycle.timeOfDay >= 24) dayNightCycle.timeOfDay = 0;
    
    const currentHour = dayNightCycle.timeOfDay;
    
    // Check if it's after sunset (needed for stars and moon)
    const isAfterSunset = currentHour > dayNightCycle.sunsetHour || currentHour < dayNightCycle.sunriseHour;
    
    // Calculate periods based on configurable values
    const dawnStart = dayNightCycle.sunriseHour - dayNightCycle.transitionDuration;
    const dayStart = dayNightCycle.sunriseHour;
    const duskStart = dayNightCycle.sunsetHour;
    const nightStart = dayNightCycle.sunsetHour + dayNightCycle.transitionDuration;
    
    let sunElevation, lightIntensity;
    
    if (currentHour >= dayNightCycle.sunriseHour && currentHour <= dayNightCycle.sunsetHour) {
      // Clean arc from sunrise to sunset
      const dayDuration = dayNightCycle.sunsetHour - dayNightCycle.sunriseHour; // 12 hours (6am to 6pm)
      const dayProgress = (currentHour - dayNightCycle.sunriseHour) / dayDuration; // 0 to 1
      
      // Simple sine arc: starts at 0 (sunrise), peaks at 0.5 (noon), ends at 0 (sunset)
      const sunAngle = dayProgress * Math.PI; // 0 to PI
      sunElevation = Math.sin(sunAngle); // 0 to 1 to 0
      lightIntensity = sunElevation;
    } else {
      // Night time (including transitions)
      sunElevation = 0;
      lightIntensity = 0.0; // Pitch black
    }
    
    // Calculate sun position for SkyMaterial
    // SkyMaterial inclination: 0 = zenith (high), -0.5 = horizon (low)
    let inclination = -0.5 + (sunElevation * 0.5); // -0.5 (horizon) to 0.0 (zenith)
    
    // Calculate sun azimuth (east to west movement)
    let azimuth = 0.25; // Default position
    if (currentHour >= dayNightCycle.sunriseHour && currentHour <= dayNightCycle.sunsetHour) {
      const dayProgress = (currentHour - dayNightCycle.sunriseHour) / (dayNightCycle.sunsetHour - dayNightCycle.sunriseHour); // 0 to 1
      azimuth = dayProgress * 0.5; // 0 (east) to 0.5 (west)
    }
    
    // Update SkyMaterial - all properties gradual
    skyboxMaterial.inclination = lightIntensity > 0 ? inclination : -1.0; // Hide sun below horizon at night
    skyboxMaterial.azimuth = azimuth;
    skyboxMaterial.turbidity = 2 + ((1 - lightIntensity) * 198); // 2 (clear day) to 200 (black night)
    skyboxMaterial.luminance = lightIntensity * 1.0; // 0.0 (black) to 1.0 (bright)
    
    // Remove sun glare at night by hiding sun disk
    if (lightIntensity > 0.1) {
      skyboxMaterial.sunPosition = new Vector3(0, 1, 0); // Sun visible during day
    } else {
      skyboxMaterial.sunPosition = new Vector3(0, -20, 0); // Hide sun completely at night
    }
    
    // Update moon visibility and light - only at night
    const moonVisibility = isAfterSunset ? Math.max(0.5, 1 - lightIntensity) : 0.0;
    if (moon && moon.material) {
      moon.material.alpha = moonVisibility;
      // Make moon glow brighter at night
      moon.material.emissiveColor = new Color3(
        moonVisibility * 0.4,
        moonVisibility * 0.4, 
        moonVisibility * 0.35
      );
    }
    
    // Update moon light intensity
    if (moonLight) {
      moonLight.intensity = moonVisibility * 15; // Bright moonlight at night
    }
    
    // Update directional light (sun/moon) - always gradual
    sunLight.intensity = lightIntensity * 4.0;
    
    if (lightIntensity > 0.1) {
      // Strong sunrise/sunset colors
      const warmth = 1.0 - sunElevation; // 0 at noon, 1 at sunrise/sunset
      if (warmth > 0.7) {
        // Deep sunrise/sunset: intense orange/red
        sunLight.diffuse = new Color3(1.0, 0.4, 0.1); // Bright orange-red
      } else if (warmth > 0.4) {
        // Mid sunrise/sunset: warm yellow-orange
        sunLight.diffuse = new Color3(1.0, 0.7, 0.2); // Golden orange
      } else {
        // Noon: bright white-yellow
        sunLight.diffuse = new Color3(1.0, 0.98, 0.9); // Clean daylight
      }
    } else {
      // Night: cool blue moonlight
      sunLight.diffuse = new Color3(0.2, 0.2, 0.4);
    }
    
    // Update sky light (ambient) - gradual based on light intensity
    skyLight.intensity = lightIntensity * 4.0; // Pure scaling, no base intensity
    
    // Color transitions gradually from night blue to day blue
    const dayColorMix = Math.max(0, Math.min(1, lightIntensity * 2)); // 0 to 1
    skyLight.diffuse = new Color3(
      0.02 + (dayColorMix * 0.68), // 0.02 to 0.7 (red)
      0.02 + (dayColorMix * 0.78), // 0.02 to 0.8 (green) 
      0.05 + (dayColorMix * 0.95)  // 0.05 to 1.0 (blue)
    );
    
    skyLight.groundColor = new Color3(
      dayColorMix * 0.4, // 0 to 0.4 (red)
      dayColorMix * 0.3, // 0 to 0.3 (green)
      dayColorMix * 0.2  // 0 to 0.2 (blue)
    );
    
    // Update rim light - gradual with day/night cycle
    rimLight.intensity = lightIntensity * 0.4;
    
    const rimColorMix = Math.max(0, Math.min(1, lightIntensity));
    rimLight.diffuse = new Color3(
      0.1 + (rimColorMix * 0.8), // 0.1 to 0.9 (red)
      0.1 + (rimColorMix * 0.6), // 0.1 to 0.7 (green)
      0.2 + (rimColorMix * 0.3)  // 0.2 to 0.5 (blue)
    );
    
    // Update bounce light - gradual with day/night cycle
    bounceLight.intensity = lightIntensity * 0.3;
    
    const bounceColorMix = Math.max(0, Math.min(1, lightIntensity));
    bounceLight.diffuse = new Color3(
      0.05 + (bounceColorMix * 0.35), // 0.05 to 0.4 (red)
      0.05 + (bounceColorMix * 0.45), // 0.05 to 0.5 (green)
      0.1 + (bounceColorMix * 0.6)    // 0.1 to 0.7 (blue)
    );
    
    // Control star system based on time
    if (isAfterSunset && lightIntensity < 0.2) {
      // Start stars at night if not already started
      if (!starSystem.isStarted()) {
        starSystem.manualEmitCount = 800;
        starSystem.start();
      }
      
      // Make stars twinkle
      if (starSystem.particles) {
        starSystem.particles.forEach((particle, index) => {
          if (particle.color) {
            const individualTwinkle = Math.sin(Date.now() * 0.002 + index * 0.1) * 0.3 + 0.7;
            particle.color.a = individualTwinkle;
          }
        });
      }
    } else {
      // Hide stars during day
      if (starSystem.isStarted()) {
        starSystem.stop();
        starSystem.reset();
      }
    }
    
    // Update time display
    const hours = Math.floor(currentHour);
    const minutes = Math.floor((currentHour - hours) * 60);
    const timeString = `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}`;
    
    const timeDisplay = document.getElementById('time-display');
    if (timeDisplay) {
      timeDisplay.textContent = timeString;
    }
  };
  
  // Register update function to render loop
  scene.registerBeforeRender(updateDayNightCycle);
  
  // Store day/night system globally for external control
  window._dayNightCycle = dayNightCycle;

  // Set camera in render store
  renderActions.setCamera(camera);
  
  console.log('✅ Default scene content loaded with Unreal-style lighting');
};

export default function BabylonRenderer(props) {
  let canvasRef;
  const [engine, setEngine] = createSignal(null);
  const [scene, setScene] = createSignal(null);
  
  // Initialize asset loader
  const { loadingTooltip, handleDragOver, handleDragEnter, handleDragLeave, handleDrop, loadAssetIntoScene, isPositioning } = useAssetLoader(scene, () => canvasRef);

  // Show UI panels when this 3D viewport is active
  createEffect(() => {
    const activeTabId = viewportStore.activeTabId;
    const activeTab = viewportStore.tabs.find(t => t.id === activeTabId);
    
    if (activeTab?.type === '3d-viewport') {
      pluginAPI.showPanel();
      pluginAPI.showProps();
      pluginAPI.showMenu();
      pluginAPI.showTabs();
    }
  });

  onMount(() => {
    initializeBabylon();
  });

  // Initialize grid system
  grid(scene);

  // Initialize camera controller
  useCameraController(() => scene()?.activeCamera, () => canvasRef, scene);

  // Initialize global shortcuts (no movement keys)
  renderShortcuts({
    // Transform gizmos
    positionMode: () => {
      const gizmoManager = renderStore.gizmoManager;
      if (gizmoManager) {
        gizmoManager.positionGizmoEnabled = !gizmoManager.positionGizmoEnabled;
        gizmoManager.rotationGizmoEnabled = false;
        gizmoManager.scaleGizmoEnabled = false;
      }
    },
    rotateMode: () => {
      const gizmoManager = renderStore.gizmoManager;
      if (gizmoManager) {
        gizmoManager.rotationGizmoEnabled = !gizmoManager.rotationGizmoEnabled;
        gizmoManager.positionGizmoEnabled = false;
        gizmoManager.scaleGizmoEnabled = false;
      }
    },
    scaleMode: () => {
      const gizmoManager = renderStore.gizmoManager;
      if (gizmoManager) {
        gizmoManager.scaleGizmoEnabled = !gizmoManager.scaleGizmoEnabled;
        gizmoManager.positionGizmoEnabled = false;
        gizmoManager.rotationGizmoEnabled = false;
      }
    },
    // Focus on selected object
    focusObject: () => {
      const selectedObject = renderStore.selectedObject;
      const camera = renderStore.camera;
      
      if (!selectedObject || !camera) {
        console.log('⚠️ No object selected or no camera available for focus');
        return;
      }
      
      console.log('🎯 Focusing camera on object:', selectedObject.name);
      
      // Get the bounding box of the selected object
      let boundingInfo, center, size;
      
      if (selectedObject.getClassName() === 'TransformNode') {
        // For TransformNode containers, use hierarchy bounding box
        const hierarchyBounds = selectedObject.getHierarchyBoundingVectors();
        center = hierarchyBounds.min.add(hierarchyBounds.max).scale(0.5);
        size = hierarchyBounds.max.subtract(hierarchyBounds.min);
      } else {
        // For regular meshes, use standard bounding box
        boundingInfo = selectedObject.getBoundingInfo();
        center = boundingInfo.boundingBox.centerWorld;
        size = boundingInfo.boundingBox.extendSizeWorld;
      }
      
      // Calculate distance to fit entire object in camera view
      const maxDimension = Math.max(size.x, size.y, size.z);
      const cameraFov = camera.fov || 0.8; // Default FOV in radians
      
      // Calculate distance needed to fit object in view
      // Using half FOV and some padding for better framing
      const padding = 1.2; // 20% padding around object
      const focusDistance = Math.max(1, (maxDimension * padding) / (2 * Math.tan(cameraFov / 2)));
      
      console.log(`Object size: ${maxDimension.toFixed(2)}, FOV: ${(cameraFov * 180/Math.PI).toFixed(1)}°, focus distance: ${focusDistance.toFixed(2)}`);
      
      // Position camera at a safe angle (45 degrees up and back) to avoid going through object
      const backDirection = new Vector3(1, 1, 1).normalize(); // Back, up, and to the side
      const newCameraPosition = center.add(backDirection.scale(focusDistance));
      
      // Smooth animation to new position
      const startPosition = camera.position.clone();
      const animationDuration = 500; // ms
      const startTime = Date.now();
      
      const animate = () => {
        const elapsed = Date.now() - startTime;
        const progress = Math.min(elapsed / animationDuration, 1);
        
        // Smooth easing function (ease-out)
        const easedProgress = 1 - Math.pow(1 - progress, 3);
        
        // Interpolate position
        camera.position = Vector3.Lerp(startPosition, newCameraPosition, easedProgress);
        
        // Set target to object center
        if (camera.setTarget) {
          camera.setTarget(center);
        }
        
        if (progress < 1) {
          requestAnimationFrame(animate);
        } else {
          console.log('✅ Camera focused on object');
          editorActions.addConsoleMessage(`Focused camera on "${selectedObject.name}"`, 'success');
        }
      };
      
      animate();
    },
    // Focus on mouse point
    focusOnMousePoint: () => {
      const camera = renderStore.camera;
      const currentScene = renderStore.scene;
      
      if (!camera || !currentScene) {
        console.log('⚠️ No camera or scene available for focus');
        return;
      }
      
      // Get current mouse position from last known coordinates
      const canvas = canvasRef;
      if (!canvas) {
        console.log('⚠️ No canvas available');
        return;
      }
      
      // Cast ray from camera through mouse position
      const pickInfo = currentScene.pick(
        currentScene.pointerX || canvas.width / 2,
        currentScene.pointerY || canvas.height / 2
      );
      
      if (pickInfo && pickInfo.hit && pickInfo.pickedPoint) {
        const targetPoint = pickInfo.pickedPoint;
        console.log('🎯 Focusing camera on point:', targetPoint);
        
        // Calculate camera position maintaining current distance but looking at picked point
        const currentDistance = Vector3.Distance(camera.position, camera.getTarget());
        const direction = camera.position.subtract(targetPoint).normalize();
        const newCameraPosition = targetPoint.add(direction.scale(currentDistance));
        
        // Smooth animation to new position
        const startPosition = camera.position.clone();
        const animationDuration = 500; // ms
        const startTime = Date.now();
        
        const animate = () => {
          const elapsed = Date.now() - startTime;
          const progress = Math.min(elapsed / animationDuration, 1);
          
          // Smooth easing function (ease-out)
          const easedProgress = 1 - Math.pow(1 - progress, 3);
          
          // Interpolate position
          camera.position = Vector3.Lerp(startPosition, newCameraPosition, easedProgress);
          
          // Set target to picked point
          if (camera.setTarget) {
            camera.setTarget(targetPoint);
          }
          
          if (progress < 1) {
            requestAnimationFrame(animate);
          } else {
            console.log('✅ Camera focused on mouse point');
            editorActions.addConsoleMessage('Focused camera on mouse point', 'success');
          }
        };
        
        animate();
      } else {
        console.log('⚠️ No surface found at mouse position');
        editorActions.addConsoleMessage('No surface found at mouse position', 'warning');
      }
    },
    // Delete selected object
    deleteObject: () => {
      const selectedObject = renderStore.selectedObject;
      
      if (!selectedObject) {
        console.log('⚠️ No object selected to delete');
        return;
      }
      
      
      console.log('🗑️ Deleting object:', selectedObject.name);
      
      // Remove from render store (this also updates the hierarchy)
      renderActions.removeObject(selectedObject);
      
      console.log('✅ Object deleted successfully');
    },
    // Snap to ground
    snapToGround: () => {
      const selectedObject = renderStore.selectedObject;
      const currentScene = renderStore.scene;
      
      if (!selectedObject || !currentScene) {
        console.log('⚠️ No object selected or no scene available for snap to ground');
        return;
      }
      
      console.log('📍 Snapping object to ground:', selectedObject.name);
      
      // Cast ray downward from object position to find ground
      const ray = new Ray(selectedObject.position.add(new Vector3(0, 100, 0)), new Vector3(0, -1, 0));
      const hit = currentScene.pickWithRay(ray, (mesh) => {
        // Exclude the selected object itself and any gizmo/helper objects
        return mesh !== selectedObject && 
               !mesh.name.includes('gizmo') && 
               !mesh.name.includes('helper') &&
               !mesh.name.startsWith('__');
      });
      
      if (hit && hit.hit && hit.pickedPoint) {
        // Get the bounding box of the selected object to calculate proper offset
        const boundingInfo = selectedObject.getBoundingInfo();
        const yOffset = Math.abs(boundingInfo.minimum.y);
        
        // Snap to the surface with proper offset so object sits on top
        selectedObject.position.y = hit.pickedPoint.y + yOffset;
        console.log('✅ Object snapped to surface at Y:', hit.pickedPoint.y + yOffset);
        
        // Add console message to editor
        editorActions.addConsoleMessage(`Snapped "${selectedObject.name}" to surface`, 'success');
      } else {
        // Fallback: snap to Y=0 (ground plane)
        const boundingInfo = selectedObject.getBoundingInfo();
        const yOffset = Math.abs(boundingInfo.minimum.y);
        selectedObject.position.y = yOffset;
        console.log('⬇️ No surface found, snapped to ground plane at Y:', yOffset);
        
        // Add console message to editor
        editorActions.addConsoleMessage(`Snapped "${selectedObject.name}" to ground plane`, 'info');
      }
    }
  });

  onCleanup(() => {
    cleanup();
  });

  // Watch for viewport settings changes
  createEffect(() => {
    const viewportSettings = editorStore.settings.viewport;
    if (scene() && viewportSettings.backgroundColor) {
      renderActions.updateSettings({
        backgroundColor: viewportSettings.backgroundColor
      });
    }
  });

  const initializeBabylon = async () => {
    if (!canvasRef) return;

    try {
      console.log('🎮 Initializing Babylon.js...');

      // Create engine
      const babylonEngine = new Engine(canvasRef, true, {
        adaptToDeviceRatio: true,
        antialias: true
      });

      // Create scene
      const babylonScene = new Scene(babylonEngine);
      babylonScene.useRightHandedSystem = true;
      babylonScene.clearColor = new Color4(0.1, 0.1, 0.15, 1);
      // Lower overall exposure to avoid overly bright results
      babylonScene.imageProcessingConfiguration.exposure = 0.85;
      
      // Enable FXAA for better anti-aliasing on lines and edges
      babylonScene.imageProcessingConfiguration.fxaaEnabled = true;

      // Enable Havok physics for RenScript
      try {
        const havokInstance = await HavokPhysics();
        const hk = new HavokPlugin(true, havokInstance);
        const enableResult = babylonScene.enablePhysics(new Vector3(0, -9.81, 0), hk);
        console.log('✅ Havok physics enabled for RenScript, result:', enableResult);
      } catch (error) {
        console.warn('⚠️ Failed to enable Havok physics:', error);
      }

      // Scene content will be loaded separately
      console.log('🎮 Scene created, ready for content loading');

      // Create highlight layer for selection
      const highlightLayer = new HighlightLayer('highlight', babylonScene);
      
      // Store in render store (gizmo manager will be set by GizmoManagerComponent)
      renderActions.setHighlightLayer(highlightLayer);

      // Start render loop
      babylonEngine.runRenderLoop(() => {
        babylonScene.render();
      });

      // Handle resize
      window.addEventListener('resize', () => {
        babylonEngine.resize();
      });


      // Add object picking for selection - only on LEFT CLICK to avoid interfering with camera
      babylonScene.onPointerObservable.add((pointerInfo) => {
        // Only process left-click events (button 0) - let right-click pass through to camera controller
        if (pointerInfo.type === 1 && pointerInfo.event && pointerInfo.event.button === 0) { // LEFT CLICK only
          if (pointerInfo.pickInfo?.hit && pointerInfo.pickInfo.pickedMesh) {
            let targetObject = pointerInfo.pickInfo.pickedMesh;
            console.log('🎯 Clicked on mesh:', targetObject.name, 'class:', targetObject.getClassName());
            
            // Walk up the hierarchy to find the top-level selectable object
            // Keep walking up until we reach a root object (no parent)
            console.log('🔍 Starting hierarchy walk from:', targetObject.name, 'ID:', targetObject.uniqueId);
            console.log('🔍 Initial parent check:', targetObject.parent?.name || 'none', 'ID:', targetObject.parent?.uniqueId || 'none');
            
            // Walk up until we reach a true root object (no parent) 
            // Don't stop at system objects if they have parents - keep going to reach our container
            let walkCount = 0;
            while (targetObject.parent && walkCount < 10) { // Safety limit
              walkCount++;
              
              console.log('⬆️ Walking up hierarchy from', targetObject.name, '(ID:', targetObject.uniqueId, ') to parent', targetObject.parent.name, '(ID:', targetObject.parent.uniqueId, ') parent class:', targetObject.parent.getClassName());
              targetObject = targetObject.parent;
              console.log('⬆️ Now at node:', targetObject.name, '(ID:', targetObject.uniqueId, ')');
              console.log('⬆️ Parent of current node:', targetObject.parent?.name || 'none', '(ID:', targetObject.parent?.uniqueId || 'none', ')');
              
              // Only stop if we encounter a system object that has no parent (true system root)
              const currentName = targetObject.name || '';
              const isSystemObject = currentName.startsWith('__') || 
                                   currentName.includes('gizmo') || 
                                   currentName.includes('helper');
              
              if (isSystemObject && !targetObject.parent) {
                console.log('🏁 Reached system root object:', currentName, '- stopping walk');
                break;
              }
            }
            
            if (walkCount >= 10) {
              console.log('⚠️ Hierarchy walk exceeded safety limit, stopping');
            }
            
            console.log('✅ Final selection target:', targetObject.name, '(ID:', targetObject.uniqueId, ') class:', targetObject.getClassName());
            
            // Use shared selection - this will update both render and editor stores
            console.log('🔗 Calling renderActions.selectObject with:', targetObject.name, 'ID:', targetObject.uniqueId);
            renderActions.selectObject(targetObject);
          } else {
            // Left click but no hit - deselect
            renderActions.selectObject(null);
          }
        }
      });

      // Make scene globally accessible
      window._cleanBabylonScene = babylonScene;

      // Update store
      renderActions.setEngine(babylonEngine);
      
      // Load default scene content first
      loadDefaultSceneContent(babylonScene, canvasRef);
      
      // Set scene in store (this initializes hierarchy with the content)
      renderActions.setScene(babylonScene);

      setEngine(babylonEngine);
      setScene(babylonScene);

      console.log('✅ Babylon.js initialized successfully');

    } catch (error) {
      console.error('❌ Failed to initialize Babylon.js:', error);
    }
  };

  const cleanup = () => {
    const babylonEngine = engine();
    const babylonScene = scene();
    
    
    // Use render store for cleanup
    renderActions.cleanup();
    
    if (babylonEngine) {
      babylonEngine.stopRenderLoop();
      babylonEngine.dispose();
    }
    
    window._cleanBabylonScene = null;
    renderActions.setEngine(null);
    renderActions.setScene(null);
    renderActions.setCamera(null);
    
    console.log('🗑️ Babylon.js cleaned up');
  };

  return (
    <>
      <canvas
        ref={canvasRef}
        className="w-full h-full outline-none"
        style={{ 'touch-action': 'none' }}
        onContextMenu={props.onContextMenu}
        onDragOver={handleDragOver}
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      />
      <GizmoManagerComponent />
      <LoadingTooltip loadingTooltip={loadingTooltip} />
      
      {/* Time Display */}
      <div 
        id="time-display"
        className="absolute top-4 right-4 bg-black/50 text-white px-3 py-1 rounded-lg font-mono text-sm backdrop-blur-sm"
      >
        12:00
      </div>
    </>
  );
}
