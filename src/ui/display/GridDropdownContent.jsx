import { editorStore, editorActions } from "@/layout/stores/EditorStore";

export default function GridDropdownContent() {
  const store = editorStore;
  const { setGridSnapping, updateGridSettings } = editorActions;
  
  const gridSettings = () => store.settings?.grid || { 
    enabled: false, 
    unit: 'meters', 
    size: 10, 
    spacing: 1, 
    color: '#4a5568',
    cellColor: '#4a5568',
    sectionColor: '#2d3748'
  };

  return (
    <div class="w-72 space-y-4 p-4">
      <div>
        <label class="block font-medium text-gray-300 mb-2">
          Grid Settings
        </label>
        <div class="space-y-3">
          <div class="flex items-center justify-between p-2 bg-gray-800/50 rounded border border-gray-700">
            <label class="text-xs font-medium text-gray-300">Enable Grid</label>
            <button
              onClick={() => updateGridSettings({ enabled: !gridSettings().enabled })}
              class={`relative inline-flex h-5 w-9 items-center rounded-full transition-all duration-200 ${
                gridSettings().enabled ? 'bg-blue-500 shadow-lg shadow-blue-500/30' : 'bg-gray-600'
              }`}
            >
              <span
                class={`inline-block h-3 w-3 transform rounded-full bg-white transition-transform duration-200 shadow-sm ${
                  gridSettings().enabled ? 'translate-x-5' : 'translate-x-1'
                }`}
              />
            </button>
          </div>

          {gridSettings().enabled && (
            <div class="space-y-3 pt-2 border-t border-gray-700">
              <div>
                <label class="block text-xs text-gray-400 mb-1">Units</label>
                <select
                  value={gridSettings().unit || 'meters'}
                  onChange={(e) => updateGridSettings({ unit: e.target.value })}
                  class="w-full bg-gray-800 border border-gray-600 text-white text-xs p-1.5 rounded focus:outline-none focus:ring-1 focus:ring-blue-500"
                >
                  <option value="meters">Meters (m)</option>
                  <option value="centimeters">Centimeters (cm)</option>
                  <option value="millimeters">Millimeters (mm)</option>
                  <option value="feet">Feet (ft)</option>
                  <option value="inches">Inches (in)</option>
                </select>
              </div>

              <div class="grid grid-cols-2 gap-2">
                {!gridSettings().infiniteGrid && (
                  <div>
                    <label class="block text-xs text-gray-400 mb-1">Size ({gridSettings().unit || 'meters'})</label>
                    <input
                      type="number"
                      step={gridSettings().unit === 'millimeters' ? "100" : gridSettings().unit === 'centimeters' ? "10" : gridSettings().unit === 'inches' ? "12" : "1"}
                      min={gridSettings().unit === 'millimeters' ? "1000" : gridSettings().unit === 'centimeters' ? "100" : "1"}
                      max={gridSettings().unit === 'millimeters' ? "50000" : gridSettings().unit === 'centimeters' ? "5000" : gridSettings().unit === 'inches' ? "600" : "100"}
                      value={gridSettings().size}
                      onChange={(e) => updateGridSettings({ size: parseInt(e.target.value) || (gridSettings().unit === 'millimeters' ? 20000 : gridSettings().unit === 'centimeters' ? 2000 : gridSettings().unit === 'inches' ? 240 : 20) })}
                      class="w-full bg-gray-800 border border-gray-600 text-white text-xs p-1.5 rounded focus:outline-none focus:ring-1 focus:ring-blue-500"
                    />
                  </div>
                )}
                <div>
                  <label class="block text-xs text-gray-400 mb-1">Cell Size ({gridSettings().unit || 'meters'})</label>
                  <input
                    type="number"
                    step={gridSettings().unit === 'millimeters' ? "10" : gridSettings().unit === 'centimeters' ? "1" : gridSettings().unit === 'inches' ? "1" : "0.1"}
                    min={gridSettings().unit === 'millimeters' ? "10" : gridSettings().unit === 'centimeters' ? "1" : gridSettings().unit === 'inches' ? "1" : "0.1"}
                    max={gridSettings().unit === 'millimeters' ? "5000" : gridSettings().unit === 'centimeters' ? "500" : gridSettings().unit === 'inches' ? "60" : "10"}
                    value={gridSettings().cellSize}
                    onChange={(e) => updateGridSettings({ cellSize: parseFloat(e.target.value) || (gridSettings().unit === 'millimeters' ? 1000 : gridSettings().unit === 'centimeters' ? 100 : gridSettings().unit === 'inches' ? 12 : 1) })}
                    class="w-full bg-gray-800 border border-gray-600 text-white text-xs p-1.5 rounded focus:outline-none focus:ring-1 focus:ring-blue-500"
                  />
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
