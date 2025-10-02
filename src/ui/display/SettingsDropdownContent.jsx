import { createSignal, onMount, Show } from 'solid-js';
import { Select } from '@/ui';
import ThemeSwitcher from '@/ui/ThemeSwitcher';
import { editorStore, editorActions } from "@/layout/stores/EditorStore";
import { IconDeviceDesktop, IconSettings, IconPalette, IconUser, IconAdjustments } from '@tabler/icons-solidjs';

function SettingsDropdownContent() {
  const { settings } = editorStore;
  const { viewport: viewportSettings } = settings;
  const { updateViewportSettings } = editorActions;
  const [webGPUSupported, setWebGPUSupported] = createSignal(false);

  onMount(async () => {
    const checkWebGPUSupport = async () => {
      if (!navigator.gpu) {
        setWebGPUSupported(false);
        return;
      }
      
      try {
        const adapter = await navigator.gpu.requestAdapter({
          powerPreference: 'high-performance',
          forceFallbackAdapter: false
        });
        
        if (!adapter) {
          setWebGPUSupported(false);
          return;
        }
        
        const device = await adapter.requestDevice({
          requiredFeatures: [],
          requiredLimits: {}
        });
        
        if (!device) {
          setWebGPUSupported(false);
          return;
        }
        
        setWebGPUSupported(true);
      } catch (error) {
        console.warn('WebGPU adapter check failed:', error);
        setWebGPUSupported(false);
      }
    };
    
    checkWebGPUSupport();
  });

  // Section collapse state
  const [sectionsOpen, setSectionsOpen] = createSignal({
    viewport: true,
    interface: false,
    editor: false,
    world: false,
    performance: false
  });
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };

  return (
    <div class="w-80 max-h-96 overflow-y-auto p-3 space-y-3">
      {/* Viewport Section */}
      <div class="bg-base-100 border border-base-300 rounded-lg">
        <div 
          class={`px-3 py-2 flex items-center gap-2 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${
            sectionsOpen().viewport ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg'
          }`} 
          onClick={() => toggleSection('viewport')}
        >
          <IconDeviceDesktop class="w-3 h-3" />
          Viewport
        </div>
        <Show when={sectionsOpen().viewport}>
          <div class="p-3 space-y-3">
            <div class="space-y-1">
              <label class="text-xs font-medium text-base-content/80">Rendering Engine</label>
              <Select
                value={viewportSettings.renderingEngine || 'webgl'}
                onChange={(e) => {
                  const newEngine = e.target.value;
                  updateViewportSettings({ renderingEngine: newEngine });
                  editorActions.addConsoleMessage(`Switching to ${newEngine.toUpperCase()} rendering...`, 'info');
                }}
                options={[
                  { value: 'webgl', label: 'WebGL' },
                  { 
                    value: 'webgpu', 
                    label: webGPUSupported() ? 'WebGPU' : 'WebGPU (Unsupported)'
                  }
                ]}
                size="sm"
              />
              <Show when={!webGPUSupported()}>
                <div class="text-xs text-warning mt-1">
                  ⚠️ WebGPU is not available in this browser/environment
                </div>
              </Show>
            </div>

            <div class="space-y-1">
              <label class="text-xs font-medium text-base-content/80">Background Color</label>
              <div class="flex items-center gap-2">
                <input 
                  type="color" 
                  value={viewportSettings.backgroundColor === 'theme' ? '#1a202c' : viewportSettings.backgroundColor} 
                  onChange={(e) => updateViewportSettings({ backgroundColor: e.target.value })}
                  class="w-8 h-8 rounded border border-base-300 bg-base-200 cursor-pointer" 
                  disabled={viewportSettings.backgroundColor === 'theme'}
                />
                <div class="flex-1 bg-base-200/80 border border-base-300 rounded px-2 py-1">
                  <div class="text-xs text-base-content/80">
                    {viewportSettings.backgroundColor === 'theme' ? 'Theme' : viewportSettings.backgroundColor.toUpperCase()}
                  </div>
                </div>
              </div>
            </div>

            <div class="space-y-1">
              <label class="text-xs font-medium text-base-content/80">Quick Presets</label>
              <div class="grid grid-cols-4 gap-1">
                <button
                  onClick={() => updateViewportSettings({ backgroundColor: 'theme' })}
                  class="h-6 rounded border-2 border-dashed border-primary text-xs text-primary transition-all hover:scale-105 hover:bg-primary/10 flex items-center justify-center font-medium"
                  title="Use Current Theme"
                >
                  🎨
                </button>
                <button
                  onClick={() => updateViewportSettings({ backgroundColor: '#000000' })}
                  class="h-6 rounded border border-base-300 transition-all hover:scale-105 hover:border-primary"
                  style={{ 'background-color': '#000000' }}
                  title="Black"
                />
                <button
                  onClick={() => updateViewportSettings({ backgroundColor: '#374151' })}
                  class="h-6 rounded border border-base-300 transition-all hover:scale-105 hover:border-primary"
                  style={{ 'background-color': '#374151' }}
                  title="Gray"
                />
                <button
                  onClick={() => updateViewportSettings({ backgroundColor: '#ffffff' })}
                  class="h-6 rounded border border-base-300 transition-all hover:scale-105 hover:border-primary"
                  style={{ 'background-color': '#ffffff' }}
                  title="White"
                />
              </div>
            </div>
          </div>
        </Show>
      </div>

      {/* Interface Section */}
      <div class="bg-base-100 border border-base-300 rounded-lg">
        <div 
          class={`px-3 py-2 flex items-center gap-2 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${
            sectionsOpen().interface ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg'
          }`} 
          onClick={() => toggleSection('interface')}
        >
          <IconPalette class="w-3 h-3" />
          Interface
        </div>
        <Show when={sectionsOpen().interface}>
          <div class="p-3 space-y-3">
            <div class="space-y-1">
              <label class="text-xs font-medium text-base-content/80">Theme</label>
              <ThemeSwitcher />
            </div>
            
            <div class="space-y-1">
              <label class="text-xs font-medium text-base-content/80">Panel Position</label>
              <Select
                value={settings.editor.panelPosition || 'right'}
                onChange={(e) => {
                  const newPosition = e.target.value;
                  editorActions.updateEditorSettings({ panelPosition: newPosition });
                  editorActions.addConsoleMessage(`Panel moved to ${newPosition} side`, 'info');
                }}
                options={[
                  { value: 'right', label: 'Right Side' },
                  { value: 'left', label: 'Left Side' }
                ]}
                size="sm"
              />
            </div>
          </div>
        </Show>
      </div>

      {/* Performance Section */}
      <div class="bg-base-100 border border-base-300 rounded-lg">
        <div 
          class={`px-3 py-2 flex items-center gap-2 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${
            sectionsOpen().performance ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg'
          }`} 
          onClick={() => toggleSection('performance')}
        >
          <IconAdjustments class="w-3 h-3" />
          Performance
        </div>
        <Show when={sectionsOpen().performance}>
          <div class="p-3 space-y-2">
            <div class="flex items-center justify-between p-2 bg-base-200/40 rounded border border-base-300/50">
              <div>
                <label class="text-xs font-medium text-base-content/80">Performance Stats</label>
                <p class="text-xs text-base-content/60 mt-0.5">Show FPS and render stats</p>
              </div>
              <button
                onClick={() => {
                  const newValue = !settings.editor.showStats;
                  editorActions.updateEditorSettings({ showStats: newValue });
                  editorActions.addConsoleMessage(`Performance stats ${newValue ? 'enabled' : 'disabled'}`, 'success');
                }}
                class={`relative inline-flex h-5 w-9 items-center rounded-full transition-all duration-200 ${
                  settings.editor.showStats ? 'bg-primary shadow-lg shadow-primary/30' : 'bg-base-content/40'
                }`}
              >
                <span
                  class={`inline-block h-3 w-3 transform rounded-full bg-white transition-transform duration-200 shadow-sm ${
                    settings.editor.showStats ? 'translate-x-5' : 'translate-x-1'
                  }`}
                />
              </button>
            </div>

            <div class="flex items-center justify-between p-2 bg-base-200/40 rounded border border-base-300/50">
              <div>
                <label class="text-xs font-medium text-base-content/80">Pause Rendering</label>
                <p class="text-xs text-base-content/60 mt-0.5">Stop render loop for debugging</p>
              </div>
              <button
                onClick={() => {
                  const newValue = !settings.editor.renderPaused;
                  editorActions.updateEditorSettings({ renderPaused: newValue });
                  editorActions.addConsoleMessage(`Rendering ${newValue ? 'paused' : 'resumed'}`, 'info');
                }}
                class={`relative inline-flex h-5 w-9 items-center rounded-full transition-all duration-200 ${
                  settings.editor.renderPaused ? 'bg-error shadow-lg shadow-error/30' : 'bg-base-content/40'
                }`}
              >
                <span
                  class={`inline-block h-3 w-3 transform rounded-full bg-white transition-transform duration-200 shadow-sm ${
                    settings.editor.renderPaused ? 'translate-x-5' : 'translate-x-1'
                  }`}
                />
              </button>
            </div>
          </div>
        </Show>
      </div>

      {/* Editor Section */}
      <div class="bg-base-100 border border-base-300 rounded-lg">
        <div 
          class={`px-3 py-2 flex items-center gap-2 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${
            sectionsOpen().editor ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg'
          }`} 
          onClick={() => toggleSection('editor')}
        >
          <IconSettings class="w-3 h-3" />
          Editor
        </div>
        <Show when={sectionsOpen().editor}>
          <div class="p-3 space-y-2">
            <div class="space-y-1">
              <label class="text-xs font-medium text-base-content/80 flex items-center justify-between">
                Script Reload Delay
                <span class="badge badge-primary badge-outline font-mono text-xs">
                  {settings.editor.scriptReloadDebounceMs || 500}ms
                </span>
              </label>
              <div class="flex items-center gap-2">
                <input
                  type="range"
                  min={100}
                  max={3000}
                  step={50}
                  value={settings.editor.scriptReloadDebounceMs || 500}
                  onChange={(e) => {
                    const newDelay = parseInt(e.target.value);
                    editorActions.updateEditorSettings({ scriptReloadDebounceMs: newDelay });
                  }}
                  onMouseUp={(e) => {
                    const newDelay = parseInt(e.target.value);
                    editorActions.addConsoleMessage(`Script reload delay set to ${newDelay}ms`, 'info');
                  }}
                  class="range range-primary range-sm flex-1"
                />
              </div>
              <div class="flex justify-between text-xs text-base-content/60">
                <span>100ms (Fast)</span>
                <span>3000ms (Slow)</span>
              </div>
              <div class="text-xs text-base-content/60">
                How long to wait after you stop typing before reloading scripts
              </div>
            </div>
          </div>
        </Show>
      </div>

      {/* World Section */}
      <div class="bg-base-100 border border-base-300 rounded-lg">
        <div 
          class={`px-3 py-2 flex items-center gap-2 font-medium text-xs border-b border-base-300/50 cursor-pointer transition-colors ${
            sectionsOpen().world ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg'
          }`} 
          onClick={() => toggleSection('world')}
        >
          <IconUser class="w-3 h-3" />
          World
        </div>
        <Show when={sectionsOpen().world}>
          <div class="p-3 space-y-2">
            <div class="space-y-1">
              <label class="text-xs font-medium text-base-content/80">Unit Measurement</label>
              <Select
                value={settings.grid.unit || 'centimeters'}
                onChange={(e) => {
                  const newUnit = e.target.value;
                  editorActions.updateGridSettings({ unit: newUnit });
                  editorActions.addConsoleMessage(`Changed unit measurement to ${newUnit}`, 'info');
                }}
                options={[
                  { value: 'centimeters', label: 'Centimeters (cm)' },
                  { value: 'meters', label: 'Meters (m)' },
                  { value: 'inches', label: 'Inches (in)' },
                  { value: 'feet', label: 'Feet (ft)' }
                ]}
                size="sm"
              />
              <div class="text-xs text-base-content/60">
                Choose the unit of measurement for position and size values
              </div>
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
}

export default SettingsDropdownContent;