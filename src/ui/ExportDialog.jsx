import { createSignal, Show } from 'solid-js';
import { exportManager } from '@/api/export/ExportManager.js';
import { getCurrentProject } from '@/api/bridge/projects';

/**
 * ExportDialog - UI for exporting projects
 */
export default function ExportDialog(props) {
  const [isExporting, setIsExporting] = createSignal(false);
  const [exportProgress, setExportProgress] = createSignal(0);
  const [exportStatus, setExportStatus] = createSignal('');
  const [exportResult, setExportResult] = createSignal(null);
  const [exportOptions, setExportOptions] = createSignal({
    buildTauri: true,
    includeAssets: true,
    optimizeBundle: true,
    outputFormat: 'both' // 'web', 'tauri', 'both'
  });

  // Subscribe to export progress
  exportManager.setProgressCallback((progress, status) => {
    setExportProgress(progress);
    setExportStatus(status);
  });

  const startExport = async () => {
    const currentProject = getCurrentProject();
    if (!currentProject) {
      alert('No project selected');
      return;
    }

    setIsExporting(true);
    setExportResult(null);
    setExportProgress(0);
    setExportStatus('Starting export...');

    try {
      const result = await exportManager.exportProject(currentProject.name, exportOptions());
      setExportResult(result);
      
      if (result.success) {
        setExportStatus('Export completed successfully!');
      } else {
        setExportStatus(`Export failed: ${result.error}`);
      }
    } catch (error) {
      setExportResult({ success: false, error: error.message });
      setExportStatus(`Export failed: ${error.message}`);
    } finally {
      setIsExporting(false);
    }
  };

  const cancelExport = () => {
    exportManager.cancelExport();
    setIsExporting(false);
    setExportResult(null);
  };

  return (
    <Show when={props.isOpen()}>
      <div class="fixed inset-0 bg-black bg-opacity-50 z-50 flex items-center justify-center">
        <div class="bg-base-100 rounded-lg shadow-xl w-[600px] max-h-[80vh] overflow-hidden">
          
          {/* Header */}
          <div class="p-4 border-b border-base-300 flex items-center justify-between">
            <h2 class="text-lg font-semibold">Export Project</h2>
            <button 
              class="btn btn-ghost btn-sm"
              onClick={props.onClose}
              disabled={isExporting()}
            >
              ✕
            </button>
          </div>

          {/* Content */}
          <div class="p-4 space-y-4">
            
            {/* Project Info */}
            <div class="bg-base-200 p-3 rounded">
              <div class="text-sm font-medium">Project: {getCurrentProject()?.name}</div>
              <div class="text-xs text-base-content opacity-70">
                Version: {getCurrentProject()?.version || '1.0.0'}
              </div>
            </div>

            {/* Export Options */}
            <Show when={!isExporting() && !exportResult()}>
              <div class="space-y-3">
                <h3 class="text-sm font-medium">Export Options</h3>
                
                <div class="space-y-2">
                  <label class="flex items-center gap-2 cursor-pointer">
                    <input 
                      type="checkbox" 
                      class="checkbox checkbox-sm" 
                      checked={exportOptions().buildTauri}
                      onChange={(e) => setExportOptions(prev => ({ ...prev, buildTauri: e.target.checked }))}
                    />
                    <span class="text-sm">Build Tauri desktop application</span>
                  </label>
                  
                  <label class="flex items-center gap-2 cursor-pointer">
                    <input 
                      type="checkbox" 
                      class="checkbox checkbox-sm" 
                      checked={exportOptions().includeAssets}
                      onChange={(e) => setExportOptions(prev => ({ ...prev, includeAssets: e.target.checked }))}
                    />
                    <span class="text-sm">Include all project assets</span>
                  </label>
                  
                  <label class="flex items-center gap-2 cursor-pointer">
                    <input 
                      type="checkbox" 
                      class="checkbox checkbox-sm" 
                      checked={exportOptions().optimizeBundle}
                      onChange={(e) => setExportOptions(prev => ({ ...prev, optimizeBundle: e.target.checked }))}
                    />
                    <span class="text-sm">Optimize bundle size</span>
                  </label>
                </div>

                <div class="space-y-1">
                  <label class="text-sm font-medium">Output Format</label>
                  <select 
                    class="select select-sm select-bordered w-full"
                    value={exportOptions().outputFormat}
                    onChange={(e) => setExportOptions(prev => ({ ...prev, outputFormat: e.target.value }))}
                  >
                    <option value="web">Web Runtime Only</option>
                    <option value="tauri">Tauri Desktop Only</option>
                    <option value="both">Both Web and Desktop</option>
                  </select>
                </div>
              </div>
            </Show>

            {/* Export Progress */}
            <Show when={isExporting()}>
              <div class="space-y-3">
                <div class="flex items-center justify-between">
                  <span class="text-sm font-medium">Exporting...</span>
                  <span class="text-sm text-base-content opacity-70">{exportProgress()}%</span>
                </div>
                
                <div class="w-full bg-base-300 rounded-full h-2">
                  <div 
                    class="bg-primary h-2 rounded-full transition-all duration-300"
                    style={`width: ${exportProgress()}%`}
                  />
                </div>
                
                <div class="text-sm text-base-content opacity-70">
                  {exportStatus()}
                </div>
              </div>
            </Show>

            {/* Export Result */}
            <Show when={exportResult()}>
              <div class="space-y-3">
                <Show when={exportResult().success}>
                  <div class="bg-success bg-opacity-10 border border-success border-opacity-20 p-3 rounded">
                    <div class="text-success font-medium text-sm mb-2">✅ Export Successful!</div>
                    <div class="text-sm space-y-1">
                      <div>Output: {exportResult().outputPath}</div>
                      <Show when={exportResult().summary}>
                        <div class="text-xs opacity-70 mt-2">
                          <div>Scripts: {exportResult().summary.stats.scripts}</div>
                          <div>Assets: {exportResult().summary.stats.assets}</div>
                          <div>Scenes: {exportResult().summary.stats.scenes}</div>
                        </div>
                      </Show>
                    </div>
                  </div>
                </Show>
                
                <Show when={!exportResult().success}>
                  <div class="bg-error bg-opacity-10 border border-error border-opacity-20 p-3 rounded">
                    <div class="text-error font-medium text-sm mb-2">❌ Export Failed</div>
                    <div class="text-sm text-error opacity-90">
                      {exportResult().error}
                    </div>
                  </div>
                </Show>
              </div>
            </Show>

          </div>

          {/* Footer */}
          <div class="p-4 border-t border-base-300 flex justify-end gap-2">
            <Show when={isExporting()}>
              <button class="btn btn-sm btn-warning" onClick={cancelExport}>
                Cancel Export
              </button>
            </Show>
            
            <Show when={!isExporting() && !exportResult()}>
              <button class="btn btn-sm btn-ghost" onClick={props.onClose}>
                Cancel
              </button>
              <button class="btn btn-sm btn-primary" onClick={startExport}>
                Start Export
              </button>
            </Show>
            
            <Show when={exportResult()}>
              <button class="btn btn-sm btn-ghost" onClick={props.onClose}>
                Close
              </button>
              <Show when={exportResult().success}>
                <button 
                  class="btn btn-sm btn-primary"
                  onClick={() => {
                    // TODO: Open export folder in file explorer
                    console.log('Opening export folder:', exportResult().outputPath);
                  }}
                >
                  Open Folder
                </button>
              </Show>
            </Show>
          </div>
          
        </div>
      </div>
    </Show>
  );
}