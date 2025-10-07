import { createMemo, createSignal } from 'solid-js';
import { weatherStore, weatherActions } from '@/stores/WeatherStore';
import { IconCloudRain, IconDroplet, IconSettings, IconRefresh, IconEye } from '@tabler/icons-solidjs';

export default function RainPanel() {
  // Note: Rain rendering is now handled by the centralized WeatherRenderer
  // This panel now only provides UI controls for the weather store
  const rain = () => weatherStore.rain;
  
  // Section collapse state
  const [sectionsOpen, setSectionsOpen] = createSignal({
    rain: true,
    intensity: true,
    appearance: true,
    presets: true
  });
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };

  // Note: Rain rendering is now handled by the centralized WeatherRenderer

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

  const resetRainSettings = () => {
    weatherActions.resetRain();
  };

  return (
    <div class="h-full flex flex-col">
      <div class="flex-1 p-2 space-y-2">
      
      {/* Rain Enable/Disable */}
      <div class="bg-base-100 border-base-300 border rounded-lg">
        <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().rain ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('rain')}>
          <IconEye class="w-3 h-3" />
          Rain Settings
        </div>
        {sectionsOpen().rain && (
          <div class="!p-2">
            <div class="space-y-2">
              <ToggleControl 
                label="Enable Rain" 
                value={rain().enabled} 
                onChange={(v) => weatherActions.setRainEnabled(v)}
                icon={IconCloudRain}
                description="Enable rain particle effects"
              />
            </div>
          </div>
        )}
      </div>

      {/* Intensity Controls */}
      {rain().enabled && (
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().intensity ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('intensity')}>
            <IconDroplet class="w-3 h-3" />
            Intensity Controls
          </div>
          {sectionsOpen().intensity && (
            <div class="!p-2">
              <div class="space-y-2">
                <SliderControl 
                  label="Rain Intensity" 
                  getValue={() => rain().intensity} 
                  min={0.1} 
                  max={3.0} 
                  step={0.1} 
                  onChange={(v) => weatherActions.setRainIntensity(v)}
                />
                
                <SliderControl 
                  label="Raindrop Size" 
                  getValue={() => rain().size} 
                  min={0.5} 
                  max={3.0} 
                  step={0.1} 
                  onChange={(v) => weatherActions.setRainSize(v)}
                />
                
                <SliderControl 
                  label="Wind Strength" 
                  getValue={() => rain().windStrength} 
                  min={0.0} 
                  max={2.0} 
                  step={0.1} 
                  onChange={(v) => weatherActions.setRainWindStrength(v)}
                />
                <p class="text-xs text-base-content/60">
                  Wind effect changes the direction of falling rain
                </p>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Appearance */}
      {rain().enabled && (
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().appearance ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('appearance')}>
            <IconSettings class="w-3 h-3" />
            Appearance
          </div>
          {sectionsOpen().appearance && (
            <div class="!p-2">
              <div class="space-y-2">
                <ColorControl 
                  label="Rain Color" 
                  value={rain().color} 
                  onChange={(v) => weatherActions.setRainColor(v)} 
                />
              </div>
            </div>
          )}
        </div>
      )}

      {/* Presets */}
      {rain().enabled && (
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().presets ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('presets')}>
            <IconCloudRain class="w-3 h-3" />
            Rain Presets
          </div>
          {sectionsOpen().presets && (
            <div class="!p-2">
              <div class="grid grid-cols-2 gap-2">
                <button 
                  onClick={() => {
                    setSetting('rainIntensity', 0.5);
                    setSetting('rainSize', 0.8);
                    setSetting('rainWind', 0.0);
                    setSetting('rainColor', [0.9, 0.95, 1.0]);
                  }}
                  class="btn btn-xs btn-outline"
                >
                  Light Drizzle
                </button>
                <button 
                  onClick={() => {
                    setSetting('rainIntensity', 1.5);
                    setSetting('rainSize', 1.2);
                    setSetting('rainWind', 0.5);
                    setSetting('rainColor', [0.7, 0.8, 0.9]);
                  }}
                  class="btn btn-xs btn-outline"
                >
                  Heavy Rain
                </button>
                <button 
                  onClick={() => {
                    setSetting('rainIntensity', 2.0);
                    setSetting('rainSize', 1.5);
                    setSetting('rainWind', 1.2);
                    setSetting('rainColor', [0.6, 0.7, 0.8]);
                  }}
                  class="btn btn-xs btn-outline"
                >
                  Storm
                </button>
                <button 
                  onClick={() => {
                    setSetting('rainIntensity', 0.3);
                    setSetting('rainSize', 0.6);
                    setSetting('rainWind', -0.3);
                    setSetting('rainColor', [0.8, 0.9, 1.0]);
                  }}
                  class="btn btn-xs btn-outline"
                >
                  Gentle Rain
                </button>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Reset Button */}
      <div class="pt-2 border-t border-base-300">
        <button 
          onClick={resetRainSettings}
          class="btn btn-outline btn-error btn-xs w-full flex items-center gap-2"
        >
          <IconRefresh class="w-3 h-3" />
          Reset Rain Settings
        </button>
      </div>
      </div>
    </div>
  );
}