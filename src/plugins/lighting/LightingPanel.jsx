import { createEffect, onCleanup, createMemo, createSignal, Show } from 'solid-js';
import { renderStore, renderActions } from '@/render/store';
import { IconSun, IconBulb, IconMoon, IconPalette, IconCamera, IconSettings, IconEye, IconCloud, IconClock } from '@tabler/icons-solidjs';
import { ImageProcessingConfiguration } from '@babylonjs/core/Materials/imageProcessingConfiguration';

export default function LightingPanel() {
  const lighting = () => renderStore.lighting;
  
  // Default lighting values for reset functionality
  const defaults = {
    sunIntensity: 4.0,
    skyIntensity: 4.0,
    rimIntensity: 0.4,
    bounceIntensity: 0.3,
    moonIntensity: 15.0,
    sunColor: [1.0, 0.98, 0.9],
    skyColor: [0.8, 0.9, 1.0],
    rimColor: [0.9, 0.7, 0.5],
    bounceColor: [0.4, 0.5, 0.7],
    timeOfDay: 12.0,
    timeSpeed: 1.0,
    timeEnabled: false,
    sunriseHour: 6.0,
    sunsetHour: 18.0,
    nightTurbidity: 48,
    dayTurbidity: 2,
    nightSkyColor: [0.02, 0.02, 0.05],
    daySkyColor: [0.7, 0.8, 1.0],
    baseLuminance: 0.3,
    dayLuminance: 1.0,
    environmentIntensity: 1.2,
    fogEnabled: true,
    fogDensityDay: 0.001,
    fogDensityNight: 0.0001,
    fogColorDay: [0.7, 0.8, 0.9],
    fogColorNight: [0.05, 0.05, 0.1],
    exposure: 0.85,
    contrast: 1.1,
    brightness: 0.0,
    saturation: 1.0,
    vignetteEnabled: false,
    vignetteWeight: 3.0,
    vignetteStretch: 0.2,
    toneMappingEnabled: true,
    toneMappingType: 'ACES',
    fxaaEnabled: true,
    shadowMapSize: 4096,
    shadowDarkness: 0.3,
    shadowBlur: 64,
    cascadeShadows: true,
    contactHardeningShadows: false,
    cloudsEnabled: false,
    cloudSize: 50,
    cloudDensity: 0.8,
    snowEnabled: false,
    snowIntensity: 500,
    starsEnabled: false,
    starIntensity: 5000,
    dayNightUpdateFrames: 60
  };
  
  // Section collapse state
  const [sectionsOpen, setSectionsOpen] = createSignal({
    time: true,
    postProcessing: false,
    sky: false,
    clouds: false,
    fog: false,
    lights: false,
    shadows: false,
    particles: false
  });
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };
  
  // Generic setter function to reduce boilerplate
  const setSetting = (key, value) => renderActions.setLightingSetting(key, value);
  
  // Time control functions
  const setTimeOfDay = (value) => {
    setSetting('timeOfDay', parseFloat(value));
    // Update global day/night cycle immediately
    if (window._dayNightCycle) {
      window._dayNightCycle.timeOfDay = parseFloat(value);
    }
  };
  
  const setTimeSpeed = (value) => {
    setSetting('timeSpeed', parseFloat(value));
    if (window._dayNightCycle) {
      window._dayNightCycle.speed = parseFloat(value);
    }
  };
  
  const toggleTime = () => {
    const newEnabled = !lighting().timeEnabled;
    setSetting('timeEnabled', newEnabled);
    if (window._dayNightCycle) {
      window._dayNightCycle.enabled = newEnabled;
    }
  };
  
  const toggleSetting = (key) => {
    setSetting(key, !lighting()[key]);
  };
  
  // Apply lighting changes to the scene in real-time
  createEffect(() => {
    const scene = window._cleanBabylonScene;
    if (!scene) return;
    
    const l = lighting();
    
    // Update lights with new base values
    const sunLight = scene.getLightByName('sunLight');
    if (sunLight) {
      sunLight._baseIntensity = l.sunIntensity;
      sunLight._baseColor = l.sunColor;
    }
    
    const skyLight = scene.getLightByName('skyLight');
    if (skyLight) {
      skyLight._baseIntensity = l.skyIntensity;
      skyLight._baseColor = l.skyColor;
    }
    
    const rimLight = scene.getLightByName('rimLight');
    if (rimLight) {
      rimLight._baseIntensity = l.rimIntensity;
      rimLight._baseColor = l.rimColor;
    }
    
    const bounceLight = scene.getLightByName('bounceLight');
    if (bounceLight) {
      bounceLight._baseIntensity = l.bounceIntensity;
      bounceLight._baseColor = l.bounceColor;
    }
    
    const moonLight = scene.getLightByName('moonLight');
    if (moonLight) {
      moonLight._baseMoonIntensity = l.moonIntensity;
    }
    
    // Update post-processing
    if (scene.imageProcessingConfiguration) {
      scene.imageProcessingConfiguration.exposure = l.exposure;
      scene.imageProcessingConfiguration.contrast = l.contrast;
      scene.imageProcessingConfiguration.vignetteEnabled = l.vignetteEnabled;
      scene.imageProcessingConfiguration.vignetteWeight = l.vignetteWeight;
      scene.imageProcessingConfiguration.vignetteStretch = l.vignetteStretch;
      scene.imageProcessingConfiguration.vignetteCameraFov = l.vignetteCameraFov;
      scene.imageProcessingConfiguration.toneMappingEnabled = l.toneMappingEnabled;
      scene.imageProcessingConfiguration.fxaaEnabled = l.fxaaEnabled;
      
      // Update tone mapping type
      switch (l.toneMappingType) {
        case 'ACES':
          scene.imageProcessingConfiguration.toneMappingType = ImageProcessingConfiguration.TONEMAPPING_ACES;
          break;
        case 'Standard':
          scene.imageProcessingConfiguration.toneMappingType = ImageProcessingConfiguration.TONEMAPPING_STANDARD;
          break;
        case 'Photographic':
          scene.imageProcessingConfiguration.toneMappingType = ImageProcessingConfiguration.TONEMAPPING_PHOTOGRAPHIC;
          break;
      }
    }
    
    // Update fog
    scene.fogEnabled = l.fogEnabled;
    
    // Update shadow settings
    if (scene.shadowGenerator) {
      scene.shadowGenerator.mapSize = l.shadowMapSize;
      scene.shadowGenerator.darkness = l.shadowDarkness;
      scene.shadowGenerator.bias = l.shadowBias;
      scene.shadowGenerator.blurKernel = l.shadowBlur;
      scene.shadowGenerator.useCascades = l.cascadeShadows;
      scene.shadowGenerator.numCascades = l.shadowCascades;
      scene.shadowGenerator.useContactHardeningShadow = l.contactHardeningShadows;
    }
    
    // Update time settings
    if (window._dayNightCycle) {
      window._dayNightCycle.timeOfDay = l.timeOfDay;
      window._dayNightCycle.speed = l.timeSpeed;
      window._dayNightCycle.enabled = l.timeEnabled;
      window._dayNightCycle.sunriseHour = l.sunriseHour;
      window._dayNightCycle.sunsetHour = l.sunsetHour;
      window._dayNightCycle.transitionDuration = l.transitionDuration;
    }
  });

  // Convert RGB array to hex color for color input
  const rgbToHex = (rgb) => {
    const r = Math.round(rgb[0] * 255).toString(16).padStart(2, '0');
    const g = Math.round(rgb[1] * 255).toString(16).padStart(2, '0');
    const b = Math.round(rgb[2] * 255).toString(16).padStart(2, '0');
    return `#${r}${g}${b}`;
  };

  // Convert hex color to RGB array
  const hexToRgb = (hex) => {
    const r = parseInt(hex.slice(1, 3), 16) / 255;
    const g = parseInt(hex.slice(3, 5), 16) / 255;
    const b = parseInt(hex.slice(5, 7), 16) / 255;
    return [r, g, b];
  };

  // Memoized formatted values that update reactively
  const currentTime = createMemo(() => {
    const hours = Math.floor(lighting().timeOfDay);
    const minutes = Math.floor((lighting().timeOfDay - hours) * 60);
    return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}`;
  });

  // Reactive formatter for different value types
  const formatValue = createMemo(() => (value, step) => {
    if (typeof value !== 'number') return value;
    if (step < 0.01) return value.toFixed(4);
    if (step < 0.1) return value.toFixed(2);
    if (step < 1) return value.toFixed(1);
    return value.toFixed(0);
  });

  // Reactive text displays for key lighting values
  const exposureText = createMemo(() => `${lighting().exposure.toFixed(2)}`);
  const contrastText = createMemo(() => `${lighting().contrast.toFixed(2)}`);
  const timeSpeedText = createMemo(() => `${lighting().timeSpeed.toFixed(1)}x`);
  const timeOfDayText = createMemo(() => `${lighting().timeOfDay.toFixed(1)}h`);
  const environmentText = createMemo(() => `${lighting().environmentIntensity.toFixed(1)}`);
  const sunIntensityText = createMemo(() => `${lighting().sunIntensity.toFixed(1)}`);
  const moonIntensityText = createMemo(() => `${lighting().moonIntensity.toFixed(1)}`);
  
  // Time status indicator
  const timeStatus = createMemo(() => lighting().timeEnabled ? 'ON' : 'OFF');
  const timeStatusClass = createMemo(() => 
    lighting().timeEnabled ? 'btn-success' : 'btn-error'
  );

  const SliderControl = ({ label, getValue, min, max, step, onChange, unit = '', resetKey }) => {
    const displayValue = createMemo(() => {
      const value = getValue();
      if (typeof value !== 'number') return value;
      if (step < 0.01) return value.toFixed(4);
      if (step < 0.1) return value.toFixed(2);
      if (step < 1) return value.toFixed(1);
      return value.toFixed(0);
    });
    
    const handleReset = () => {
      if (resetKey && defaults[resetKey] !== undefined) {
        onChange(defaults[resetKey]);
      }
    };
    
    return (
      <div>
        <div class="flex items-center justify-between">
          <label class="text-xs text-base-content/80 leading-none">
            {label}
          </label>
          <div class="flex items-center gap-1">
            <span class="text-xs font-mono text-base-content/60 min-w-[3rem] text-right">
              {displayValue()}{unit}
            </span>
            {resetKey && (
              <button
                onClick={handleReset}
                class="btn btn-xs btn-ghost opacity-60 hover:opacity-100 min-h-0 h-3 w-3 p-0 text-xs"
                title={`Reset ${label}`}
              >
                ↺
              </button>
            )}
          </div>
        </div>
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={getValue()}
          onInput={(e) => onChange(parseFloat(e.target.value))}
          class="range range-primary range-xs w-full mt-0.5"
        />
      </div>
    );
  };

  const ColorControl = ({ label, value, onChange, resetKey }) => {
    const handleReset = () => {
      if (resetKey && defaults[resetKey] !== undefined) {
        onChange(defaults[resetKey]);
      }
    };
    
    return (
      <div>
        <div class="flex items-center justify-between mb-0.5">
          <label class="text-xs text-base-content/80">{label}</label>
          {resetKey && (
            <button
              onClick={handleReset}
              class="btn btn-xs btn-ghost opacity-60 hover:opacity-100 min-h-0 h-6 w-6 p-0"
              title={`Reset ${label}`}
            >
              ↺
            </button>
          )}
        </div>
        <input
          type="color"
          value={rgbToHex(value)}
          onInput={(e) => onChange(hexToRgb(e.target.value))}
          class="w-full h-8 rounded border border-base-300"
        />
      </div>
    );
  };

  const ToggleControl = ({ label, value, onChange, icon, resetKey }) => {
    const handleReset = () => {
      if (resetKey && defaults[resetKey] !== undefined) {
        onChange(defaults[resetKey]);
      }
    };
    
    return (
      <div class="flex items-center justify-between">
        <label class="text-xs text-base-content/80 flex items-center gap-1 leading-none">
          {icon && <icon class="w-3 h-3" />}
          {label}
        </label>
        <div class="flex items-center gap-1">
          {resetKey && (
            <button
              onClick={handleReset}
              class="btn btn-xs btn-ghost opacity-60 hover:opacity-100 min-h-0 h-3 w-3 p-0 text-xs"
              title={`Reset ${label}`}
            >
              ↺
            </button>
          )}
          <input
            type="checkbox"
            checked={value}
            onChange={(e) => onChange(e.target.checked)}
            class="toggle toggle-primary toggle-xs"
          />
        </div>
      </div>
    );
  };

  return (
    <div class="h-full flex flex-col">
      {/* Content */}
      <div class="flex-1 p-2 space-y-2">
        
        {/* Time Control */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50">
            <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('time')}>
              <IconClock class="w-3 h-3" />
              Time Control
            </div>
            <input
              type="checkbox"
              checked={lighting().timeEnabled}
              onChange={(e) => {
                e.stopPropagation();
                toggleTime();
              }}
              onClick={(e) => e.stopPropagation()}
              class="toggle toggle-primary toggle-xs"
            />
          </div>
          <Show when={sectionsOpen().time}>
            <div class="!p-2">
              <div class="space-y-0.5">
              
              <div class="bg-base-100/50 rounded-lg p-1 text-center">
                <div class="text-xs font-mono">{currentTime()}</div>
                <div class="text-xs text-base-content/60">Current Time</div>
              </div>
              
              <div class="space-y-1">
                <SliderControl 
                  label="Time of Day" 
                  getValue={() => lighting().timeOfDay} 
                  min={0} 
                  max={24} 
                  step={0.1} 
                  onChange={setTimeOfDay}
                  unit="h"
                  resetKey="timeOfDay"
                />
                <SliderControl 
                  label="Time Speed" 
                  getValue={() => lighting().timeSpeed} 
                  min={0} 
                  max={5} 
                  step={0.1} 
                  onChange={setTimeSpeed}
                  unit="x" 
                  resetKey="timeSpeed"
                />
                <SliderControl 
                  label="Update Every" 
                  getValue={() => lighting().dayNightUpdateFrames || 60} 
                  min={1} 
                  max={60} 
                  step={1} 
                  onChange={(v) => setSetting('dayNightUpdateFrames', v)}
                  unit=" frames" 
                  resetKey="dayNightUpdateFrames"
                />
                <SliderControl 
                  label="Sunrise Hour" 
                  getValue={() => lighting().sunriseHour} 
                  min={0} 
                  max={12} 
                  step={0.5} 
                  onChange={(v) => setSetting('sunriseHour', v)}
                  unit="h" 
                  resetKey="sunriseHour"
                />
                <SliderControl 
                  label="Sunset Hour" 
                  getValue={() => lighting().sunsetHour} 
                  min={12} 
                  max={24} 
                  step={0.5} 
                  onChange={(v) => setSetting('sunsetHour', v)}
                  unit="h" 
                  resetKey="sunsetHour"
                />
              </div>
              </div>
            </div>
          </Show>
        </div>
        
        {/* Post Processing */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer" onClick={() => toggleSection('postProcessing')}>
            <IconCamera class="w-3 h-3" />
            Post Processing
          </div>
          <Show when={sectionsOpen().postProcessing}>
            <div class="!p-2">
            <div class="space-y-0.5">
            <ToggleControl 
              label="Tone Mapping" 
              value={lighting().toneMappingEnabled} 
              onChange={(v) => setSetting('toneMappingEnabled', v)}
              resetKey="toneMappingEnabled"
            />
            
            {lighting().toneMappingEnabled && (
              <div>
                <label class="block text-xs text-base-content/80 mb-0.5">Tone Mapping Type</label>
                <select 
                  value={lighting().toneMappingType}
                  onChange={(e) => setSetting('toneMappingType', e.target.value)}
                  class="select select-sm w-full border border-base-300"
                >
                  <option value="ACES">ACES</option>
                  <option value="Standard">Standard</option>
                  <option value="Photographic">Photographic</option>
                </select>
              </div>
            )}
            <ToggleControl 
              label="Anti-Aliasing (FXAA)" 
              value={lighting().fxaaEnabled} 
              onChange={(v) => setSetting('fxaaEnabled', v)}
              resetKey="fxaaEnabled"
            />
            <ToggleControl 
              label="Vignette" 
              value={lighting().vignetteEnabled} 
              onChange={(v) => setSetting('vignetteEnabled', v)}
              resetKey="vignetteEnabled"
            />
            
            <SliderControl 
              label="Exposure" 
              getValue={() => lighting().exposure} 
              min={0.1} 
              max={5.0} 
              step={0.05} 
              onChange={(v) => setSetting('exposure', v)} 
              resetKey="exposure"
            />
            <SliderControl 
              label="Contrast" 
              getValue={() => lighting().contrast} 
              min={0.1} 
              max={3.0} 
              step={0.05} 
              onChange={(v) => setSetting('contrast', v)} 
              resetKey="contrast"
            />
            <SliderControl 
              label="Brightness" 
              getValue={() => lighting().brightness} 
              min={-1.0} 
              max={1.0} 
              step={0.05} 
              onChange={(v) => setSetting('brightness', v)} 
              resetKey="brightness"
            />
            <SliderControl 
              label="Saturation" 
              getValue={() => lighting().saturation} 
              min={0.0} 
              max={3.0} 
              step={0.05} 
              onChange={(v) => setSetting('saturation', v)} 
              resetKey="saturation"
            />
            
            {lighting().vignetteEnabled && (
              <>
                <SliderControl 
                  label="Vignette Strength" 
                  getValue={() => lighting().vignetteWeight} 
                  min={0} 
                  max={10} 
                  step={0.1} 
                  onChange={(v) => setSetting('vignetteWeight', v)} 
                  resetKey="vignetteWeight"
                />
                <SliderControl 
                  label="Vignette Stretch" 
                  getValue={() => lighting().vignetteStretch} 
                  min={0} 
                  max={2} 
                  step={0.05} 
                  onChange={(v) => setSetting('vignetteStretch', v)} 
                  resetKey="vignetteStretch"
                />
              </>
            )}
            </div>
            </div>
          </Show>
        </div>
        
        {/* Sky & Atmosphere */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer" onClick={() => toggleSection('sky')}>
            <IconPalette class="w-3 h-3" />
            Sky & Atmosphere
          </div>
          <Show when={sectionsOpen().sky}>
            <div class="!p-2">
            <div class="space-y-0.5">
            <ColorControl 
              label="Night Sky Color" 
              value={lighting().nightSkyColor} 
              onChange={(v) => setSetting('nightSkyColor', v)} 
              resetKey="nightSkyColor"
            />
            <ColorControl 
              label="Day Sky Color" 
              value={lighting().daySkyColor} 
              onChange={(v) => setSetting('daySkyColor', v)} 
              resetKey="daySkyColor"
            />
            
            <SliderControl 
              label="Night Turbidity" 
              getValue={() => lighting().nightTurbidity} 
              min={1} 
              max={200} 
              step={1} 
              onChange={(v) => setSetting('nightTurbidity', v)} 
              resetKey="nightTurbidity"
            />
            <SliderControl 
              label="Day Turbidity" 
              getValue={() => lighting().dayTurbidity} 
              min={0.1} 
              max={50} 
              step={0.1} 
              onChange={(v) => setSetting('dayTurbidity', v)} 
              resetKey="dayTurbidity"
            />
            <SliderControl 
              label="Night Luminance" 
              getValue={() => lighting().baseLuminance} 
              min={0} 
              max={5.0} 
              step={0.01} 
              onChange={(v) => setSetting('baseLuminance', v)} 
              resetKey="baseLuminance"
            />
            <SliderControl 
              label="Day Luminance" 
              getValue={() => lighting().dayLuminance} 
              min={0} 
              max={10.0} 
              step={0.01} 
              onChange={(v) => setSetting('dayLuminance', v)} 
              resetKey="dayLuminance"
            />
            <SliderControl 
              label="Environment Intensity" 
              getValue={() => lighting().environmentIntensity} 
              min={0} 
              max={10.0} 
              step={0.1} 
              onChange={(v) => setSetting('environmentIntensity', v)} 
              resetKey="environmentIntensity"
            />
            </div>
            </div>
          </Show>
        </div>
        
        {/* Clouds */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer" onClick={() => toggleSection('clouds')}>
            <IconCloud class="w-3 h-3" />
            Clouds
          </div>
          <Show when={sectionsOpen().clouds}>
            <div class="!p-2">
            <div class="space-y-0.5">
            <ToggleControl 
              label="Enable Clouds" 
              value={lighting().cloudsEnabled} 
              onChange={(v) => setSetting('cloudsEnabled', v)}
              resetKey="cloudsEnabled"
            />
            <SliderControl 
              label="Cloud Size" 
              getValue={() => lighting().cloudSize} 
              min={1} 
              max={100} 
              step={1} 
              onChange={(v) => setSetting('cloudSize', v)} 
              resetKey="cloudSize"
            />
            <SliderControl 
              label="Cloud Density" 
              getValue={() => lighting().cloudDensity} 
              min={0} 
              max={2.0} 
              step={0.05} 
              onChange={(v) => setSetting('cloudDensity', v)} 
              resetKey="cloudDensity"
            />
            </div>
            </div>
          </Show>
        </div>
        
        {/* Fog */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer" onClick={() => toggleSection('fog')}>
            <IconEye class="w-3 h-3" />
            Fog
          </div>
          <Show when={sectionsOpen().fog}>
            <div class="!p-2">
            <div class="space-y-0.5">
            <ToggleControl 
              label="Enable Fog" 
              value={lighting().fogEnabled} 
              onChange={(v) => setSetting('fogEnabled', v)}
              resetKey="fogEnabled"
            />
            
            {lighting().fogEnabled && (
              <>
                <ColorControl 
                  label="Day Fog Color" 
                  value={lighting().fogColorDay} 
                  onChange={(v) => setSetting('fogColorDay', v)} 
                  resetKey="fogColorDay"
                />
                <ColorControl 
                  label="Night Fog Color" 
                  value={lighting().fogColorNight} 
                  onChange={(v) => setSetting('fogColorNight', v)} 
                  resetKey="fogColorNight"
                />
                <SliderControl 
                  label="Day Fog Density" 
                  getValue={() => lighting().fogDensityDay} 
                  min={0} 
                  max={0.1} 
                  step={0.0001} 
                  onChange={(v) => setSetting('fogDensityDay', v)} 
                  resetKey="fogDensityDay"
                />
                <SliderControl 
                  label="Night Fog Density" 
                  getValue={() => lighting().fogDensityNight} 
                  min={0} 
                  max={0.1} 
                  step={0.0001} 
                  onChange={(v) => setSetting('fogDensityNight', v)} 
                  resetKey="fogDensityNight"
                />
              </>
            )}
            </div>
            </div>
          </Show>
        </div>

        {/* Light Sources */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer" onClick={() => toggleSection('lights')}>
            <IconSun class="w-3 h-3" />
            Light Sources
          </div>
          <Show when={sectionsOpen().lights}>
            <div class="!p-2">
            <div class="space-y-0.5">
            {/* Sun Light */}
            <div class="bg-base-100/30 rounded-lg p-0.5">
              <div class="text-xs font-medium mb-0.5">Sun Light</div>
              <div class="space-y-1">
                <SliderControl 
                  label="Intensity" 
                  getValue={() => lighting().sunIntensity} 
                  min={0} 
                  max={50} 
                  step={0.1} 
                  onChange={(v) => setSetting('sunIntensity', v)} 
                  resetKey="sunIntensity"
                />
                <ColorControl 
                  label="Color" 
                  value={lighting().sunColor} 
                  onChange={(v) => setSetting('sunColor', v)} 
                  resetKey="sunColor"
                />
              </div>
            </div>
            
            {/* Sky Light */}
            <div class="bg-base-100/30 rounded-lg p-0.5">
              <div class="text-xs font-medium mb-0.5">Sky Light (Ambient)</div>
              <div class="space-y-1">
                <SliderControl 
                  label="Intensity" 
                  getValue={() => lighting().skyIntensity} 
                  min={0} 
                  max={50} 
                  step={0.1} 
                  onChange={(v) => setSetting('skyIntensity', v)} 
                  resetKey="skyIntensity"
                />
                <ColorControl 
                  label="Color" 
                  value={lighting().skyColor} 
                  onChange={(v) => setSetting('skyColor', v)} 
                  resetKey="skyColor"
                />
              </div>
            </div>
            
            {/* Rim Light */}
            <div class="bg-base-100/30 rounded-lg p-0.5">
              <div class="text-xs font-medium mb-0.5">Rim Light (Atmospheric)</div>
              <div class="space-y-1">
                <SliderControl 
                  label="Intensity" 
                  getValue={() => lighting().rimIntensity} 
                  min={0} 
                  max={20} 
                  step={0.1} 
                  onChange={(v) => setSetting('rimIntensity', v)} 
                  resetKey="rimIntensity"
                />
                <ColorControl 
                  label="Color" 
                  value={lighting().rimColor} 
                  onChange={(v) => setSetting('rimColor', v)} 
                  resetKey="rimColor"
                />
              </div>
            </div>
            
            {/* Bounce Light */}
            <div class="bg-base-100/30 rounded-lg p-0.5">
              <div class="text-xs font-medium mb-0.5">Bounce Light (Indirect)</div>
              <div class="space-y-1">
                <SliderControl 
                  label="Intensity" 
                  getValue={() => lighting().bounceIntensity} 
                  min={0} 
                  max={20} 
                  step={0.1} 
                  onChange={(v) => setSetting('bounceIntensity', v)} 
                  resetKey="bounceIntensity"
                />
                <ColorControl 
                  label="Color" 
                  value={lighting().bounceColor} 
                  onChange={(v) => setSetting('bounceColor', v)} 
                  resetKey="bounceColor"
                />
              </div>
            </div>
            
            {/* Moon Light */}
            <div class="bg-base-100/30 rounded-lg p-0.5">
              <div class="text-xs font-medium mb-0.5">Moon Light</div>
              <div class="space-y-1">
                <SliderControl 
                  label="Intensity" 
                  getValue={() => lighting().moonIntensity} 
                  min={0} 
                  max={100} 
                  step={0.5} 
                  onChange={(v) => setSetting('moonIntensity', v)} 
                  resetKey="moonIntensity"
                />
              </div>
            </div>
            </div>
            </div>
          </Show>
        </div>
        
        {/* Shadows */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer" onClick={() => toggleSection('shadows')}>
            <IconSettings class="w-3 h-3" />
            Shadows
          </div>
          <Show when={sectionsOpen().shadows}>
            <div class="!p-2">
            <div class="space-y-0.5">
            <ToggleControl 
              label="Cascade Shadows" 
              value={lighting().cascadeShadows} 
              onChange={(v) => setSetting('cascadeShadows', v)}
              resetKey="cascadeShadows"
            />
            <ToggleControl 
              label="Contact Hardening" 
              value={lighting().contactHardeningShadows} 
              onChange={(v) => setSetting('contactHardeningShadows', v)}
              resetKey="contactHardeningShadows"
            />
            
            <SliderControl 
              label="Shadow Darkness" 
              getValue={() => lighting().shadowDarkness} 
              min={0} 
              max={1.0} 
              step={0.05} 
              onChange={(v) => setSetting('shadowDarkness', v)} 
              resetKey="shadowDarkness"
            />
            <SliderControl 
              label="Shadow Softness" 
              getValue={() => lighting().shadowBlur} 
              min={0} 
              max={256} 
              step={4} 
              onChange={(v) => setSetting('shadowBlur', v)} 
              resetKey="shadowBlur"
            />
            <SliderControl 
              label="Shadow Map Size" 
              getValue={() => lighting().shadowMapSize} 
              min={512} 
              max={8192} 
              step={512} 
              onChange={(v) => setSetting('shadowMapSize', v)} 
              resetKey="shadowMapSize"
            />
            </div>
            </div>
          </Show>
        </div>
        
        {/* Particles */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer" onClick={() => toggleSection('particles')}>
            <IconBulb class="w-3 h-3" />
            Particles
          </div>
          <Show when={sectionsOpen().particles}>
            <div class="!p-2">
            <div class="space-y-0.5">
            <ToggleControl 
              label="Snow" 
              value={lighting().snowEnabled} 
              onChange={(v) => setSetting('snowEnabled', v)}
              resetKey="snowEnabled"
            />
            <ToggleControl 
              label="Stars" 
              value={lighting().starsEnabled} 
              onChange={(v) => setSetting('starsEnabled', v)}
              resetKey="starsEnabled"
            />
            
            <SliderControl 
              label="Snow Intensity" 
              getValue={() => lighting().snowIntensity} 
              min={0} 
              max={1000} 
              step={10} 
              onChange={(v) => setSetting('snowIntensity', v)} 
              resetKey="snowIntensity"
            />
            <SliderControl 
              label="Star Count" 
              getValue={() => lighting().starIntensity} 
              min={0} 
              max={10000} 
              step={100} 
              onChange={(v) => setSetting('starIntensity', v)} 
              resetKey="starIntensity"
            />
            </div>
            </div>
          </Show>
        </div>
        
        {/* Reset Button */}
        <div class="p-1">
          <button 
            onClick={() => renderActions.resetLightingSettings()}
            class="btn btn-outline btn-error btn-xs w-full"
          >
            Reset All Settings
          </button>
        </div>
      </div>
    </div>
  );
}