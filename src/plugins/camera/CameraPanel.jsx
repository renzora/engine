import { createSignal, createEffect, Show } from 'solid-js';
import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";
import { Camera, Move, Eye, Palette, Sun, Cube, Grid3x3 } from '@/ui/icons';

export default function CameraPanel() {
  const { setCameraType, setCameraSpeed, setCameraSensitivity, setCameraFriction, setRenderMode } = viewportActions;
  
  // Default values for reset functionality
  const defaults = {
    speed: 2,
    mouseSensitivity: 0.004,
    friction: 2,
    type: 'universal',
    renderMode: 'solid',
    fov: 60,
    vignetteEnabled: false,
    vignetteAmount: 0.5,
    vignetteColor: [0, 0, 0],
    nightColor: [0.1, 0.1, 0.15]
  };
  
  // Section collapse state
  const [sectionsOpen, setSectionsOpen] = createSignal({
    camera: true,
    render: false,
    effects: false
  });
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };
  
  // Camera settings
  const cameraSpeed = () => viewportStore.camera.speed || 2;
  const mouseSensitivity = () => viewportStore.camera.mouseSensitivity || 0.004;
  const cameraFriction = () => viewportStore.camera.friction || 2;
  const renderMode = () => viewportStore.renderMode || 'solid';
  const cameraType = () => viewportStore.camera.type || 'universal';
  
  // Vignette and FOV signals
  const [vignetteEnabled, setVignetteEnabled] = createSignal(false);
  const [vignetteAmount, setVignetteAmount] = createSignal(0.5);
  const [vignetteColor, setVignetteColor] = createSignal([0, 0, 0]);
  const [cameraFOV, setCameraFOV] = createSignal(60);
  const [nightColor, setNightColor] = createSignal([0.1, 0.1, 0.15]);
  
  // Apply vignette and FOV changes to scene
  createEffect(() => {
    const scene = window._cleanBabylonScene;
    if (!scene || !scene._camera) return;
    
    const camera = scene._camera;
    
    // Update FOV
    camera.fov = (cameraFOV() * Math.PI) / 180;
    
    // Store camera settings globally
    if (!window._cameraSettings) window._cameraSettings = {};
    window._cameraSettings.nightColor = nightColor();
    window._cameraSettings.vignette = {
      enabled: vignetteEnabled(),
      amount: vignetteAmount(),
      color: vignetteColor()
    };
    
    // Update built-in vignette system
    if (window.updateVignetteSettings) {
      window.updateVignetteSettings();
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
  
  const renderModes = [
    { id: 'wireframe', label: 'Wireframe', icon: Grid3x3 },
    { id: 'solid', label: 'Solid', icon: Cube },
    { id: 'material', label: 'Material', icon: Palette },
    { id: 'rendered', label: 'Rendered', icon: Sun }
  ];
  
  const speedPresets = [
    { value: 1, label: 'Slow' },
    { value: 2, label: 'Normal' },
    { value: 5, label: 'Fast' },
    { value: 10, label: 'Very Fast' }
  ];

  const cameraTypes = [
    { id: 'universal', label: 'Fly Camera', icon: Move, description: 'WASD + QE flight controls' },
    { id: 'arcrotate', label: 'Orbit Camera', icon: Eye, description: 'Orbit around target' }
  ];

  const SliderControl = ({ label, getValue, min, max, step, onChange, unit = '', resetKey }) => {
    const displayValue = () => {
      const value = getValue();
      if (typeof value !== 'number') return value;
      if (step < 0.01) return value.toFixed(4);
      if (step < 0.1) return value.toFixed(2);
      if (step < 1) return value.toFixed(1);
      return value.toFixed(0);
    };
    
    const handleReset = () => {
      if (resetKey && defaults[resetKey] !== undefined) {
        onChange(defaults[resetKey]);
      }
    };
    
    return (
      <div>
        <div class="flex items-center justify-between mb-1">
          <label class="text-xs text-base-content/80">
            {label}: {displayValue()}{unit}
          </label>
          {resetKey && (
            <button
              onClick={handleReset}
              class="btn btn-xs btn-ghost opacity-60 hover:opacity-100 min-h-0 h-5 w-5 p-0"
              title={`Reset ${label}`}
            >
              ↺
            </button>
          )}
        </div>
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

  const ColorControl = ({ label, value, onChange, resetKey }) => {
    const handleReset = () => {
      if (resetKey && defaults[resetKey] !== undefined) {
        onChange(defaults[resetKey]);
      }
    };
    
    return (
      <div>
        <div class="flex items-center justify-between mb-1">
          <label class="text-xs text-base-content/80">{label}</label>
          {resetKey && (
            <button
              onClick={handleReset}
              class="btn btn-xs btn-ghost opacity-60 hover:opacity-100 min-h-0 h-5 w-5 p-0"
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
          class="w-full h-6 rounded border border-base-300"
        />
      </div>
    );
  };

  const ToggleControl = ({ label, value, onChange, resetKey }) => {
    const handleReset = () => {
      if (resetKey && defaults[resetKey] !== undefined) {
        onChange(defaults[resetKey]);
      }
    };
    
    return (
      <div class="flex items-center justify-between">
        <label class="text-xs text-base-content/80">{label}</label>
        <div class="flex items-center gap-1">
          {resetKey && (
            <button
              onClick={handleReset}
              class="btn btn-xs btn-ghost opacity-60 hover:opacity-100 min-h-0 h-5 w-5 p-0"
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
    <div class="h-full flex flex-col bg-base-200">
      {/* Header */}
      <div class="px-2 py-1 border-b border-base-300/50 bg-base-100/80 backdrop-blur-sm">
        <div class="flex items-center gap-2">
          <div class="p-1 bg-gradient-to-br from-primary/20 to-secondary/20 rounded border border-primary/30">
            <Camera class="w-3 h-3 text-primary" />
          </div>
          <div>
            <h2 class="text-xs font-medium text-base-content">Camera</h2>
          </div>
        </div>
      </div>

      {/* Content */}
      <div class="flex-1 overflow-y-auto p-0.5 space-y-0.5">
        
        {/* Camera Controls */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer" onClick={() => toggleSection('camera')}>
            <Camera class="w-3 h-3" />
            Camera Controls
          </div>
          <Show when={sectionsOpen().camera}>
            <div class="!p-2">
              <div class="space-y-0.5">
              {/* Camera Type */}
              <div>
                <label class="block text-xs font-medium text-base-content/80 mb-1">Camera Type</label>
                <div class="grid grid-cols-1 gap-1">
                  {cameraTypes.map((type) => (
                    <button
                      onClick={() => setCameraType(type.id)}
                      class={`btn btn-xs flex items-center gap-2 justify-start ${
                        cameraType() === type.id ? 'btn-primary' : 'btn-ghost'
                      }`}
                      title={type.description}
                    >
                      <type.icon class="w-3 h-3" />
                      <span>{type.label}</span>
                    </button>
                  ))}
                </div>
              </div>

              {/* Speed Presets */}
              <div>
                <label class="block text-xs font-medium text-base-content/80 mb-1">Speed Presets</label>
                <div class="grid grid-cols-2 gap-1">
                  {speedPresets.map((preset) => (
                    <button
                      onClick={() => setCameraSpeed(preset.value)}
                      class={`btn btn-xs ${
                        cameraSpeed() === preset.value ? 'btn-primary' : 'btn-ghost'
                      }`}
                    >
                      {preset.label}
                    </button>
                  ))}
                </div>
              </div>

              {/* Camera Settings */}
              <SliderControl 
                label="Speed" 
                getValue={() => cameraSpeed()} 
                min={0.1} 
                max={10} 
                step={0.1} 
                onChange={(v) => setCameraSpeed(v)}
                resetKey="speed"
              />
              
              <SliderControl 
                label="Mouse Sensitivity" 
                getValue={() => mouseSensitivity() * 1000} 
                min={1} 
                max={10} 
                step={0.1} 
                onChange={(v) => setCameraSensitivity(v / 1000)}
                resetKey="mouseSensitivity"
              />
              
              <SliderControl 
                label="Movement Momentum" 
                getValue={() => cameraFriction()} 
                min={1} 
                max={5} 
                step={1} 
                onChange={(v) => setCameraFriction(v)}
                resetKey="friction"
              />
              
              <SliderControl 
                label="Field of View" 
                getValue={() => cameraFOV()} 
                min={30} 
                max={120} 
                step={1} 
                onChange={(v) => setCameraFOV(v)}
                unit="°"
                resetKey="fov"
              />
              </div>
            </div>
          </Show>
        </div>
        
        {/* Render Mode */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer" onClick={() => toggleSection('render')}>
            <Palette class="w-3 h-3" />
            Render Mode
          </div>
          <Show when={sectionsOpen().render}>
            <div class="!p-2">
              <div class="space-y-0.5">
                <div class="grid grid-cols-2 gap-1">
                  {renderModes.map((mode) => (
                    <button
                      onClick={() => setRenderMode(mode.id)}
                      class={`btn btn-xs flex items-center gap-1 justify-start ${
                        renderMode() === mode.id ? 'btn-primary' : 'btn-ghost'
                      }`}
                      title={mode.label}
                    >
                      <mode.icon class="w-3 h-3" />
                      <span class="text-xs">{mode.label}</span>
                    </button>
                  ))}
                </div>
              </div>
            </div>
          </Show>
        </div>
        
        {/* Visual Effects */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer" onClick={() => toggleSection('effects')}>
            <Eye class="w-3 h-3" />
            Visual Effects
          </div>
          <Show when={sectionsOpen().effects}>
            <div class="!p-2">
              <div class="space-y-0.5">
                <ToggleControl 
                  label="Vignette" 
                  value={vignetteEnabled()} 
                  onChange={(v) => setVignetteEnabled(v)}
                  resetKey="vignetteEnabled"
                />
                
                {vignetteEnabled() && (
                  <div class="space-y-0.5">
                    <SliderControl 
                      label="Vignette Amount" 
                      getValue={() => vignetteAmount()} 
                      min={0} 
                      max={1} 
                      step={0.01} 
                      onChange={(v) => setVignetteAmount(v)}
                      resetKey="vignetteAmount"
                    />
                    
                    <ColorControl 
                      label="Vignette Color" 
                      value={vignetteColor()} 
                      onChange={(v) => setVignetteColor(v)}
                      resetKey="vignetteColor"
                    />
                  </div>
                )}
                
                <ColorControl 
                  label="Night Color Tint" 
                  value={nightColor()} 
                  onChange={(v) => setNightColor(v)}
                  resetKey="nightColor"
                />
              </div>
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
}