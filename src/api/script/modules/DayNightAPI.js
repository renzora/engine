import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';
import { Color3, Color4 } from '@babylonjs/core/Maths/math.color.js';

/**
 * DayNightAPI - Day/night cycle system for RenScript
 * Priority: MEDIUM - Advanced lighting feature
 */
export class DayNightAPI {
  constructor(scene, babylonObject) {
    this.scene = scene;
    this.babylonObject = babylonObject;
    this.dayNightCycles = new Map(); // Store multiple cycles by name
  }

  // === DAY/NIGHT CYCLE CREATION ===
  
  createDayNightCycle(name, options = {}) {
    const cycle = {
      name: name,
      timeOfDay: options.timeOfDay || 12.0,
      speed: options.speed || 0.1,
      enabled: options.enabled !== undefined ? options.enabled : true,
      sunriseHour: options.sunriseHour || 6.0,
      sunsetHour: options.sunsetHour || 18.0,
      transitionDuration: options.transitionDuration || 1.0,
      updateFrames: options.updateFrames || 60,
      
      // Lighting configuration
      dayTurbidity: options.dayTurbidity || 2,
      nightTurbidity: options.nightTurbidity || 10,
      baseLuminance: options.baseLuminance || 0.1,
      dayLuminance: options.dayLuminance || 1.0,
      
      // Color settings
      skyColor: options.skyColor || [0.7, 0.8, 1.0],
      nightSkyColor: options.nightSkyColor || [0.05, 0.05, 0.15],
      daySkyColor: options.daySkyColor || [0.4, 0.6, 1.0],
      sunColor: options.sunColor || [1.0, 0.98, 0.9],
      rimColor: options.rimColor || [0.9, 0.7, 0.5],
      bounceColor: options.bounceColor || [0.4, 0.5, 0.7],
      
      // Fog settings
      fogColorDay: options.fogColorDay || [0.7, 0.8, 1.0],
      fogColorNight: options.fogColorNight || [0.1, 0.1, 0.2],
      fogDensityDay: options.fogDensityDay || 0.01,
      fogDensityNight: options.fogDensityNight || 0.005,
      
      // Intensity settings
      skyIntensity: options.skyIntensity || 4.0,
      sunIntensity: options.sunIntensity || 4.0,
      rimIntensity: options.rimIntensity || 0.4,
      bounceIntensity: options.bounceIntensity || 0.3,
      moonIntensity: options.moonIntensity || 15.0,
      environmentIntensity: options.environmentIntensity || 1.0,
      
      // Cloud settings
      cloudsEnabled: options.cloudsEnabled !== undefined ? options.cloudsEnabled : true,
      cloudSize: options.cloudSize || 25,
      cloudDensity: options.cloudDensity || 0.6,
      
      // Feature toggles
      snowEnabled: options.snowEnabled || false,
      snowIntensity: options.snowIntensity || 100,
      starsEnabled: options.starsEnabled !== undefined ? options.starsEnabled : true,
      starIntensity: options.starIntensity || 300,
      
      // Internal state
      frameCounter: 0,
      isRunning: false,
      updateCallback: null
    };
    
    this.dayNightCycles.set(name, cycle);
    return cycle;
  }

  // === CYCLE CONTROL ===
  
  startDayNightCycle(cycleName, lights = {}, skyMaterial = null, moon = null, moonLight = null, snowSystem = null, starSystem = null) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle || !this.scene) return false;
    
    if (cycle.isRunning) {
      this.stopDayNightCycle(cycleName);
    }
    
    // Store references to scene objects
    cycle.lights = lights;
    cycle.skyMaterial = skyMaterial;
    cycle.moon = moon;
    cycle.moonLight = moonLight;
    cycle.snowSystem = snowSystem;
    cycle.starSystem = starSystem;
    
    // Create update function
    cycle.updateCallback = () => {
      this._updateCycle(cycle);
    };
    
    // Register with scene
    this.scene.registerBeforeRender(cycle.updateCallback);
    cycle.isRunning = true;
    
    // Store globally for external access
    if (!window._dayNightCycles) window._dayNightCycles = new Map();
    window._dayNightCycles.set(cycleName, cycle);
    
    return true;
  }

  stopDayNightCycle(cycleName) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle || !this.scene) return false;
    
    if (cycle.updateCallback) {
      this.scene.unregisterBeforeRender(cycle.updateCallback);
      cycle.updateCallback = null;
    }
    
    cycle.isRunning = false;
    
    // Remove from global access
    if (window._dayNightCycles) {
      window._dayNightCycles.delete(cycleName);
    }
    
    return true;
  }

  // === CYCLE PROPERTIES ===
  
  setTimeOfDay(cycleName, hour) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle) return false;
    cycle.timeOfDay = hour;
    return true;
  }

  getTimeOfDay(cycleName) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle) return 12.0;
    return cycle.timeOfDay;
  }

  setTimeSpeed(cycleName, speed) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle) return false;
    cycle.speed = speed;
    return true;
  }

  getTimeSpeed(cycleName) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle) return 0.1;
    return cycle.speed;
  }

  enableCycle(cycleName, enabled = true) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle) return false;
    cycle.enabled = enabled;
    return true;
  }

  isCycleEnabled(cycleName) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle) return false;
    return cycle.enabled;
  }

  // === CYCLE CONFIGURATION ===
  
  setSunTimes(cycleName, sunriseHour, sunsetHour) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle) return false;
    cycle.sunriseHour = sunriseHour;
    cycle.sunsetHour = sunsetHour;
    return true;
  }

  getSunTimes(cycleName) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle) return { sunrise: 6.0, sunset: 18.0 };
    return { sunrise: cycle.sunriseHour, sunset: cycle.sunsetHour };
  }

  setTransitionDuration(cycleName, duration) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle) return false;
    cycle.transitionDuration = duration;
    return true;
  }

  getTransitionDuration(cycleName) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle) return 1.0;
    return cycle.transitionDuration;
  }

  // === CYCLE STATUS ===
  
  getCycleInfo(cycleName) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle) return null;
    
    const currentHour = cycle.timeOfDay;
    const isAfterSunset = currentHour > cycle.sunsetHour || currentHour < cycle.sunriseHour;
    const hours = Math.floor(currentHour);
    const minutes = Math.floor((currentHour - hours) * 60);
    const timeString = `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}`;
    
    return {
      name: cycle.name,
      timeOfDay: cycle.timeOfDay,
      timeString: timeString,
      speed: cycle.speed,
      enabled: cycle.enabled,
      isRunning: cycle.isRunning,
      isDayTime: !isAfterSunset,
      isNightTime: isAfterSunset,
      sunriseHour: cycle.sunriseHour,
      sunsetHour: cycle.sunsetHour,
      transitionDuration: cycle.transitionDuration
    };
  }

  getAllCycles() {
    return Array.from(this.dayNightCycles.keys());
  }

  // === INTERNAL UPDATE FUNCTION ===
  
  _updateCycle(cycle) {
    if (!cycle.enabled || !this.scene) return;
    
    // Frame-based updates to reduce computation
    cycle.frameCounter++;
    if (cycle.frameCounter < cycle.updateFrames) return;
    cycle.frameCounter = 0;
    
    // Advance time
    cycle.timeOfDay += cycle.speed * (1/60);
    if (cycle.timeOfDay >= 24) cycle.timeOfDay = 0;
    
    const currentHour = cycle.timeOfDay;
    const isAfterSunset = currentHour > cycle.sunsetHour || currentHour < cycle.sunriseHour;
    
    // Calculate periods
    const dawnStart = cycle.sunriseHour - cycle.transitionDuration;
    const dayStart = cycle.sunriseHour;
    const duskStart = cycle.sunsetHour;
    const nightStart = cycle.sunsetHour + cycle.transitionDuration;
    
    // Calculate sun elevation and position
    const timeFromSunrise = currentHour - cycle.sunriseHour;
    const dayDuration = cycle.sunsetHour - cycle.sunriseHour;
    const solarNoon = (cycle.sunriseHour + cycle.sunsetHour) / 2;
    const timeFromSolarNoon = currentHour - solarNoon;
    const maxElevationAngle = 70 * Math.PI / 180;
    
    const elevationProgress = Math.cos((timeFromSolarNoon / (dayDuration / 2)) * Math.PI / 2);
    const sunElevationAngle = Math.max(0, elevationProgress * maxElevationAngle);
    const sunElevation = Math.sin(sunElevationAngle);
    const lightIntensity = sunElevation;
    
    // Calculate sun azimuth
    let sunAzimuthDegrees;
    if (currentHour >= cycle.sunriseHour && currentHour <= cycle.sunsetHour) {
      const dayProgress = (currentHour - cycle.sunriseHour) / dayDuration;
      sunAzimuthDegrees = 90 + (dayProgress * 180);
    } else {
      sunAzimuthDegrees = currentHour < 12 ? 270 : 90;
    }
    
    const sunAzimuthRadians = sunAzimuthDegrees * Math.PI / 180;
    
    // Calculate 3D direction vector
    const sunDirectionX = Math.cos(sunElevationAngle) * Math.cos(sunAzimuthRadians);
    const sunDirectionY = -Math.sin(sunElevationAngle);
    const sunDirectionZ = Math.cos(sunElevationAngle) * Math.sin(sunAzimuthRadians);
    
    // Update lights
    if (cycle.lights?.sunLight) {
      cycle.lights.sunLight.direction = new Vector3(sunDirectionX, sunDirectionY, sunDirectionZ);
      cycle.lights.sunLight.intensity = lightIntensity * cycle.sunIntensity;
      
      // Update sun color based on elevation
      if (lightIntensity > 0.1) {
        const warmth = 1.0 - sunElevation;
        if (warmth > 0.7) {
          cycle.lights.sunLight.diffuse = new Color3(1.0, 0.4, 0.1);
        } else if (warmth > 0.4) {
          cycle.lights.sunLight.diffuse = new Color3(1.0, 0.7, 0.2);
        } else {
          cycle.lights.sunLight.diffuse = new Color3(...cycle.sunColor);
        }
      } else {
        cycle.lights.sunLight.diffuse = new Color3(
          cycle.nightSkyColor[0] * 2,
          cycle.nightSkyColor[1] * 2,
          cycle.nightSkyColor[2] * 2
        );
      }
    }
    
    // Update sky light
    if (cycle.lights?.skyLight) {
      cycle.lights.skyLight.intensity = lightIntensity * cycle.skyIntensity;
      
      const dayColorMix = Math.max(0, Math.min(1, lightIntensity * 2));
      cycle.lights.skyLight.diffuse = new Color3(
        cycle.nightSkyColor[0] + (dayColorMix * (cycle.daySkyColor[0] - cycle.nightSkyColor[0])),
        cycle.nightSkyColor[1] + (dayColorMix * (cycle.daySkyColor[1] - cycle.nightSkyColor[1])),
        cycle.nightSkyColor[2] + (dayColorMix * (cycle.daySkyColor[2] - cycle.nightSkyColor[2]))
      );
      
      cycle.lights.skyLight.groundColor = new Color3(
        dayColorMix * 0.4,
        dayColorMix * 0.3,
        dayColorMix * 0.2
      );
    }
    
    // Update rim light
    if (cycle.lights?.rimLight) {
      cycle.lights.rimLight.intensity = lightIntensity * cycle.rimIntensity;
      
      const rimColorMix = Math.max(0, Math.min(1, lightIntensity));
      cycle.lights.rimLight.diffuse = new Color3(
        0.1 + (rimColorMix * cycle.rimColor[0]),
        0.1 + (rimColorMix * cycle.rimColor[1]),
        0.2 + (rimColorMix * cycle.rimColor[2])
      );
    }
    
    // Update bounce light
    if (cycle.lights?.bounceLight) {
      cycle.lights.bounceLight.intensity = lightIntensity * cycle.bounceIntensity;
      
      const bounceColorMix = Math.max(0, Math.min(1, lightIntensity));
      cycle.lights.bounceLight.diffuse = new Color3(
        0.05 + (bounceColorMix * cycle.bounceColor[0]),
        0.05 + (bounceColorMix * cycle.bounceColor[1]),
        0.1 + (bounceColorMix * cycle.bounceColor[2])
      );
    }
    
    // Update sky material
    if (cycle.skyMaterial) {
      const inclination = -0.5 + (sunElevation * 0.5);
      let azimuth = (sunAzimuthDegrees - 90) / 360;
      if (azimuth < 0) azimuth += 1;
      
      cycle.skyMaterial.inclination = lightIntensity > 0 ? inclination : -1.0;
      cycle.skyMaterial.azimuth = azimuth;
      cycle.skyMaterial.turbidity = cycle.dayTurbidity + ((1 - lightIntensity) * (cycle.nightTurbidity - cycle.dayTurbidity));
      cycle.skyMaterial.luminance = cycle.baseLuminance + (lightIntensity * (cycle.dayLuminance - cycle.baseLuminance));
      
      // Update clouds
      cycle.skyMaterial.cloudsEnabled = cycle.cloudsEnabled;
      cycle.skyMaterial.cumulusCloudSize = cycle.cloudSize;
      cycle.skyMaterial.cumulusCloudDensity = cycle.cloudsEnabled ? 
        (lightIntensity > 0.2 ? cycle.cloudDensity : 0.0) : 0.0;
      
      // Update sun position for sky material
      if (lightIntensity > 0.1) {
        cycle.skyMaterial.sunPosition = new Vector3(0, 1, 0);
      } else {
        cycle.skyMaterial.sunPosition = new Vector3(0, -20, 0);
      }
    }
    
    // Update moon
    if (cycle.moon && cycle.moonLight) {
      // Calculate moon position opposite to sun
      const moonAzimuthDegrees = (sunAzimuthDegrees + 180) % 360;
      const moonElevationAngle = sunElevationAngle > 0 ? Math.max(0, maxElevationAngle - sunElevationAngle) : maxElevationAngle * 0.5;
      const moonAzimuthRadians = moonAzimuthDegrees * Math.PI / 180;
      
      const moonDistance = 400;
      const moonX = Math.cos(moonElevationAngle) * Math.cos(moonAzimuthRadians) * moonDistance;
      const moonY = Math.sin(moonElevationAngle) * moonDistance + 100;
      const moonZ = Math.cos(moonElevationAngle) * Math.sin(moonAzimuthRadians) * moonDistance;
      
      cycle.moon.position = new Vector3(moonX, moonY, moonZ);
      cycle.moonLight.position = cycle.moon.position;
      
      // Update moon visibility
      const moonVisibility = isAfterSunset ? Math.max(0.5, 1 - lightIntensity) : 0.0;
      if (cycle.moon.material) {
        cycle.moon.material.alpha = moonVisibility;
        cycle.moon.material.emissiveColor = new Color3(
          moonVisibility * 0.4,
          moonVisibility * 0.4,
          moonVisibility * 0.35
        );
      }
      
      cycle.moonLight.intensity = moonVisibility * cycle.moonIntensity;
    }
    
    // Update scene properties
    const dayColorMix = Math.max(0, Math.min(1, lightIntensity * 2));
    const currentSkyColor = [
      cycle.nightSkyColor[0] + (dayColorMix * (cycle.daySkyColor[0] - cycle.nightSkyColor[0])),
      cycle.nightSkyColor[1] + (dayColorMix * (cycle.daySkyColor[1] - cycle.nightSkyColor[1])),
      cycle.nightSkyColor[2] + (dayColorMix * (cycle.daySkyColor[2] - cycle.nightSkyColor[2]))
    ];
    
    // Update scene clear color
    this.scene.clearColor = new Color4(
      currentSkyColor[0],
      currentSkyColor[1],
      currentSkyColor[2],
      1
    );
    
    // Update fog
    if (this.scene.fogEnabled) {
      const currentFogColor = [
        cycle.fogColorNight[0] + (dayColorMix * (cycle.fogColorDay[0] - cycle.fogColorNight[0])),
        cycle.fogColorNight[1] + (dayColorMix * (cycle.fogColorDay[1] - cycle.fogColorNight[1])),
        cycle.fogColorNight[2] + (dayColorMix * (cycle.fogColorDay[2] - cycle.fogColorNight[2]))
      ];
      
      this.scene.fogColor = new Color3(...currentFogColor);
      this.scene.fogDensity = cycle.fogDensityNight + (dayColorMix * (cycle.fogDensityDay - cycle.fogDensityNight));
    }
    
    // Update environment intensity
    this.scene.environmentIntensity = cycle.environmentIntensity * (0.3 + lightIntensity * 0.7);
    
    // Control particle systems
    if (cycle.snowSystem && cycle.snowEnabled) {
      if (!cycle.snowSystem.isStarted()) {
        cycle.snowSystem.start();
      }
      cycle.snowSystem.emitRate = cycle.snowIntensity;
    } else if (cycle.snowSystem && cycle.snowSystem.isStarted()) {
      cycle.snowSystem.stop();
    }
    
    // Control stars
    if (cycle.starSystem && cycle.starsEnabled && isAfterSunset && lightIntensity < 0.3) {
      if (!cycle.starSystem.isStarted()) {
        cycle.starSystem.manualEmitCount = Math.min(500, cycle.starIntensity);
        cycle.starSystem.start();
      }
      
      // Update star twinkling
      if (cycle.starSystem.particles && this.scene.getFrameId() % 4 === 0) {
        const currentTime = Date.now() * 0.001;
        const particleCount = Math.min(cycle.starSystem.particles.length, 500);
        
        for (let i = 0; i < particleCount; i++) {
          const particle = cycle.starSystem.particles[i];
          if (particle?.color) {
            const twinkleSpeed = 0.5 + (i % 3) * 0.3;
            const phase = i * 0.1;
            const twinkle = Math.sin(currentTime * twinkleSpeed + phase) * 0.4 + 0.8;
            const twinkleIntensity = i % 5 === 0 ? 0.6 : 0.3;
            
            particle.color.a = Math.max(0.4, twinkle * twinkleIntensity + (1 - twinkleIntensity));
            
            // Set star colors once
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
    } else if (cycle.starSystem && cycle.starSystem.isStarted()) {
      cycle.starSystem.stop();
      cycle.starSystem.reset();
    }
    
    // Update time display if exists
    const hours = Math.floor(currentHour);
    const minutes = Math.floor((currentHour - hours) * 60);
    const timeString = `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}`;
    
    const timeDisplay = document.getElementById('time-display');
    if (timeDisplay) {
      timeDisplay.textContent = timeString;
    }
  }

  // === CYCLE MANAGEMENT ===
  
  disposeCycle(cycleName) {
    const cycle = this.dayNightCycles.get(cycleName);
    if (!cycle) return false;
    
    this.stopDayNightCycle(cycleName);
    this.dayNightCycles.delete(cycleName);
    
    return true;
  }

  disposeAllCycles() {
    for (const cycleName of this.dayNightCycles.keys()) {
      this.disposeCycle(cycleName);
    }
    return true;
  }

  // === SHORT NAME ALIASES ===
  
  cycle(name, options = {}) {
    return this.createDayNightCycle(name, options);
  }
  
  startCycle(cycleName, lights = {}, skyMaterial = null, moon = null, moonLight = null, snowSystem = null, starSystem = null) {
    return this.startDayNightCycle(cycleName, lights, skyMaterial, moon, moonLight, snowSystem, starSystem);
  }
  
  stopCycle(cycleName) {
    return this.stopDayNightCycle(cycleName);
  }
  
  cycleInfo(cycleName) {
    return this.getCycleInfo(cycleName);
  }
  
  allCycles() {
    return this.getAllCycles();
  }
}