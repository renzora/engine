import { useState, useEffect } from 'react';
import { Icons } from '@/plugins/editor/components/Icons.jsx';
import SliderWithTooltip from '@/plugins/editor/components/ui/SliderWithTooltip.jsx';
import CollapsibleSection from '@/plugins/editor/components/ui/CollapsibleSection.jsx';
import Select from '@/plugins/ui/components/Select.jsx';
import { globalStore, actions } from "@/store.js";
import { useSnapshot } from 'valtio';

function Settings() {
  const settings = useSnapshot(globalStore.editor.settings);
  const { viewport: viewportSettings } = settings;
  const { updateViewportSettings } = actions.editor;
  const [webGPUSupported, setWebGPUSupported] = useState(false);

  // Check WebGPU support on component mount
  useEffect(() => {
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
  }, []);

  return (
    <div className="flex-1 overflow-y-auto scrollbar-thin">
      <div>
        <CollapsibleSection title="Viewport" defaultOpen={true} index={1}>
          <div className="space-y-4">
            {/* Rendering Engine */}
            <div className="space-y-2">
              <label className="text-xs font-medium text-gray-300 uppercase tracking-wide">Rendering Engine</label>
              <Select
                value={viewportSettings.renderingEngine || 'webgl'}
                onChange={(e) => {
                  const newEngine = e.target.value;
                  updateViewportSettings({ renderingEngine: newEngine });
                  
                  // Trigger viewport refresh with new rendering engine
                  actions.editor.addConsoleMessage(`Switching to ${newEngine.toUpperCase()} rendering...`, 'info');
                }}
                options={[
                  { value: 'webgl', label: 'WebGL' },
                  { 
                    value: 'webgpu', 
                    label: webGPUSupported ? 'WebGPU' : 'WebGPU (Unsupported)'
                  }
                ]}
                size="sm"
              />
              {!webGPUSupported && (
                <div className="text-xs text-yellow-400 mt-1">
                  ⚠️ WebGPU is not available in this browser/environment
                </div>
              )}
            </div>

            {/* Background Color */}
            <div className="space-y-2">
              <label className="text-xs font-medium text-gray-300 uppercase tracking-wide">Background Color</label>
              <div className="flex items-center gap-2">
                <input 
                  type="color" 
                  value={viewportSettings.backgroundColor} 
                  onChange={(e) => updateViewportSettings({ backgroundColor: e.target.value })}
                  className="w-10 h-10 rounded-lg border border-slate-600 bg-slate-800 cursor-pointer" 
                />
                <div className="flex-1 bg-slate-800/80 border border-slate-600 rounded-lg p-2">
                  <div className="text-xs text-gray-300">{viewportSettings.backgroundColor.toUpperCase()}</div>
                </div>
              </div>
            </div>

            {/* Quick Background Presets */}
            <div className="space-y-2">
              <label className="text-xs font-medium text-gray-300 uppercase tracking-wide">Quick Presets</label>
              <div className="grid grid-cols-4 gap-2">
                <button
                  onClick={() => updateViewportSettings({ backgroundColor: '#1a202c' })}
                  className="h-8 rounded-lg border border-slate-600 transition-all hover:scale-105 hover:border-blue-500"
                  style={{ backgroundColor: '#1a202c' }}
                  title="Dark Blue"
                />
                <button
                  onClick={() => updateViewportSettings({ backgroundColor: '#000000' })}
                  className="h-8 rounded-lg border border-slate-600 transition-all hover:scale-105 hover:border-blue-500"
                  style={{ backgroundColor: '#000000' }}
                  title="Black"
                />
                <button
                  onClick={() => updateViewportSettings({ backgroundColor: '#374151' })}
                  className="h-8 rounded-lg border border-slate-600 transition-all hover:scale-105 hover:border-blue-500"
                  style={{ backgroundColor: '#374151' }}
                  title="Gray"
                />
                <button
                  onClick={() => updateViewportSettings({ backgroundColor: '#ffffff' })}
                  className="h-8 rounded-lg border border-slate-600 transition-all hover:scale-105 hover:border-blue-500"
                  style={{ backgroundColor: '#ffffff' }}
                  title="White"
                />
              </div>
            </div>
          </div>
        </CollapsibleSection>

        <CollapsibleSection title="Interface" defaultOpen={false} index={2}>
          <div className="space-y-4">
            {/* Panel Position */}
            <div className="space-y-2">
              <label className="text-xs font-medium text-gray-300 uppercase tracking-wide">Panel Position</label>
              <Select
                value={settings.editor.panelPosition || 'right'}
                onChange={(e) => {
                  const newPosition = e.target.value;
                  actions.editor.updateEditorSettings({ panelPosition: newPosition });
                  actions.editor.addConsoleMessage(`Panel moved to ${newPosition} side`, 'info');
                }}
                options={[
                  { value: 'right', label: 'Right Side' },
                  { value: 'left', label: 'Left Side' }
                ]}
                size="sm"
              />
              <div className="text-xs text-gray-400 mt-1">
                Choose which side of the screen to display the properties panel
              </div>
            </div>
          </div>
        </CollapsibleSection>
        
        <CollapsibleSection title="Performance" defaultOpen={false} index={3}>
          <div className="space-y-4">
            {/* Stats.js Toggle */}
            <div className="flex items-center justify-between p-3 bg-slate-800/40 rounded-lg border border-slate-700/50">
              <div>
                <label className="text-xs font-medium text-gray-300">Performance Stats</label>
                <p className="text-xs text-gray-500 mt-0.5">Show FPS, memory usage, and render statistics</p>
              </div>
              <button
                onClick={() => {
                  const newValue = !settings.editor.showStats;
                  console.log('Settings: Stats toggle clicked, newValue:', newValue);
                  
                  actions.editor.updateEditorSettings({ showStats: newValue });
                  
                  actions.editor.addConsoleMessage(`Performance stats ${newValue ? 'enabled' : 'disabled'}`, 'success');
                }}
                className={`relative inline-flex h-6 w-11 items-center rounded-full transition-all duration-200 ${settings.editor.showStats ? 'bg-blue-500 shadow-lg shadow-blue-500/30' : 'bg-slate-600'}`}
              >
                <span
                  className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform duration-200 shadow-sm ${settings.editor.showStats ? 'translate-x-6' : 'translate-x-1'}`}
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