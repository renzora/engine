import { createSignal, Show } from 'solid-js';
import { editorStore, editorActions } from "@/layout/stores/EditorStore";
import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";
import { Grid3x3, Settings, Palette } from '@/ui/icons';

export default function GridPanel() {
  const store = editorStore;
  const { setGridSnapping, updateGridSettings } = editorActions;
  
  // Default values for reset functionality
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
  
  // Section collapse state
  const [sectionsOpen, setSectionsOpen] = createSignal({
    grid: true,
    snapping: true,
    appearance: false
  });
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };
  
  const gridSnapping = () => viewportStore.gridSnapping || false;
  const gridSettings = () => store.settings?.grid || defaults;

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
          class="w-full h-6 rounded border border-base-300"
        />
      </div>
    );
  };

  return (
    <div class="h-full flex flex-col bg-base-200">
      {/* Header */}
      <div class="px-2 py-1 border-b border-base-300/50 bg-base-100/80 backdrop-blur-sm">
        <div class="flex items-center gap-2">
          <div class="p-1 bg-gradient-to-br from-primary/20 to-secondary/20 rounded border border-primary/30">
            <Grid3x3 class="w-3 h-3 text-primary" />
          </div>
          <div>
            <h2 class="text-xs font-medium text-base-content">Grid</h2>
          </div>
        </div>
      </div>

      {/* Content */}
      <div class="flex-1 overflow-y-auto p-0.5 space-y-0.5">
        
        {/* Grid Settings */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer" onClick={() => toggleSection('grid')}>
            <Grid3x3 class="w-3 h-3" />
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
                      onChange={(e) => updateGridSettings({ unit: e.target.value })}
                      class="select select-xs w-full border border-base-300"
                    >
                      <option value="meters">Meters (m)</option>
                      <option value="centimeters">Centimeters (cm)</option>
                      <option value="millimeters">Millimeters (mm)</option>
                      <option value="feet">Feet (ft)</option>
                      <option value="inches">Inches (in)</option>
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
                  
                  <SliderControl 
                    label="Cell Size" 
                    getValue={() => gridSettings().cellSize} 
                    min={0.1} 
                    max={10} 
                    step={0.1} 
                    onChange={(v) => updateGridSettings({ cellSize: v })}
                    unit={` ${gridSettings().unit || 'm'}`}
                    resetKey="cellSize"
                  />
                  
                  {/* Position Controls */}
                  <div>
                    <label class="text-xs text-base-content/80 mb-1 block">Position</label>
                    <div class="grid grid-cols-3 gap-1">
                      {['X', 'Y', 'Z'].map((axis, index) => (
                        <div class="relative">
                          <span class="absolute left-0 top-0 bottom-0 w-4 flex items-center justify-center text-xs text-base-content/70 pointer-events-none font-medium bg-base-300 border border-base-300 rounded-l">
                            {axis}
                          </span>
                          <input
                            type="number"
                            step="0.1"
                            value={gridSettings().position[index]}
                            onChange={(e) => {
                              const newPos = [...gridSettings().position];
                              newPos[index] = parseFloat(e.target.value) || 0;
                              updateGridSettings({ position: newPos });
                            }}
                            class="w-full input input-xs pl-5 pr-1 text-center border border-base-300"
                          />
                        </div>
                      ))}
                    </div>
                  </div>
                </div>
              )}
              </div>
            </div>
          </Show>
        </div>
        
        {/* Snapping */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer" onClick={() => toggleSection('snapping')}>
            <Settings class="w-3 h-3" />
            Snapping
          </div>
          <Show when={sectionsOpen().snapping}>
            <div class="!p-2">
              <div class="space-y-0.5">
                <ToggleControl 
                  label="Grid Snapping" 
                  value={gridSnapping()} 
                  onChange={(v) => setGridSnapping(v)}
                  resetKey="gridSnapping"
                />
              </div>
            </div>
          </Show>
        </div>
        
        {/* Appearance */}
        <div class="bg-base-100 border-base-300 border rounded-lg">
          <div class="!min-h-0 !py-1 !px-2 flex items-center gap-1.5 font-medium text-xs border-b border-base-300/50 cursor-pointer" onClick={() => toggleSection('appearance')}>
            <Palette class="w-3 h-3" />
            Appearance
          </div>
          <Show when={sectionsOpen().appearance}>
            <div class="!p-2">
              <div class="space-y-0.5">
                <ColorControl 
                  label="Cell Color" 
                  value={gridSettings().cellColor} 
                  onChange={(v) => updateGridSettings({ cellColor: v })}
                  resetKey="cellColor"
                />
                
                <ColorControl 
                  label="Section Color" 
                  value={gridSettings().sectionColor} 
                  onChange={(v) => updateGridSettings({ sectionColor: v })}
                  resetKey="sectionColor"
                />
              </div>
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
}