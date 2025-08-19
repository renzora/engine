import { createSignal, onMount, Show } from 'solid-js';
import { CollapsibleSection } from '@/ui';
import { Select } from '@/ui';
import ThemeSwitcher from '@/ui/ThemeSwitcher';
import { editorStore, editorActions } from "@/layout/stores/EditorStore";
import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";

function Settings() {
  const { settings } = editorStore;
  const { viewport: viewportSettings } = settings;
  const { updateViewportSettings } = editorActions;
  const [webGPUSupported, setWebGPUSupported] = createSignal(false);

  onMount(async () => {
    const checkWebGPUSupport = async () => {
      if (!navigator.gpu) {
        console.log('WebGPU check: navigator.gpu not available')
        setWebGPUSupported(false);
        return;
      }
      
      try {
        console.log('WebGPU check: Testing adapter...')
        const adapter = await navigator.gpu.requestAdapter({
          powerPreference: 'high-performance',
          forceFallbackAdapter: false
        });
        
        if (!adapter) {
          console.log('WebGPU check: No adapter available')
          setWebGPUSupported(false);
          return;
        }
        
        console.log('WebGPU check: Adapter found, testing device...')
        const device = await adapter.requestDevice({
          requiredFeatures: [],
          requiredLimits: {}
        });
        
        if (!device) {
          console.log('WebGPU check: Device creation failed')
          setWebGPUSupported(false);
          return;
        }
        
        console.log('WebGPU check: Device created successfully')
        console.log('WebGPU adapter info:', {
          vendor: adapter.info?.vendor || 'Unknown',
          architecture: adapter.info?.architecture || 'Unknown',
          device: adapter.info?.device || 'Unknown',
          description: adapter.info?.description || 'Unknown'
        })
        
        setWebGPUSupported(true);
      } catch (error) {
        console.warn('WebGPU adapter check failed:', error);
        setWebGPUSupported(false);
      }
    };
    
    checkWebGPUSupport();
  });

  return (
    <div class="flex-1 overflow-y-auto scrollbar-thin">
      <div>
        <CollapsibleSection title="Viewport" defaultOpen={true} index={1}>
          <div class="space-y-4 p-4">
            <div class="space-y-2">
              <label class="text-xs font-medium text-base-content/80 uppercase tracking-wide">Rendering Engine</label>
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

            <div class="space-y-2">
              <label class="text-xs font-medium text-base-content/80 uppercase tracking-wide">Background Color</label>
              <div class="flex items-center gap-2">
                <input 
                  type="color" 
                  value={viewportSettings.backgroundColor === 'theme' ? '#1a202c' : viewportSettings.backgroundColor} 
                  onChange={(e) => updateViewportSettings({ backgroundColor: e.target.value })}
                  class="w-10 h-10 rounded-lg border border-base-300 bg-base-200 cursor-pointer" 
                  disabled={viewportSettings.backgroundColor === 'theme'}
                />
                <div class="flex-1 bg-base-200/80 border border-base-300 rounded-lg p-2">
                  <div class="text-xs text-base-content/80">
                    {viewportSettings.backgroundColor === 'theme' ? 'Using Current Theme' : viewportSettings.backgroundColor.toUpperCase()}
                  </div>
                </div>
              </div>
            </div>

            <div class="space-y-2">
              <label class="text-xs font-medium text-base-content/80 uppercase tracking-wide">Quick Presets</label>
              <div class="grid grid-cols-4 gap-2">
                <button
                  onClick={() => updateViewportSettings({ backgroundColor: 'theme' })}
                  class="h-8 rounded-lg border-2 border-dashed border-primary text-xs text-primary transition-all hover:scale-105 hover:bg-primary/10 flex items-center justify-center font-medium"
                  title="Use Current Theme"
                >
                  🎨
                </button>
                <button
                  onClick={() => updateViewportSettings({ backgroundColor: '#000000' })}
                  class="h-8 rounded-lg border border-base-300 transition-all hover:scale-105 hover:border-primary"
                  style={{ 'background-color': '#000000' }}
                  title="Black"
                />
                <button
                  onClick={() => updateViewportSettings({ backgroundColor: '#374151' })}
                  class="h-8 rounded-lg border border-base-300 transition-all hover:scale-105 hover:border-primary"
                  style={{ 'background-color': '#374151' }}
                  title="Gray"
                />
                <button
                  onClick={() => updateViewportSettings({ backgroundColor: '#ffffff' })}
                  class="h-8 rounded-lg border border-base-300 transition-all hover:scale-105 hover:border-primary"
                  style={{ 'background-color': '#ffffff' }}
                  title="White"
                />
              </div>
            </div>
          </div>
        </CollapsibleSection>

        <CollapsibleSection title="Interface" defaultOpen={false} index={2}>
          <div class="space-y-4 p-4">
            <div class="space-y-2">
              <label class="text-xs font-medium text-base-content/80 uppercase tracking-wide">Theme</label>
              <ThemeSwitcher />
              <div class="text-xs text-base-content/60 mt-1">
                Choose your preferred visual theme for the editor interface
              </div>
            </div>
            
            <div class="space-y-2">
              <label class="text-xs font-medium text-base-content/80 uppercase tracking-wide">Panel Position</label>
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
              <div class="text-xs text-base-content/60 mt-1">
                Choose which side of the screen to display the properties panel
              </div>
            </div>
          </div>
        </CollapsibleSection>
        
        <CollapsibleSection title="Performance" defaultOpen={false} index={3}>
          <div class="space-y-4 p-4">
            <div class="flex items-center justify-between p-3 bg-base-200/40 rounded-lg border border-base-300/50">
              <div>
                <label class="text-xs font-medium text-base-content/80">Performance Stats</label>
                <p class="text-xs text-base-content/60 mt-0.5">Show FPS, memory usage, and render statistics</p>
              </div>
              <button
                onClick={() => {
                  const newValue = !settings.editor.showStats;
                  console.log('Settings: Stats toggle clicked, newValue:', newValue);
                  
                  editorActions.updateEditorSettings({ showStats: newValue });
                  
                  editorActions.addConsoleMessage(`Performance stats ${newValue ? 'enabled' : 'disabled'}`, 'success');
                }}
                class={`relative inline-flex h-6 w-11 items-center rounded-full transition-all duration-200 ${settings.editor.showStats ? 'bg-primary shadow-lg shadow-primary/30' : 'bg-base-content/40'}`}
              >
                <span
                  class={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform duration-200 shadow-sm ${settings.editor.showStats ? 'translate-x-6' : 'translate-x-1'}`}
                />
              </button>
            </div>
          </div>
        </CollapsibleSection>
      </div>
    </div>
  );
}

export default Settings;