import { createSignal, Show, For } from 'solid-js';
import { editorStore, editorActions } from "@/layout/stores/EditorStore";
import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";
import { renderStore, renderActions } from '@/render/store.jsx';
import { IconGridDots, IconSettings, IconPalette, IconGrid3x3, IconX, IconTarget } from '@tabler/icons-solidjs';

// Global state for dropdown sections to persist across renders
const [globalSectionsOpen, setGlobalSectionsOpen] = createSignal({
  grid: true,
  gridSnapping: true,
  gizmoSnapping: true,
  appearance: true
});

// Global state for gizmo snapping to persist across renders
const [globalGizmoSnapEnabled, setGlobalGizmoSnapEnabled] = createSignal(true);
const [globalGizmoSnapAmount, setGlobalGizmoSnapAmount] = createSignal(1);

export default function GridSettingsDropdown() {
  const store = editorStore;
  const { updateGridSettings } = editorActions;
  const { setGridSnapping } = viewportActions;
  
  // Default values for grid settings
  const defaults = {
    enabled: false,
    unit: 'meters',
    size: 10,
    cellSize: 1,
    position: [0, 0, 0],
    cellColor: '#4a5568',
    sectionColor: '#2d3748',
    infiniteGrid: false,
    gridSnapping: false
  };

  // Unit-specific default cell sizes
  const unitDefaults = {
    'meters': 1,
    'centimeters': 100,
    'millimeters': 1000,
    'feet': 3,
    'inches': 12
  };
  
  // Use global state for sections to persist across dropdown renders
  const sectionsOpen = globalSectionsOpen;
  const setSectionsOpen = setGlobalSectionsOpen;
  
  // Use global state for gizmo snapping to persist across dropdown renders
  const gizmoSnapEnabled = globalGizmoSnapEnabled;
  const setGizmoSnapEnabled = setGlobalGizmoSnapEnabled;
  const gizmoSnapAmount = globalGizmoSnapAmount;
  const setGizmoSnapAmount = setGlobalGizmoSnapAmount;
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };
  
  const gridSnapping = () => viewportStore.gridSnapping || false;
  const gridSettings = () => store.settings?.grid || defaults;

  // Handle unit change with appropriate cell size default
  const handleUnitChange = (newUnit) => {
    const defaultCellSize = unitDefaults[newUnit] || 1;
    updateGridSettings({ 
      unit: newUnit,
      cellSize: defaultCellSize
    });
    console.log(`📏 Grid unit changed to ${newUnit} with default cell size ${defaultCellSize}`);
  };

  // Gizmo snap presets
  const snapPresets = [
    { value: 0.5, label: '0.5' },
    { value: 1, label: '1' },
    { value: 2, label: '2' },
    { value: 3, label: '3' },
    { value: 4, label: '4' },
    { value: 5, label: '5' }
  ];

  // Gizmo snapping handlers
  const handleGizmoSnapToggle = () => {
    const enabled = !gizmoSnapEnabled();
    setGizmoSnapEnabled(enabled);
    
    // Apply to gizmo manager if available
    const gizmoManager = renderStore.gizmoManager;
    if (gizmoManager) {
      // Enable/disable snapping on all gizmos
      if (gizmoManager.gizmos.positionGizmo) {
        gizmoManager.gizmos.positionGizmo.snapDistance = enabled ? gizmoSnapAmount() : 0;
      }
      if (gizmoManager.gizmos.rotationGizmo) {
        gizmoManager.gizmos.rotationGizmo.snapDistance = enabled ? Math.PI / 12 : 0; // 15 degrees
      }
      if (gizmoManager.gizmos.scaleGizmo) {
        gizmoManager.gizmos.scaleGizmo.snapDistance = enabled ? 0.1 : 0;
      }
    }
    
    console.log(`🎯 Gizmo snapping ${enabled ? 'enabled' : 'disabled'}`);
  };

  const handleGizmoSnapAmountChange = (amount) => {
    setGizmoSnapAmount(amount);
    
    // Apply to gizmo manager if snapping is enabled
    if (gizmoSnapEnabled()) {
      const gizmoManager = renderStore.gizmoManager;
      if (gizmoManager && gizmoManager.gizmos.positionGizmo) {
        gizmoManager.gizmos.positionGizmo.snapDistance = amount;
      }
    }
    
    console.log(`🎯 Gizmo snap amount set to ${amount}`);
  };

  // Control components
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
          value={value}
          onInput={(e) => onChange(e.target.value)}
          onChange={(e) => onChange(e.target.value)}
          class="w-full h-6 rounded border border-base-300"
        />
      </div>
    );
  };

  return (
    <div class="w-80 space-y-3 p-4 max-h-96 overflow-y-auto">
      
      {/* Grid Settings */}
      <div class="bg-base-100 border-base-300 border rounded-lg">
        <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().grid ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('grid')}>
          <IconGridDots class="w-3 h-3" />
          Grid Settings
        </div>
        <Show when={sectionsOpen().grid}>
          <div class="!p-2">
            <div class="space-y-0.5">
            <ToggleControl 
              label="Enable Grid" 
              value={gridSettings().enabled} 
              onChange={(v) => updateGridSettings({ enabled: v })}
              resetKey="enabled"
            />
            
            <ToggleControl 
              label="Infinite Grid" 
              value={gridSettings().infiniteGrid} 
              onChange={(v) => updateGridSettings({ infiniteGrid: v })}
              resetKey="infiniteGrid"
            />
            
            {gridSettings().enabled && (
              <div class="space-y-0.5">
                <div>
                  <label class="text-xs text-base-content/80 mb-1 block">Units</label>
                  <select
                    value={gridSettings().unit || 'meters'}
                    onChange={(e) => handleUnitChange(e.target.value)}
                    class="select select-xs w-full border border-base-300"
                  >
                    <option value="meters">Meters (m) - Default: 1</option>
                    <option value="centimeters">Centimeters (cm) - Default: 100</option>
                    <option value="millimeters">Millimeters (mm) - Default: 1000</option>
                    <option value="feet">Feet (ft) - Default: 3</option>
                    <option value="inches">Inches (in) - Default: 12</option>
                  </select>
                </div>
                
                {!gridSettings().infiniteGrid && (
                  <SliderControl 
                    label="Size" 
                    getValue={() => gridSettings().size} 
                    min={1} 
                    max={100} 
                    step={1} 
                    onChange={(v) => updateGridSettings({ size: v })}
                    unit={` ${gridSettings().unit || 'm'}`}
                    resetKey="size"
                  />
                )}
                
                {/* Cell Size with Slider and Manual Input */}
                <div>
                  <div class="flex items-center justify-between mb-1">
                    <label class="text-xs text-base-content/80">
                      Cell Size: {gridSettings().cellSize}{` ${gridSettings().unit || 'm'}`}
                    </label>
                    <button
                      onClick={() => {
                        const currentUnit = gridSettings().unit || 'meters';
                        const defaultCellSize = unitDefaults[currentUnit] || 1;
                        updateGridSettings({ cellSize: defaultCellSize });
                      }}
                      class="btn btn-xs btn-ghost opacity-60 hover:opacity-100 min-h-0 h-5 w-5 p-0"
                      title={`Reset Cell Size to ${unitDefaults[gridSettings().unit || 'meters'] || 1}`}
                    >
                      ↺
                    </button>
                  </div>
                  
                  <div class="space-y-2">
                    {/* Slider */}
                    <input
                      type="range"
                      min={0.01}
                      max={100}
                      step={0.01}
                      value={gridSettings().cellSize}
                      onInput={(e) => updateGridSettings({ cellSize: parseFloat(e.target.value) })}
                      class="range range-primary w-full range-xs"
                    />
                    
                    {/* Manual Input */}
                    <div class="flex items-center gap-2">
                      <label class="text-xs text-base-content/70 whitespace-nowrap">Manual:</label>
                      <input
                        type="number"
                        min="0.001"
                        max="1000"
                        step="0.001"
                        value={gridSettings().cellSize}
                        onInput={(e) => {
                          const value = parseFloat(e.target.value);
                          if (!isNaN(value) && value > 0) {
                            updateGridSettings({ cellSize: value });
                          }
                        }}
                        class="input input-xs flex-1 text-center border border-base-300"
                        placeholder="Enter value"
                      />
                      <span class="text-xs text-base-content/70">{gridSettings().unit || 'm'}</span>
                    </div>
                  </div>
                </div>
                
                {/* Position Controls */}
                <div>
                  <label class="text-xs text-base-content/80 mb-1 block">Position</label>
                  <div class="grid grid-cols-3 gap-1">
                    <For each={['X', 'Y', 'Z']}>
                      {(axis, index) => (
                        <div class="relative">
                          <span class="absolute left-0 top-0 bottom-0 w-4 flex items-center justify-center text-xs text-base-content/70 pointer-events-none font-medium bg-base-300 border border-base-300 rounded-l">
                            {axis}
                          </span>
                          <input
                            type="number"
                            step="0.1"
                            value={gridSettings().position[index()]}
                            onChange={(e) => {
                              const newPos = [...gridSettings().position];
                              newPos[index()] = parseFloat(e.target.value) || 0;
                              updateGridSettings({ position: newPos });
                            }}
                            class="w-full input input-xs pl-5 pr-1 text-center border border-base-300"
                          />
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              </div>
            )}
            </div>
          </div>
        </Show>
      </div>
      
      {/* Grid Snapping */}
      <div class="bg-base-100 border-base-300 border rounded-lg">
        <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().gridSnapping ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('gridSnapping')}>
          <IconSettings class="w-3 h-3" />
          Grid Snapping
        </div>
        <Show when={sectionsOpen().gridSnapping}>
          <div class="!p-2">
            <div class="space-y-0.5">
              <ToggleControl 
                label="Snap Objects to Grid" 
                value={gridSnapping()} 
                onChange={(v) => setGridSnapping(v)}
                resetKey="gridSnapping"
              />
              <div class="text-xs text-base-content/60 mt-1">
                Objects will snap to grid when placing or moving
              </div>
            </div>
          </div>
        </Show>
      </div>
      
      {/* Gizmo Snapping */}
      <div class="bg-base-100 border-base-300 border rounded-lg">
        <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().gizmoSnapping ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('gizmoSnapping')}>
          <IconTarget class="w-3 h-3" />
          Gizmo Snapping
        </div>
        <Show when={sectionsOpen().gizmoSnapping}>
          <div class="!p-2">
            <div class="space-y-2">
              <div class="flex items-center gap-2">
                <button
                  onClick={handleGizmoSnapToggle}
                  class={`btn btn-xs flex items-center gap-2 ${
                    gizmoSnapEnabled() ? 'btn-primary' : 'btn-ghost'
                  }`}
                >
                  {gizmoSnapEnabled() ? (
                    <IconGrid3x3 class="w-3 h-3" />
                  ) : (
                    <IconX class="w-3 h-3" />
                  )}
                  <span class="text-xs">{gizmoSnapEnabled() ? 'On' : 'Off'}</span>
                </button>
                <span class="text-xs text-base-content/80">Transform Gizmo Snapping</span>
              </div>
              
              <div class={`space-y-2 ${!gizmoSnapEnabled() ? 'opacity-50' : ''}`}>
                <div>
                  <label class="block text-xs text-base-content/80 mb-1">
                    Position Snap: {gizmoSnapAmount()}
                  </label>
                  
                  <div class="grid grid-cols-6 gap-1 mb-1">
                    <For each={snapPresets}>
                      {(preset) => (
                        <button
                          onClick={() => handleGizmoSnapAmountChange(preset.value)}
                          disabled={!gizmoSnapEnabled()}
                          class={`btn btn-xs ${
                            gizmoSnapAmount() === preset.value && gizmoSnapEnabled()
                              ? 'btn-primary'
                              : 'btn-ghost'
                          }`}
                        >
                          {preset.label}
                        </button>
                      )}
                    </For>
                  </div>
                  
                  <input
                    type="range"
                    min="0.5"
                    max="5"
                    step="0.5"
                    value={gizmoSnapAmount()}
                    onInput={(e) => handleGizmoSnapAmountChange(parseFloat(e.target.value))}
                    disabled={!gizmoSnapEnabled()}
                    class="range range-primary w-full range-xs"
                  />
                </div>
                
                <div class="text-xs text-base-content/60 space-y-1 pt-1">
                  <div>• Position: Snaps to {gizmoSnapAmount()} unit intervals</div>
                  <div>• Rotation: Snaps to 15° increments</div>
                  <div>• Scale: Snaps to 0.1 increments</div>
                </div>
              </div>
            </div>
          </div>
        </Show>
      </div>
      
      {/* Appearance */}
      <div class="bg-base-100 border-base-300 border rounded-lg">
        <div class={`!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${ sectionsOpen().appearance ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`} onClick={() => toggleSection('appearance')}>
          <IconPalette class="w-3 h-3" />
          Grid Appearance
        </div>
        <Show when={sectionsOpen().appearance}>
          <div class="!p-2">
            <div class="space-y-0.5">
              <ColorControl 
                label="Cell Color" 
                value={gridSettings().cellColor || defaults.cellColor} 
                onChange={(v) => updateGridSettings({ cellColor: v })}
                resetKey="cellColor"
              />
              
              <ColorControl 
                label="Section Color" 
                value={gridSettings().sectionColor || defaults.sectionColor} 
                onChange={(v) => updateGridSettings({ sectionColor: v })}
                resetKey="sectionColor"
              />
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
}