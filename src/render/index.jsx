import { onMount, onCleanup, createSignal, createEffect, onCleanup as solidOnCleanup } from 'solid-js';
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
import { TransformNode } from '@babylonjs/core/Meshes/transformNode';
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
import Stats from 'stats.js';
import { pluginAPI } from '@/api/plugin';
import { viewportStore } from '@/layout/stores/ViewportStore.jsx';



const loadDefaultSceneContent = (scene, canvas) => {
  if (window.DEBUG_RENDER) console.log('🌟 Loading default scene content');
  
  // Create default camera positioned diagonally to show X and Z axis intersection
  const camera = new UniversalCamera(
    'camera',
    new Vector3(7, 5, 7),
    scene
  );
  // Don't attach Babylon's native controls - we use custom camera controller
  camera.setTarget(Vector3.Zero());


  // Unreal Engine style lighting setup
  
  // Configure image processing - values will be updated from lighting settings
  const lightingSettings = renderStore.lighting;
  scene.imageProcessingConfiguration.toneMappingEnabled = lightingSettings.toneMappingEnabled;
  scene.imageProcessingConfiguration.toneMappingType = ImageProcessingConfiguration.TONEMAPPING_ACES;
  scene.imageProcessingConfiguration.exposure = lightingSettings.exposure;
  scene.imageProcessingConfiguration.contrast = lightingSettings.contrast;
  
  // Vignette settings from lighting store
  scene.imageProcessingConfiguration.vignetteEnabled = lightingSettings.vignetteEnabled;
  scene.imageProcessingConfiguration.vignetteWeight = lightingSettings.vignetteWeight;
  scene.imageProcessingConfiguration.vignetteStretch = lightingSettings.vignetteStretch;
  scene.imageProcessingConfiguration.vignetteCameraFov = lightingSettings.vignetteCameraFov;
  
  // Vignette settings are now controlled directly by lighting settings
  
  // Day/Night Cycle System - values from lighting settings
  const dayNightCycle = {
    timeOfDay: lightingSettings.timeOfDay,
    speed: lightingSettings.timeSpeed,
    enabled: lightingSettings.timeEnabled,
    sunriseHour: lightingSettings.sunriseHour,
    sunsetHour: lightingSettings.sunsetHour,
    transitionDuration: lightingSettings.transitionDuration
  };

  // Sky material
  var skyboxMaterial = new SkyMaterial("skyMaterial", scene);
  skyboxMaterial.backFaceCulling = false;
  
  // Add clouds to the sky - values from lighting settings
  skyboxMaterial.cloudsEnabled = lightingSettings.cloudsEnabled;
  skyboxMaterial.cumulusCloudSize = lightingSettings.cloudSize;
  skyboxMaterial.cumulusCloudDensity = lightingSettings.cloudDensity;

  // Sky mesh (sphere)
  var skybox = CreateSphere("skyBox", { diameter: 1000.0 }, scene);
  skybox.material = skyboxMaterial;
  skybox.infiniteDistance = true;
  
  
  // Sky light - will be controlled by day/night cycle
  const skyLight = new HemisphericLight('skyLight', new Vector3(0, 1, 0), scene);
  skyLight.intensity = 1.0; // Initial intensity, will be updated by cycle
  skyLight.diffuse = new Color3(lightingSettings.skyColor[0], lightingSettings.skyColor[1], lightingSettings.skyColor[2]);
  skyLight.groundColor = new Color3(0.6, 0.55, 0.5); // Initial ground color
  skyLight._baseIntensity = lightingSettings.skyIntensity;
  skyLight._baseColor = lightingSettings.skyColor;
  
  // Main directional light - will be controlled by day/night cycle
  const sunLight = new DirectionalLight('sunLight', new Vector3(-0.3, -0.8, -0.5), scene);
  sunLight.intensity = 1.0; // Initial intensity, will be updated by cycle
  sunLight.diffuse = new Color3(lightingSettings.sunColor[0], lightingSettings.sunColor[1], lightingSettings.sunColor[2]);
  sunLight.specular = new Color3(1.0, 0.95, 0.85);
  sunLight._baseIntensity = lightingSettings.sunIntensity;
  sunLight._baseColor = lightingSettings.sunColor;
  
  // Sun position will be calculated dynamically based on time of day
  
  // Rim light - simulates atmospheric scattering
  const rimLight = new DirectionalLight('rimLight', new Vector3(0.8, 0.2, -0.6), scene);
  rimLight.intensity = lightingSettings.rimIntensity;
  rimLight.diffuse = new Color3(lightingSettings.rimColor[0], lightingSettings.rimColor[1], lightingSettings.rimColor[2]);
  rimLight.specular = new Color3(0.0, 0.0, 0.0); // No specular for rim
  rimLight._baseIntensity = lightingSettings.rimIntensity;
  rimLight._baseColor = lightingSettings.rimColor;
  
  // Bounce light - simulates indirect lighting
  const bounceLight = new DirectionalLight('bounceLight', new Vector3(-0.2, 0.3, 0.9), scene);
  bounceLight.intensity = lightingSettings.bounceIntensity;
  bounceLight.diffuse = new Color3(lightingSettings.bounceColor[0], lightingSettings.bounceColor[1], lightingSettings.bounceColor[2]);
  bounceLight.specular = new Color3(0.0, 0.0, 0.0); // No specular for bounce
  bounceLight._baseIntensity = lightingSettings.bounceIntensity;
  bounceLight._baseColor = lightingSettings.bounceColor;

  // Optimized shadow generator with performance-focused settings
  const optimizedShadowSize = Math.min(2048, lightingSettings.shadowMapSize); // Cap at 2048 for performance
  const shadowGenerator = new ShadowGenerator(optimizedShadowSize, sunLight);
  shadowGenerator.usePercentageCloserFiltering = true;
  shadowGenerator.filteringQuality = ShadowGenerator.QUALITY_MEDIUM; // Reduced from HIGH
  shadowGenerator.darkness = lightingSettings.shadowDarkness;
  shadowGenerator.bias = lightingSettings.shadowBias;
  
  // Disable expensive contact hardening for better performance
  shadowGenerator.useContactHardeningShadow = false;
  
  // Reduce cascade count for performance
  shadowGenerator.useCascades = lightingSettings.cascadeShadows;
  shadowGenerator.numCascades = Math.min(2, lightingSettings.shadowCascades); // Max 2 cascades
  shadowGenerator.cascadeBlendPercentage = 0.1;
  
  // Use faster shadow mapping instead of exponential
  shadowGenerator.useExponentialShadowMap = false;
  shadowGenerator.usePoissonSampling = true; // Faster than exponential
  shadowGenerator.blurKernel = Math.min(32, lightingSettings.shadowBlur); // Reduce blur kernel
  
  // Store shadow generator for access by physics objects
  scene.shadowGenerator = shadowGenerator;
  
  // Disable expensive reflection probe for better performance
  // Use static environment texture instead
  scene.environmentIntensity = lightingSettings.environmentIntensity;
  
  // Configure global lighting settings for Unreal-style rendering
  scene.autoClear = true;
  scene.autoClearDepthAndStencil = true;
  
  // Fog disabled by default for performance
  scene.fogEnabled = lightingSettings.fogEnabled;
  if (lightingSettings.fogEnabled) {
    scene.fogMode = 2; // FOGMODE_EXP2
    scene.fogDensity = lightingSettings.fogDensityDay;
    scene.fogColor = new Color3(lightingSettings.fogColorDay[0], lightingSettings.fogColorDay[1], lightingSettings.fogColorDay[2]);
  }
  

  // Create snow particle system - controlled by lighting settings
  const snowSystem = new ParticleSystem('snow', lightingSettings.snowIntensity, scene);
  
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
  snowSystem.emitRate = lightingSettings.snowIntensity;
  
  // Gentle falling motion
  snowSystem.gravity = new Vector3(0, -1.5, 0);
  snowSystem.direction1 = new Vector3(-0.2, -1, -0.2);
  snowSystem.direction2 = new Vector3(0.2, -1, 0.2);
  snowSystem.minAngularSpeed = 0;
  snowSystem.maxAngularSpeed = Math.PI;
  
  // Add slight wind drift
  snowSystem.minEmitPower = 0.5;
  snowSystem.maxEmitPower = 1.0;
  
  // Start snow only if enabled
  if (lightingSettings.snowEnabled) {
    snowSystem.start();
  }

  // Create realistic star field for night sky - controlled by lighting settings
  const starSystem = new ParticleSystem('stars', lightingSettings.starIntensity, scene);
  
  // Create varied star texture with different sizes and brightness
  const starTexture = new DynamicTexture('starTexture', { width: 32, height: 32 }, scene);
  const starContext = starTexture.getContext();
  starContext.clearRect(0, 0, 32, 32);
  
  // Create bright star with soft glow
  const starGradient = starContext.createRadialGradient(16, 16, 0, 16, 16, 12);
  starGradient.addColorStop(0, 'rgba(255, 255, 255, 1)');
  starGradient.addColorStop(0.3, 'rgba(255, 255, 255, 0.8)');
  starGradient.addColorStop(0.7, 'rgba(200, 200, 255, 0.4)');
  starGradient.addColorStop(1, 'rgba(150, 150, 255, 0)');
  starContext.fillStyle = starGradient;
  starContext.beginPath();
  starContext.arc(16, 16, 12, 0, 2 * Math.PI);
  starContext.fill();
  starTexture.update();
  
  starSystem.particleTexture = starTexture;
  starSystem.emitter = new Vector3(0, 0, 0);
  starSystem.minEmitBox = new Vector3(-500, 100, -500); // Wide spread, well above horizon
  starSystem.maxEmitBox = new Vector3(500, 500, 500); // Full hemisphere coverage
  
  // Realistic star colors and brightness variation
  starSystem.color1 = new Color4(1, 1, 1, 1); // Bright white stars
  starSystem.color2 = new Color4(0.8, 0.9, 1, 0.9); // Slightly blue tinted stars
  starSystem.colorDead = new Color4(1, 1, 1, 0);
  
  starSystem.minSize = 0.05; // Smaller minimum size
  starSystem.maxSize = 0.4; // Larger maximum for bright stars
  
  // Make stars emit light with additive blending
  starSystem.blendMode = ParticleSystem.BLENDMODE_ADD;
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
  // Moon position will be calculated dynamically opposite to the sun
  moon.position = new Vector3(100, 300, 200); // Initial position, will be updated
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
  moonLight._baseMoonIntensity = lightingSettings.moonIntensity;


  // Function to update day/night cycle - configurable frame rate
  let frameCounter = 0;
  const updateDayNightCycle = () => {
    // Early exit if disabled to avoid any calculations
    if (!dayNightCycle.enabled) return;
    
    // Get lighting settings for feature checks
    const currentLightingSettings = renderStore.lighting;
    
    // Get update frequency from lighting settings
    const updateFrames = renderStore.lighting.dayNightUpdateFrames || 60;
    frameCounter++;
    if (frameCounter < updateFrames) return;
    frameCounter = 0;
    
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
    
    // Calculate realistic sun position based on time of day
    // Sun rises in east (90°), peaks south at noon (180°), sets in west (270°)
    const timeFromSunrise = currentHour - dayNightCycle.sunriseHour;
    const dayDuration = dayNightCycle.sunsetHour - dayNightCycle.sunriseHour;
    
    // Calculate sun elevation (height above horizon)
    // Sine wave that peaks at solar noon
    const solarNoon = (dayNightCycle.sunriseHour + dayNightCycle.sunsetHour) / 2;
    const timeFromSolarNoon = currentHour - solarNoon;
    const maxElevationAngle = 70 * Math.PI / 180; // Maximum elevation at solar noon (70 degrees)
    
    // Calculate elevation using sine wave centered on solar noon
    const elevationProgress = Math.cos((timeFromSolarNoon / (dayDuration / 2)) * Math.PI / 2);
    const sunElevationAngle = Math.max(0, elevationProgress * maxElevationAngle);
    sunElevation = Math.sin(sunElevationAngle); // Convert to 0-1 for intensity
    
    // Calculate sun azimuth (compass direction)
    // Sun moves from east (90°) to west (270°) during daylight hours
    let sunAzimuthDegrees;
    if (currentHour >= dayNightCycle.sunriseHour && currentHour <= dayNightCycle.sunsetHour) {
      const dayProgress = (currentHour - dayNightCycle.sunriseHour) / dayDuration;
      sunAzimuthDegrees = 90 + (dayProgress * 180); // 90° (east) to 270° (west)
    } else {
      // At night, position sun below horizon on opposite side
      sunAzimuthDegrees = currentHour < 12 ? 270 : 90; // West before midnight, east after
    }
    
    const sunAzimuthRadians = sunAzimuthDegrees * Math.PI / 180;
    
    // Calculate 3D direction vector for the directional light
    const sunDirectionX = Math.cos(sunElevationAngle) * Math.cos(sunAzimuthRadians);
    const sunDirectionY = -Math.sin(sunElevationAngle); // Negative because light points down
    const sunDirectionZ = Math.cos(sunElevationAngle) * Math.sin(sunAzimuthRadians);
    
    // Update sun light direction
    sunLight.direction = new Vector3(sunDirectionX, sunDirectionY, sunDirectionZ);
    
    // Light intensity follows sun elevation naturally - no hard cutoffs
    lightIntensity = sunElevation;
    
    // Calculate sun position for SkyMaterial using same calculations as directional light
    // SkyMaterial inclination: 0 = zenith (high), -0.5 = horizon (low)
    let inclination = -0.5 + (sunElevation * 0.5); // -0.5 (horizon) to 0.0 (zenith)
    
    // Convert our azimuth to SkyMaterial's azimuth system
    // SkyMaterial azimuth: 0 = east, 0.25 = south, 0.5 = west, 0.75 = north
    let azimuth = (sunAzimuthDegrees - 90) / 360; // Convert to 0-1 range
    if (azimuth < 0) azimuth += 1; // Handle negative values
    
    // Update SkyMaterial - all properties gradual
    skyboxMaterial.inclination = lightIntensity > 0 ? inclination : -1.0; // Hide sun below horizon at night
    skyboxMaterial.azimuth = azimuth;
    
    // Use lighting settings from renderStore (reuse existing variable)
    
    // Dynamic turbidity and luminance
    skyboxMaterial.turbidity = currentLightingSettings.dayTurbidity + ((1 - lightIntensity) * (currentLightingSettings.nightTurbidity - currentLightingSettings.dayTurbidity));
    skyboxMaterial.luminance = currentLightingSettings.baseLuminance + (lightIntensity * (currentLightingSettings.dayLuminance - currentLightingSettings.baseLuminance));
    
    
    // Update cloud settings from lighting store
    skyboxMaterial.cloudsEnabled = currentLightingSettings.cloudsEnabled;
    skyboxMaterial.cumulusCloudSize = currentLightingSettings.cloudSize;
    // Hide clouds at night for clearer star visibility unless forced on
    skyboxMaterial.cumulusCloudDensity = currentLightingSettings.cloudsEnabled ? 
      (lightIntensity > 0.2 ? currentLightingSettings.cloudDensity : 0.0) : 0.0;
    
    // Remove sun glare at night by hiding sun disk
    if (lightIntensity > 0.1) {
      skyboxMaterial.sunPosition = new Vector3(0, 1, 0); // Sun visible during day
    } else {
      skyboxMaterial.sunPosition = new Vector3(0, -20, 0); // Hide sun completely at night
    }
    
    // Calculate moon position opposite to the sun
    const moonAzimuthDegrees = (sunAzimuthDegrees + 180) % 360; // Opposite side of sky
    const moonElevationAngle = sunElevationAngle > 0 ? Math.max(0, maxElevationAngle - sunElevationAngle) : maxElevationAngle * 0.5;
    const moonAzimuthRadians = moonAzimuthDegrees * Math.PI / 180;
    
    // Position moon in sky opposite to sun
    const moonDistance = 400; // Distance from center
    const moonX = Math.cos(moonElevationAngle) * Math.cos(moonAzimuthRadians) * moonDistance;
    const moonY = Math.sin(moonElevationAngle) * moonDistance + 100; // Offset upward
    const moonZ = Math.cos(moonElevationAngle) * Math.sin(moonAzimuthRadians) * moonDistance;
    
    moon.position = new Vector3(moonX, moonY, moonZ);
    moonLight.position = moon.position;
    
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
      const baseMoonIntensity = moonLight._baseMoonIntensity || 15.0;
      moonLight.intensity = moonVisibility * baseMoonIntensity;
    }
    
    // Update directional light (sun/moon) - always gradual
    const baseSunIntensity = sunLight._baseIntensity || 4.0;
    sunLight.intensity = lightIntensity * baseSunIntensity;
    
    // Use configurable base colors
    const baseColor = sunLight._baseColor || [1.0, 0.98, 0.9];
    
    if (lightIntensity > 0.1) {
      // Strong sunrise/sunset colors
      const warmth = 1.0 - sunElevation; // 0 at noon, 1 at sunrise/sunset
      if (warmth > 0.7) {
        // Deep sunrise/sunset: intense orange-red
        sunLight.diffuse = new Color3(1.0, 0.4, 0.1); // Bright orange-red
      } else if (warmth > 0.4) {
        // Mid sunrise/sunset: warm yellow-orange
        sunLight.diffuse = new Color3(1.0, 0.7, 0.2); // Golden orange
      } else {
        // Noon: use configurable base color
        sunLight.diffuse = new Color3(baseColor[0], baseColor[1], baseColor[2]);
      }
    } else {
      // Night: use night sky color for moonlight tint
      const nightTint = currentLightingSettings.nightSkyColor;
      sunLight.diffuse = new Color3(nightTint[0] * 2, nightTint[1] * 2, nightTint[2] * 2);
    }
    
    // Update sky light (ambient) - gradual based on light intensity
    const baseSkyIntensity = skyLight._baseIntensity || 4.0;
    skyLight.intensity = lightIntensity * baseSkyIntensity;
    
    // Update sky light color with configurable day/night sky colors  
    const baseSkyColor = skyLight._baseColor || currentLightingSettings.skyColor;
    const nightSkyColor = currentLightingSettings.nightSkyColor;
    const daySkyColor = currentLightingSettings.daySkyColor;
    
    const dayColorMix = Math.max(0, Math.min(1, lightIntensity * 2)); // 0 to 1
    skyLight.diffuse = new Color3(
      nightSkyColor[0] + (dayColorMix * (daySkyColor[0] - nightSkyColor[0])), // Blend night to day
      nightSkyColor[1] + (dayColorMix * (daySkyColor[1] - nightSkyColor[1])),
      nightSkyColor[2] + (dayColorMix * (daySkyColor[2] - nightSkyColor[2]))
    );
    
    skyLight.groundColor = new Color3(
      dayColorMix * 0.4, // 0 to 0.4 (red)
      dayColorMix * 0.3, // 0 to 0.3 (green)
      dayColorMix * 0.2  // 0 to 0.2 (blue)
    );
    
    // Calculate current sky color blend for fog and clear color
    const currentSkyColor = [
      nightSkyColor[0] + (dayColorMix * (daySkyColor[0] - nightSkyColor[0])),
      nightSkyColor[1] + (dayColorMix * (daySkyColor[1] - nightSkyColor[1])),
      nightSkyColor[2] + (dayColorMix * (daySkyColor[2] - nightSkyColor[2]))
    ];
    
    // Update fog color using configurable fog colors
    const fogColorNight = currentLightingSettings.fogColorNight;
    const fogColorDay = currentLightingSettings.fogColorDay;
    
    const currentFogColor = [
      fogColorNight[0] + (dayColorMix * (fogColorDay[0] - fogColorNight[0])),
      fogColorNight[1] + (dayColorMix * (fogColorDay[1] - fogColorNight[1])),
      fogColorNight[2] + (dayColorMix * (fogColorDay[2] - fogColorNight[2]))
    ];
    
    scene.fogColor = new Color3(
      currentFogColor[0],
      currentFogColor[1],
      currentFogColor[2]
    );
    
    // Use configurable fog density
    const fogDensityNight = currentLightingSettings.fogDensityNight;
    const fogDensityDay = currentLightingSettings.fogDensityDay;
    scene.fogDensity = fogDensityNight + (dayColorMix * (fogDensityDay - fogDensityNight));
    
    // Update environment intensity
    const envIntensity = currentLightingSettings.environmentIntensity;
    scene.environmentIntensity = envIntensity * (0.3 + lightIntensity * 0.7); // 30% at night, 100% at day
    
    // Update scene clear color using custom sky colors
    scene.clearColor = new Color4(
      currentSkyColor[0],
      currentSkyColor[1], 
      currentSkyColor[2],
      1
    );
    
    // Update rim and bounce lights - keep on but reduced at night
    const baseRimIntensity = rimLight._baseIntensity || 0.4;
    rimLight.intensity = lightIntensity * baseRimIntensity;
    
    const baseRimColor = rimLight._baseColor || [0.9, 0.7, 0.5];
    const rimColorMix = Math.max(0, Math.min(1, lightIntensity));
    rimLight.diffuse = new Color3(
      0.1 + (rimColorMix * baseRimColor[0]), // Night to day color transition
      0.1 + (rimColorMix * baseRimColor[1]),
      0.2 + (rimColorMix * baseRimColor[2])
    );
    
    // Update bounce light - gradual with day/night cycle
    const baseBounceIntensity = bounceLight._baseIntensity || 0.3;
    bounceLight.intensity = lightIntensity * baseBounceIntensity;
    
    // Update bounce light color with configurable base color and day/night transition
    const baseBounceColor = bounceLight._baseColor || [0.4, 0.5, 0.7];
    const bounceColorMix = Math.max(0, Math.min(1, lightIntensity));
    bounceLight.diffuse = new Color3(
      0.05 + (bounceColorMix * baseBounceColor[0]), // Night to day color transition
      0.05 + (bounceColorMix * baseBounceColor[1]),
      0.1 + (bounceColorMix * baseBounceColor[2])
    );
    
    // Control snow system - only process if enabled
    if (currentLightingSettings.snowEnabled) {
      if (!snowSystem.isStarted()) {
        snowSystem.start();
      }
      snowSystem.emitRate = currentLightingSettings.snowIntensity;
    } else if (snowSystem.isStarted()) {
      snowSystem.stop();
    }
    
    // Control realistic star field based on time and settings - only if enabled
    if (currentLightingSettings.starsEnabled && isAfterSunset && lightIntensity < 0.3) {
      // Start stars at night if not already started
      if (!starSystem.isStarted()) {
        starSystem.manualEmitCount = Math.min(500, currentLightingSettings.starIntensity); // Limit to 500 stars
        starSystem.start();
      }
      
      // Optimized star twinkling - update only every 4th frame to reduce GC pressure
      if (starSystem.particles && scene.getFrameId() % 4 === 0) {
        const currentTime = Date.now() * 0.001;
        const particleCount = Math.min(starSystem.particles.length, 500); // Limit processing
        
        for (let i = 0; i < particleCount; i++) {
          const particle = starSystem.particles[i];
          if (particle?.color) {
            // Pre-calculated values to avoid repeated calculations
            const twinkleSpeed = 0.5 + (i % 3) * 0.3;
            const phase = i * 0.1;
            const twinkle = Math.sin(currentTime * twinkleSpeed + phase) * 0.4 + 0.8;
            const twinkleIntensity = i % 5 === 0 ? 0.6 : 0.3;
            
            particle.color.a = Math.max(0.4, twinkle * twinkleIntensity + (1 - twinkleIntensity));
            
            // Set star colors only once during initialization, not every frame
            if (!particle._colorSet) {
              if (i % 7 === 0) {
                particle.color.r = 0.9;
                particle.color.g = 0.95;
                particle.color.b = 1.0;
              } else if (i % 11 === 0) {
                particle.color.r = 1.0;
                particle.color.g = 0.98;
                particle.color.b = 0.9;
              } else {
                particle.color.r = 1.0;
                particle.color.g = 1.0;
                particle.color.b = 1.0;
              }
              particle._colorSet = true;
            }
          }
        }
      }
    } else if (starSystem.isStarted()) {
      // Hide stars during day or if disabled
      starSystem.stop();
      starSystem.reset();
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
  
  if (window.DEBUG_RENDER) console.log('✅ Default scene content loaded with Unreal-style lighting');
};

export default function BabylonRenderer(props) {
  let canvasRef;
  const [engine, setEngine] = createSignal(null);
  const [scene, setScene] = createSignal(null);
  const [stats, setStats] = createSignal(null);
  
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
      
      // Cancel any existing focus animation
      if (window._focusAnimationId) {
        cancelAnimationFrame(window._focusAnimationId);
        window._focusAnimationId = null;
      }
      
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
          window._focusAnimationId = requestAnimationFrame(animate);
        } else {
          window._focusAnimationId = null;
          console.log('✅ Camera focused on object');
          editorActions.addConsoleMessage(`Focused camera on "${selectedObject.name}"`, 'success');
        }
      };
      
      window._focusAnimationId = requestAnimationFrame(animate);
    },
    // Focus on mouse point
    focusOnMousePoint: () => {
      const camera = renderStore.camera;
      const currentScene = renderStore.scene;
      
      if (!camera || !currentScene) {
        console.log('⚠️ No camera or scene available for focus');
        return;
      }
      
      // Cancel any existing mouse focus animation
      if (window._mouseFocusAnimationId) {
        cancelAnimationFrame(window._mouseFocusAnimationId);
        window._mouseFocusAnimationId = null;
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
            window._mouseFocusAnimationId = requestAnimationFrame(animate);
          } else {
            window._mouseFocusAnimationId = null;
            console.log('✅ Camera focused on mouse point');
            editorActions.addConsoleMessage('Focused camera on mouse point', 'success');
          }
        };
        
        window._mouseFocusAnimationId = requestAnimationFrame(animate);
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
      if (window.DEBUG_RENDER) console.log('🎮 Initializing Babylon.js...');

      // Create engine
      const babylonEngine = new Engine(canvasRef, true, {
        adaptToDeviceRatio: true,
        antialias: true
      });

      // Create scene
      const babylonScene = new Scene(babylonEngine);
      babylonScene.useRightHandedSystem = true;
      // Clear color will be set dynamically by the day/night cycle
      babylonScene.clearColor = new Color4(0.7, 0.8, 1.0, 1); // Start with day color
      // Use lighting settings for exposure
      babylonScene.imageProcessingConfiguration.exposure = renderStore.lighting.exposure;
      
      // Enable FXAA for better anti-aliasing on lines and edges
      babylonScene.imageProcessingConfiguration.fxaaEnabled = renderStore.lighting.fxaaEnabled;

      // Enable Havok physics for RenScript
      try {
        const havokInstance = await HavokPhysics();
        const hk = new HavokPlugin(true, havokInstance);
        const enableResult = babylonScene.enablePhysics(new Vector3(0, -9.81, 0), hk);
        if (window.DEBUG_RENDER) console.log('✅ Havok physics enabled for RenScript, result:', enableResult);
      } catch (error) {
        console.warn('⚠️ Failed to enable Havok physics:', error);
      }

      // Scene content will be loaded separately
      if (window.DEBUG_RENDER) console.log('🎮 Scene created, ready for content loading');

      // Create highlight layer for selection
      const highlightLayer = new HighlightLayer('highlight', babylonScene);
      
      // Store in render store (gizmo manager will be set by GizmoManagerComponent)
      renderActions.setHighlightLayer(highlightLayer);

      // Initialize stats.js with custom object count panel
      const statsInstance = new Stats();
      
      // Add custom panel for scene objects
      const objectPanel = statsInstance.addPanel(new Stats.Panel('OBJ', '#ff8', '#221'));
      
      // Show all panels simultaneously by modifying the DOM structure
      statsInstance.showPanel(0); // Show only FPS panel
      
      statsInstance.dom.style.position = 'absolute';
      statsInstance.dom.style.left = '8px';
      statsInstance.dom.style.bottom = '8px';
      statsInstance.dom.style.top = 'auto';
      statsInstance.dom.style.right = 'auto';
      statsInstance.dom.style.zIndex = '100';
      
      // Let stats.js handle panel display naturally
      
      // Find the canvas container and append stats
      const canvasContainer = canvasRef.parentElement;
      if (canvasContainer) {
        canvasContainer.style.position = 'relative';
        canvasContainer.appendChild(statsInstance.dom);
      }
      
      // Function to count scene objects
      const getObjectCount = () => {
        const meshes = babylonScene.meshes.filter(mesh => 
          !mesh.name.includes('gizmo') && 
          !mesh.name.includes('helper') && 
          !mesh.name.startsWith('__')
        );
        const lights = babylonScene.lights.filter(light => 
          !light.name.includes('gizmo') && 
          !light.name.includes('helper') && 
          !light.name.startsWith('__')
        );
        const cameras = babylonScene.cameras.filter(camera => 
          !camera.name.includes('gizmo') && 
          !camera.name.includes('helper') && 
          !camera.name.startsWith('__')
        );
        return meshes.length + lights.length + cameras.length;
      };
      
      setStats(statsInstance);
      
      // Optimized render loop with frame skipping and pause capability
      let renderLoopRunning = true;
      let renderFrameCounter = 0;
      
      const renderLoop = () => {
        if (!renderLoopRunning) return;
        
        // Check if rendering is paused
        if (editorStore.settings.editor.renderPaused) {
          requestAnimationFrame(renderLoop);
          return;
        }
        
        statsInstance.begin();
        
        // Update object count panel only every 10th frame for performance
        renderFrameCounter++;
        if (renderFrameCounter >= 10) {
          const objectCount = getObjectCount();
          objectPanel.update(objectCount, 1000);
          renderFrameCounter = 0;
        }
        
        babylonScene.render();
        statsInstance.end();
      };
      
      babylonEngine.runRenderLoop(renderLoop);

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

      // Load persisted lighting settings
      renderActions.loadPersistedLightingSettings();
      
      // Update store
      renderActions.setEngine(babylonEngine);
      
      // Load default scene content first
      loadDefaultSceneContent(babylonScene, canvasRef);
      
      // Set scene in store (this initializes hierarchy with the content)
      renderActions.setScene(babylonScene);

      setEngine(babylonEngine);
      setScene(babylonScene);

      if (window.DEBUG_RENDER) console.log('✅ Babylon.js initialized successfully');

    } catch (error) {
      console.error('❌ Failed to initialize Babylon.js:', error);
    }
  };

  const cleanup = () => {
    const babylonEngine = engine();
    const babylonScene = scene();
    const statsInstance = stats();
    
    renderLoopRunning = false;
    
    // Cancel any running animations
    if (window._focusAnimationId) {
      cancelAnimationFrame(window._focusAnimationId);
      window._focusAnimationId = null;
    }
    if (window._mouseFocusAnimationId) {
      cancelAnimationFrame(window._mouseFocusAnimationId);
      window._mouseFocusAnimationId = null;
    }
    
    // Clean up particle systems before disposing scene
    if (babylonScene) {
      const particleSystems = babylonScene.particleSystems?.slice() || [];
      particleSystems.forEach(system => {
        if (system && typeof system.dispose === 'function') {
          system.stop();
          system.dispose();
        }
      });
      
      // Unregister all before render callbacks to prevent memory leaks
      babylonScene.unregisterBeforeRender();
    }
    
    // Use render store for cleanup
    renderActions.cleanup();
    
    if (babylonEngine) {
      babylonEngine.stopRenderLoop();
      babylonEngine.dispose();
    }
    
    // Clean up stats
    if (statsInstance && statsInstance.dom && statsInstance.dom.parentElement) {
      statsInstance.dom.parentElement.removeChild(statsInstance.dom);
    }
    
    window._cleanBabylonScene = null;
    window._dayNightCycle = null;
    renderActions.setEngine(null);
    renderActions.setScene(null);
    renderActions.setCamera(null);
    
    if (window.DEBUG_RENDER) console.log('🗑️ Babylon.js cleaned up');
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
