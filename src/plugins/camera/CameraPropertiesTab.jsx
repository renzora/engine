import { createSignal, createEffect, Show } from 'solid-js';
import { IconCamera, IconEye, IconSettings } from '@tabler/icons-solidjs';
import { cameraSettings, cameraActions } from './cameraStore.jsx';

export default function CameraPropertiesTab(props) {
  // Get the selected camera object
  const selectedObject = () => props.selectedObject;
  
  // Check if the selected object is a camera
  const isCamera = () => {
    const obj = selectedObject();
    return obj && obj.getClassName && (
      obj.getClassName().includes('Camera') || 
      obj.getClassName() === 'UniversalCamera' ||
      obj.getClassName() === 'ArcRotateCamera' ||
      obj.getClassName() === 'FreeCamera'
    );
  };
  
  // Section collapse state
  const [sectionsOpen, setSectionsOpen] = createSignal({
    basic: true,
    effects: false
  });
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };
  
  // Use reactive camera settings from store
  const settings = () => cameraSettings();
  const { setFOV, setVignetteEnabled, setVignetteAmount, setVignetteColor, resetToDefaults } = cameraActions;
  
  // Initialize values from camera
  createEffect(() => {
    const camera = selectedObject();
    if (isCamera() && camera) {
      // Get FOV from camera (convert from radians to degrees)
      if (camera.fov !== undefined) {
        setFOV(Math.round((camera.fov * 180) / Math.PI));
      }
    }
  });
  
  // Initialize camera settings when a camera is selected
  createEffect(() => {
    const camera = selectedObject();
    if (isCamera() && camera) {
      // Get FOV from camera (convert from radians to degrees)
      if (camera.fov !== undefined) {
        setFOV(Math.round((camera.fov * 180) / Math.PI));
      }
    }
  });
  
  // Convert RGB array to hex for color input
  const rgbToHex = (rgb) => {
    const r = Math.round(rgb[0] * 255).toString(16).padStart(2, '0');
    const g = Math.round(rgb[1] * 255).toString(16).padStart(2, '0');
    const b = Math.round(rgb[2] * 255).toString(16).padStart(2, '0');
    return `#${r}${g}${b}`;
  };
  
  // Convert hex to RGB array
  const hexToRgb = (hex) => {
    const r = parseInt(hex.slice(1, 3), 16) / 255;
    const g = parseInt(hex.slice(3, 5), 16) / 255;
    const b = parseInt(hex.slice(5, 7), 16) / 255;
    return [r, g, b];
  };
  
  // Reset handled by cameraActions.resetToDefaults
  
  const SliderControl = ({ label, getValue, min, max, step, onChange, unit = '' }) => {
    const displayValue = () => {
      const value = getValue();
      if (typeof value !== 'number') return value;
      if (step < 0.01) return value.toFixed(4);
      if (step < 0.1) return value.toFixed(2);
      if (step < 1) return value.toFixed(1);
      return value.toFixed(0);
    };
    
    return (
      <div>
        <label class="text-xs text-base-content/80 mb-1 block">
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
        />
      </div>
    );
  };

  const ColorControl = ({ label, value, onChange }) => {
    return (
      <div>
        <label class="text-xs text-base-content/80 mb-1 block">{label}</label>
        <input
          type="color"
          value={rgbToHex(value)}
          onInput={(e) => onChange(hexToRgb(e.target.value))}
          class="w-full h-6 rounded border border-base-300"
        />
      </div>
    );
  };

  const ToggleControl = ({ label, value, onChange }) => {
    return (
      <div class="flex items-center justify-between">
        <label class="text-xs text-base-content/80">{label}</label>
        <input
          type="checkbox"
          checked={value}
          onChange={(e) => onChange(e.target.checked)}
          class="toggle toggle-primary toggle-xs"
        />
      </div>
    );
  };

  // Don't render if no camera is selected
  if (!isCamera()) {
    return null;
  }

  return (
    <div class="h-full flex flex-col">
      {/* Header */}
      <div class="flex items-center justify-between p-3 border-b border-base-300">
        <div class="flex items-center gap-2">
          <IconCamera class="w-4 h-4" />
          <span class="font-medium text-sm">Camera Settings</span>
        </div>
        <button
          onClick={resetToDefaults}
          class="btn btn-xs btn-ghost"
          title="Reset to defaults"
        >
          Reset
        </button>
      </div>
      
      {/* Content */}
      <div class="flex-1 space-y-2 p-2 overflow-y-auto">
        
        {/* Basic Camera Settings */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().basic ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
            <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('basic')}>
              <IconSettings class="w-3 h-3" />
              Basic Settings
            </div>
            <input
              type="checkbox"
              checked={sectionsOpen().basic}
              onChange={(e) => {
                e.stopPropagation();
                toggleSection('basic');
              }}
              onClick={(e) => e.stopPropagation()}
              class="toggle toggle-primary toggle-xs"
            />
          </div>
          <Show when={sectionsOpen().basic}>
            <div class="!p-2">
              <div class="space-y-0.5">
                <div class="form-control">
                  <div class="flex items-center justify-between mb-1">
                    <label class="text-xs font-medium text-base-content">Field of View</label>
                    <span class="text-xs text-base-content/60">{settings().fov}°</span>
                  </div>
                  <input
                    type="range"
                    min={30}
                    max={120}
                    step={1}
                    value={settings().fov}
                    onInput={(e) => setFOV(parseFloat(e.target.value))}
                    class="range range-primary range-xs"
                  />
                </div>
              </div>
            </div>
          </Show>
        </div>
        
        {/* Visual Effects */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().effects ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
            <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('effects')}>
              <IconEye class="w-3 h-3" />
              Visual Effects
            </div>
            <input
              type="checkbox"
              checked={sectionsOpen().effects}
              onChange={(e) => {
                e.stopPropagation();
                toggleSection('effects');
              }}
              onClick={(e) => e.stopPropagation()}
              class="toggle toggle-primary toggle-xs"
            />
          </div>
          <Show when={sectionsOpen().effects}>
            <div class="!p-2">
              <div class="space-y-0.5">
                <div class="form-control">
                  <div class="flex items-center justify-between">
                    <label class="text-xs font-medium text-base-content">Vignette</label>
                    <input
                      type="checkbox"
                      checked={settings().vignette.enabled}
                      onChange={(e) => setVignetteEnabled(e.target.checked)}
                      class="toggle toggle-primary toggle-sm"
                    />
                  </div>
                </div>
                
                <Show when={settings().vignette.enabled}>
                  <div class="space-y-0.5 pl-4 border-l-2 border-base-300">
                    <div class="form-control">
                      <div class="flex items-center justify-between mb-1">
                        <label class="text-xs font-medium text-base-content">Vignette Amount</label>
                        <span class="text-xs text-base-content/60">{settings().vignette.amount.toFixed(2)}</span>
                      </div>
                      <input
                        type="range"
                        min={0}
                        max={1}
                        step={0.01}
                        value={settings().vignette.amount}
                        onInput={(e) => setVignetteAmount(parseFloat(e.target.value))}
                        class="range range-primary range-xs"
                      />
                    </div>
                    
                    <div class="form-control">
                      <label class="text-xs font-medium text-base-content mb-1">Vignette Color</label>
                      <input
                        type="color"
                        value={rgbToHex(settings().vignette.color)}
                        onInput={(e) => setVignetteColor(hexToRgb(e.target.value))}
                        class="w-full h-6 rounded border border-base-300"
                      />
                    </div>
                  </div>
                </Show>
              </div>
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
}