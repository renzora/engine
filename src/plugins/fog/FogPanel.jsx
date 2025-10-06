import { createEffect, createMemo, createSignal } from 'solid-js';
import { renderStore, renderActions } from '@/render/store';
import { IconEye, IconDroplet, IconCloud, IconSettings, IconRefresh } from '@tabler/icons-solidjs';
import { Color3 } from '@babylonjs/core/Maths/math.color';

export default function FogPanel() {
  // Get lighting settings from render store
  const lighting = () => renderStore.lighting;
  
  // Generic setter function
  const setSetting = (key, value) => renderActions.setLightingSetting(key, value);
  
  // Section collapse state
  const [sectionsOpen, setSectionsOpen] = createSignal({
    fog: true,
    distance: true,
    appearance: true,
    presets: true
  });
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };
  
  // Apply fog changes to the scene in real-time
  createEffect(() => {
    const scene = window._cleanBabylonScene || renderStore.scene;
    if (!scene) return;
    
    const l = lighting();
    
    // Update fog settings
    scene.fogEnabled = l.fogEnabled;
    
    if (l.fogEnabled) {
      // Set fog mode based on settings
      if (l.fogMode === 'linear') {
        scene.fogMode = 1; // BABYLON.Scene.FOGMODE_LINEAR
        scene.fogStart = l.fogStart;
        scene.fogEnd = l.fogEnd;
      } else if (l.fogMode === 'exponential') {
        scene.fogMode = 2; // BABYLON.Scene.FOGMODE_EXP
        scene.fogDensity = l.fogDensity;
      } else if (l.fogMode === 'exponential2') {
        scene.fogMode = 3; // BABYLON.Scene.FOGMODE_EXP2
        scene.fogDensity = l.fogDensity;
      }
      
      // Use single fog color
      const fogColor = l.fogColor || l.fogColorDay || [0.7, 0.7, 0.7];
      scene.fogColor = new Color3(fogColor[0], fogColor[1], fogColor[2]);
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

  const SliderControl = ({ label, getValue, min, max, step, onChange, unit = '', disabled = false }) => {
    const displayValue = createMemo(() => {
      const value = getValue();
      if (typeof value !== 'number') return value;
      if (step < 0.01) return value.toFixed(4);
      if (step < 0.1) return value.toFixed(2);
      if (step < 1) return value.toFixed(1);
      return value.toFixed(0);
    });
    
    return (
      <div class={disabled ? 'opacity-50' : ''}>
        <label class="block text-xs text-base-content/80 mb-1">
          {label}: {displayValue()}{unit}
        </label>
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={getValue()}
          onInput={(e) => onChange(parseFloat(e.target.value))}
          class="range range-primary w-full range-xs"
          disabled={disabled}
        />
      </div>
    );
  };

  const ColorControl = ({ label, value, onChange, disabled = false }) => (
    <div class={disabled ? 'opacity-50' : ''}>
      <label class="block text-xs text-base-content/80 mb-1">{label}</label>
      <input
        type="color"
        value={rgbToHex(value)}
        onInput={(e) => onChange(hexToRgb(e.target.value))}
        class="w-full h-6 rounded border border-base-300"
        disabled={disabled}
      />
    </div>
  );

  const ToggleControl = ({ label, value, onChange, icon, description }) => (
    <div class="space-y-1">
      <div class="flex items-center justify-between">
        <label class="text-xs text-base-content/80 flex items-center gap-1">
          {icon && <icon class="w-3 h-3" />}
          {label}
        </label>
        <input
          type="checkbox"
          checked={value}
          onChange={(e) => onChange(e.target.checked)}
          class="toggle toggle-primary toggle-xs"
        />
      </div>
      {description && (
        <p class="text-xs text-base-content/60">{description}</p>
      )}
    </div>
  );

  const resetFogSettings = () => {
    setSetting('fogEnabled', false);
    setSetting('fogMode', 'linear');
    setSetting('fogStart', 20);
    setSetting('fogEnd', 200);
    setSetting('fogDensity', 0.01);
    setSetting('fogColor', [0.7, 0.7, 0.7]);
    setSetting('fogIntensity', 1.0);
    setSetting('fogHeightFalloff', 0.0);
  };

  return (
    <div class="h-full flex flex-col">
      <div class="flex-1 p-2 space-y-2">
      
      {/* Fog Enable/Disable */}
      <div class="bg-base-100 border-base-300 border rounded-lg">
        <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().fog ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('fog')}>
          <IconEye class="w-3 h-3" />
          Fog Settings
        </div>
        {sectionsOpen().fog && (
          <div class="!p-2">
            <div class="space-y-2">
              <ToggleControl 
                label="Enable Fog" 
                value={lighting().fogEnabled} 
                onChange={(v) => setSetting('fogEnabled', v)}
                icon={IconCloud}
                description="Enable atmospheric fog effects for depth perception"
              />
              
              {lighting().fogEnabled && (
                <div class="space-y-2">
                  <div>
                    <label class="block text-xs text-base-content/80 mb-1">Fog Type</label>
                    <select 
                      value={lighting().fogMode || 'linear'}
                      onChange={(e) => setSetting('fogMode', e.target.value)}
                      class="select select-xs w-full border border-base-300"
                    >
                      <option value="linear">Linear</option>
                      <option value="exponential">Exponential</option>
                      <option value="exponential2">Exponential²</option>
                    </select>
                    <p class="text-xs text-base-content/60 mt-1">
                      Linear fog has distinct start/end points, exponential fog fades naturally
                    </p>
                  </div>
                  
                  <SliderControl 
                    label="Fog Intensity" 
                    getValue={() => lighting().fogIntensity || 1.0} 
                    min={0} 
                    max={3.0} 
                    step={0.1} 
                    onChange={(v) => setSetting('fogIntensity', v)} 
                  />
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Distance Controls */}
      {lighting().fogEnabled && (
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().distance ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('distance')}>
            <IconDroplet class="w-3 h-3" />
            Distance Controls
          </div>
          {sectionsOpen().distance && (
            <div class="!p-2">
              <div class="space-y-2">
                {(lighting().fogMode || 'linear') === 'linear' ? (
                  <>
                    <SliderControl 
                      label="Fog Start Distance" 
                      getValue={() => lighting().fogStart || 20} 
                      min={0} 
                      max={500} 
                      step={1} 
                      onChange={(v) => setSetting('fogStart', v)}
                      unit=" units" 
                    />
                    <SliderControl 
                      label="Fog End Distance" 
                      getValue={() => lighting().fogEnd || 200} 
                      min={1} 
                      max={1000} 
                      step={1} 
                      onChange={(v) => setSetting('fogEnd', v)}
                      unit=" units" 
                    />
                  </>
                ) : (
                  <SliderControl 
                    label="Fog Density" 
                    getValue={() => lighting().fogDensity || 0.01} 
                    min={0.001} 
                    max={0.1} 
                    step={0.001} 
                    onChange={(v) => setSetting('fogDensity', v)} 
                  />
                )}
                
                <SliderControl 
                  label="Height Falloff" 
                  getValue={() => lighting().fogHeightFalloff || 0.0} 
                  min={0} 
                  max={0.1} 
                  step={0.001} 
                  onChange={(v) => setSetting('fogHeightFalloff', v)}
                />
                <p class="text-xs text-base-content/60">
                  Height falloff creates fog that is denser near the ground
                </p>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Appearance */}
      {lighting().fogEnabled && (
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().appearance ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('appearance')}>
            <IconSettings class="w-3 h-3" />
            Appearance
          </div>
          {sectionsOpen().appearance && (
            <div class="!p-2">
              <div class="space-y-2">
                <ColorControl 
                  label="Fog Color" 
                  value={lighting().fogColor || lighting().fogColorDay || [0.7, 0.7, 0.7]} 
                  onChange={(v) => setSetting('fogColor', v)} 
                />
              </div>
            </div>
          )}
        </div>
      )}

      {/* Presets */}
      {lighting().fogEnabled && (
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().presets ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('presets')}>
            <IconCloud class="w-3 h-3" />
            Fog Presets
          </div>
          {sectionsOpen().presets && (
            <div class="!p-2">
              <div class="grid grid-cols-2 gap-2">
                <button 
                  onClick={() => {
                    setSetting('fogMode', 'linear');
                    setSetting('fogStart', 10);
                    setSetting('fogEnd', 100);
                    setSetting('fogColor', [0.9, 0.9, 0.95]);
                    setSetting('fogIntensity', 0.8);
                  }}
                  class="btn btn-xs btn-outline"
                >
                  Light Haze
                </button>
                <button 
                  onClick={() => {
                    setSetting('fogMode', 'exponential');
                    setSetting('fogDensity', 0.02);
                    setSetting('fogColor', [0.7, 0.8, 0.9]);
                    setSetting('fogIntensity', 1.2);
                  }}
                  class="btn btn-xs btn-outline"
                >
                  Heavy Fog
                </button>
                <button 
                  onClick={() => {
                    setSetting('fogMode', 'linear');
                    setSetting('fogStart', 50);
                    setSetting('fogEnd', 300);
                    setSetting('fogColor', [0.6, 0.7, 0.8]);
                    setSetting('fogIntensity', 1.0);
                  }}
                  class="btn btn-xs btn-outline"
                >
                  Distance Fog
                </button>
                <button 
                  onClick={() => {
                    setSetting('fogMode', 'exponential2');
                    setSetting('fogDensity', 0.008);
                    setSetting('fogHeightFalloff', 0.02);
                    setSetting('fogColor', [0.8, 0.85, 0.9]);
                    setSetting('fogIntensity', 1.1);
                  }}
                  class="btn btn-xs btn-outline"
                >
                  Ground Fog
                </button>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Reset Button */}
      <div class="pt-2 border-t border-base-300">
        <button 
          onClick={resetFogSettings}
          class="btn btn-outline btn-error btn-xs w-full flex items-center gap-2"
        >
          <IconRefresh class="w-3 h-3" />
          Reset Fog Settings
        </button>
      </div>
      </div>
    </div>
  );
}