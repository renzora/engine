import { createEffect, onCleanup, createMemo } from 'solid-js';
import { renderStore, renderActions } from '@/render/store';
import { Sun, Lightbulb, Moon, Palette, Camera, Settings, Eye, Cloud, Clock } from '@/ui/icons';
import { ImageProcessingConfiguration } from '@babylonjs/core/Materials/imageProcessingConfiguration';

export default function LightDropdownContent() {
  const lighting = () => renderStore.lighting;
  
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

  const SliderControl = ({ label, getValue, min, max, step, onChange, unit = '' }) => {
    const displayValue = createMemo(() => {
      const value = getValue();
      if (typeof value !== 'number') return value;
      if (step < 0.01) return value.toFixed(4);
      if (step < 0.1) return value.toFixed(2);
      if (step < 1) return value.toFixed(1);
      return value.toFixed(0);
    });
    
    return (
      <div>
        <label class="block text-sm text-base-content/80 mb-1">
          {label}: {displayValue()}{unit}
        </label>
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={getValue()}
          onInput={(e) => onChange(parseFloat(e.target.value))}
          class="range range-primary w-full"
        />
      </div>
    );
  };

  const ColorControl = ({ label, value, onChange }) => (
    <div>
      <label class="block text-sm text-base-content/80 mb-1">{label}</label>
      <input
        type="color"
        value={rgbToHex(value)}
        onInput={(e) => onChange(hexToRgb(e.target.value))}
        class="w-full h-8 rounded border border-base-300"
      />
    </div>
  );

  const ToggleControl = ({ label, value, onChange, icon }) => (
    <div class="flex items-center justify-between">
      <label class="text-sm text-base-content/80 flex items-center gap-1">
        {icon && <icon class="w-4 h-4" />}
        {label}
      </label>
      <input
        type="checkbox"
        checked={value}
        onChange={(e) => onChange(e.target.checked)}
        class="toggle toggle-primary toggle-sm"
      />
    </div>
  );

  return (
    <div class="w-96 space-y-6 p-4 bg-base-200 text-base-content max-h-[90vh] overflow-y-auto">
      
      {/* Time Control */}
      <div>
        <div class="flex items-center justify-between mb-2">
          <label class="block font-medium text-base-content flex items-center gap-1">
            <Clock class="w-4 h-4" />
            Time Control
          </label>
          <button
            onClick={toggleTime}
            class={`btn btn-xs ${timeStatusClass()}`}
          >
            {timeStatus()}
          </button>
        </div>
        
        <div class="bg-base-100 rounded-lg p-2 text-center mb-3">
          <div class="text-lg font-mono">{currentTime()}</div>
          <div class="text-xs text-base-content/60">Current Time</div>
        </div>
        
        <div class="space-y-2">
          <SliderControl 
            label="Time of Day" 
            getValue={() => lighting().timeOfDay} 
            min={0} 
            max={24} 
            step={0.1} 
            onChange={setTimeOfDay}
            unit="h" 
          />
          <SliderControl 
            label="Time Speed" 
            getValue={() => lighting().timeSpeed} 
            min={0} 
            max={5} 
            step={0.1} 
            onChange={setTimeSpeed}
            unit="x" 
          />
          <SliderControl 
            label="Sunrise Hour" 
            getValue={() => lighting().sunriseHour} 
            min={0} 
            max={12} 
            step={0.5} 
            onChange={(v) => setSetting('sunriseHour', v)}
            unit="h" 
          />
          <SliderControl 
            label="Sunset Hour" 
            getValue={() => lighting().sunsetHour} 
            min={12} 
            max={24} 
            step={0.5} 
            onChange={(v) => setSetting('sunsetHour', v)}
            unit="h" 
          />
        </div>
      </div>
      
      {/* Post Processing */}
      <div>
        <label class="block font-medium text-base-content mb-2">
          <Camera class="w-4 h-4 inline mr-1" />
          Post Processing
        </label>
        
        <div class="space-y-2">
          <ToggleControl 
            label="Tone Mapping" 
            value={lighting().toneMappingEnabled} 
            onChange={(v) => setSetting('toneMappingEnabled', v)}
          />
          
          {lighting().toneMappingEnabled && (
            <div>
              <label class="block text-sm text-base-content/80 mb-1">Tone Mapping Type</label>
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
          />
          <ToggleControl 
            label="Vignette" 
            value={lighting().vignetteEnabled} 
            onChange={(v) => setSetting('vignetteEnabled', v)}
          />
          
          <SliderControl 
            label="Exposure" 
            getValue={() => lighting().exposure} 
            min={0.1} 
            max={5.0} 
            step={0.05} 
            onChange={(v) => setSetting('exposure', v)} 
          />
          <SliderControl 
            label="Contrast" 
            getValue={() => lighting().contrast} 
            min={0.1} 
            max={3.0} 
            step={0.05} 
            onChange={(v) => setSetting('contrast', v)} 
          />
          <SliderControl 
            label="Brightness" 
            getValue={() => lighting().brightness} 
            min={-1.0} 
            max={1.0} 
            step={0.05} 
            onChange={(v) => setSetting('brightness', v)} 
          />
          <SliderControl 
            label="Saturation" 
            getValue={() => lighting().saturation} 
            min={0.0} 
            max={3.0} 
            step={0.05} 
            onChange={(v) => setSetting('saturation', v)} 
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
              />
              <SliderControl 
                label="Vignette Stretch" 
                getValue={() => lighting().vignetteStretch} 
                min={0} 
                max={2} 
                step={0.05} 
                onChange={(v) => setSetting('vignetteStretch', v)} 
              />
            </>
          )}
        </div>
      </div>
      
      {/* Sky & Atmosphere */}
      <div>
        <label class="block font-medium text-base-content mb-2">
          <Palette class="w-4 h-4 inline mr-1" />
          Sky & Atmosphere
        </label>
        
        <div class="space-y-2">
          <ColorControl 
            label="Night Sky Color" 
            value={lighting().nightSkyColor} 
            onChange={(v) => setSetting('nightSkyColor', v)} 
          />
          <ColorControl 
            label="Day Sky Color" 
            value={lighting().daySkyColor} 
            onChange={(v) => setSetting('daySkyColor', v)} 
          />
          
          <SliderControl 
            label="Night Turbidity" 
            getValue={() => lighting().nightTurbidity} 
            min={1} 
            max={200} 
            step={1} 
            onChange={(v) => setSetting('nightTurbidity', v)} 
          />
          <SliderControl 
            label="Day Turbidity" 
            getValue={() => lighting().dayTurbidity} 
            min={0.1} 
            max={50} 
            step={0.1} 
            onChange={(v) => setSetting('dayTurbidity', v)} 
          />
          <SliderControl 
            label="Night Luminance" 
            getValue={() => lighting().baseLuminance} 
            min={0} 
            max={5.0} 
            step={0.01} 
            onChange={(v) => setSetting('baseLuminance', v)} 
          />
          <SliderControl 
            label="Day Luminance" 
            getValue={() => lighting().dayLuminance} 
            min={0} 
            max={10.0} 
            step={0.01} 
            onChange={(v) => setSetting('dayLuminance', v)} 
          />
          <SliderControl 
            label="Environment Intensity" 
            getValue={() => lighting().environmentIntensity} 
            min={0} 
            max={10.0} 
            step={0.1} 
            onChange={(v) => setSetting('environmentIntensity', v)} 
          />
        </div>
      </div>
      
      {/* Clouds */}
      <div>
        <label class="block font-medium text-base-content mb-2">
          <Cloud class="w-4 h-4 inline mr-1" />
          Clouds
        </label>
        
        <div class="space-y-2">
          <ToggleControl 
            label="Enable Clouds" 
            value={lighting().cloudsEnabled} 
            onChange={(v) => setSetting('cloudsEnabled', v)}
          />
          <SliderControl 
            label="Cloud Size" 
            getValue={() => lighting().cloudSize} 
            min={1} 
            max={100} 
            step={1} 
            onChange={(v) => setSetting('cloudSize', v)} 
          />
          <SliderControl 
            label="Cloud Density" 
            getValue={() => lighting().cloudDensity} 
            min={0} 
            max={2.0} 
            step={0.05} 
            onChange={(v) => setSetting('cloudDensity', v)} 
          />
        </div>
      </div>
      
      {/* Fog */}
      <div>
        <label class="block font-medium text-base-content mb-2">
          <Eye class="w-4 h-4 inline mr-1" />
          Fog
        </label>
        
        <div class="space-y-2">
          <ToggleControl 
            label="Enable Fog" 
            value={lighting().fogEnabled} 
            onChange={(v) => setSetting('fogEnabled', v)}
          />
          
          {lighting().fogEnabled && (
            <>
              <ColorControl 
                label="Day Fog Color" 
                value={lighting().fogColorDay} 
                onChange={(v) => setSetting('fogColorDay', v)} 
              />
              <ColorControl 
                label="Night Fog Color" 
                value={lighting().fogColorNight} 
                onChange={(v) => setSetting('fogColorNight', v)} 
              />
              <SliderControl 
                label="Day Fog Density" 
                getValue={() => lighting().fogDensityDay} 
                min={0} 
                max={0.1} 
                step={0.0001} 
                onChange={(v) => setSetting('fogDensityDay', v)} 
              />
              <SliderControl 
                label="Night Fog Density" 
                getValue={() => lighting().fogDensityNight} 
                min={0} 
                max={0.1} 
                step={0.0001} 
                onChange={(v) => setSetting('fogDensityNight', v)} 
              />
            </>
          )}
        </div>
      </div>

      {/* Lights */}
      <div>
        <label class="block font-medium text-base-content mb-2">
          <Sun class="w-4 h-4 inline mr-1" />
          Light Sources
        </label>
        
        <div class="space-y-3">
          {/* Sun Light */}
          <div class="bg-base-100 rounded-lg p-3">
            <div class="text-sm font-medium mb-2">Sun Light</div>
            <div class="space-y-2">
              <SliderControl 
                label="Intensity" 
                getValue={() => lighting().sunIntensity} 
                min={0} 
                max={50} 
                step={0.1} 
                onChange={(v) => setSetting('sunIntensity', v)} 
              />
              <ColorControl 
                label="Color" 
                value={lighting().sunColor} 
                onChange={(v) => setSetting('sunColor', v)} 
              />
            </div>
          </div>
          
          {/* Sky Light */}
          <div class="bg-base-100 rounded-lg p-3">
            <div class="text-sm font-medium mb-2">Sky Light (Ambient)</div>
            <div class="space-y-2">
              <SliderControl 
                label="Intensity" 
                getValue={() => lighting().skyIntensity} 
                min={0} 
                max={50} 
                step={0.1} 
                onChange={(v) => setSetting('skyIntensity', v)} 
              />
              <ColorControl 
                label="Color" 
                value={lighting().skyColor} 
                onChange={(v) => setSetting('skyColor', v)} 
              />
            </div>
          </div>
          
          {/* Rim Light */}
          <div class="bg-base-100 rounded-lg p-3">
            <div class="text-sm font-medium mb-2">Rim Light (Atmospheric)</div>
            <div class="space-y-2">
              <SliderControl 
                label="Intensity" 
                getValue={() => lighting().rimIntensity} 
                min={0} 
                max={20} 
                step={0.1} 
                onChange={(v) => setSetting('rimIntensity', v)} 
              />
              <ColorControl 
                label="Color" 
                value={lighting().rimColor} 
                onChange={(v) => setSetting('rimColor', v)} 
              />
            </div>
          </div>
          
          {/* Bounce Light */}
          <div class="bg-base-100 rounded-lg p-3">
            <div class="text-sm font-medium mb-2">Bounce Light (Indirect)</div>
            <div class="space-y-2">
              <SliderControl 
                label="Intensity" 
                getValue={() => lighting().bounceIntensity} 
                min={0} 
                max={20} 
                step={0.1} 
                onChange={(v) => setSetting('bounceIntensity', v)} 
              />
              <ColorControl 
                label="Color" 
                value={lighting().bounceColor} 
                onChange={(v) => setSetting('bounceColor', v)} 
              />
            </div>
          </div>
          
          {/* Moon Light */}
          <div class="bg-base-100 rounded-lg p-3">
            <div class="text-sm font-medium mb-2">Moon Light</div>
            <div class="space-y-2">
              <SliderControl 
                label="Intensity" 
                getValue={() => lighting().moonIntensity} 
                min={0} 
                max={100} 
                step={0.5} 
                onChange={(v) => setSetting('moonIntensity', v)} 
              />
            </div>
          </div>
        </div>
      </div>
      
      {/* Shadows */}
      <div>
        <label class="block font-medium text-base-content mb-2">
          <Settings class="w-4 h-4 inline mr-1" />
          Shadows
        </label>
        
        <div class="space-y-2">
          <ToggleControl 
            label="Cascade Shadows" 
            value={lighting().cascadeShadows} 
            onChange={(v) => setSetting('cascadeShadows', v)}
          />
          <ToggleControl 
            label="Contact Hardening" 
            value={lighting().contactHardeningShadows} 
            onChange={(v) => setSetting('contactHardeningShadows', v)}
          />
          
          <SliderControl 
            label="Shadow Darkness" 
            getValue={() => lighting().shadowDarkness} 
            min={0} 
            max={1.0} 
            step={0.05} 
            onChange={(v) => setSetting('shadowDarkness', v)} 
          />
          <SliderControl 
            label="Shadow Softness" 
            getValue={() => lighting().shadowBlur} 
            min={0} 
            max={256} 
            step={4} 
            onChange={(v) => setSetting('shadowBlur', v)} 
          />
          <SliderControl 
            label="Shadow Map Size" 
            getValue={() => lighting().shadowMapSize} 
            min={512} 
            max={8192} 
            step={512} 
            onChange={(v) => setSetting('shadowMapSize', v)} 
          />
        </div>
      </div>
      
      {/* Particles */}
      <div>
        <label class="block font-medium text-base-content mb-2">
          <Lightbulb class="w-4 h-4 inline mr-1" />
          Particles
        </label>
        
        <div class="space-y-2">
          <ToggleControl 
            label="Snow" 
            value={lighting().snowEnabled} 
            onChange={(v) => setSetting('snowEnabled', v)}
          />
          <ToggleControl 
            label="Stars" 
            value={lighting().starsEnabled} 
            onChange={(v) => setSetting('starsEnabled', v)}
          />
          
          <SliderControl 
            label="Snow Intensity" 
            getValue={() => lighting().snowIntensity} 
            min={0} 
            max={1000} 
            step={10} 
            onChange={(v) => setSetting('snowIntensity', v)} 
          />
          <SliderControl 
            label="Star Count" 
            getValue={() => lighting().starIntensity} 
            min={0} 
            max={10000} 
            step={100} 
            onChange={(v) => setSetting('starIntensity', v)} 
          />
        </div>
      </div>
      
      {/* Reset Button */}
      <div class="pt-4 border-t border-base-300">
        <button 
          onClick={() => renderActions.resetLightingSettings()}
          class="btn btn-outline btn-error btn-sm w-full"
        >
          Reset All Settings
        </button>
      </div>
    </div>
  );
}