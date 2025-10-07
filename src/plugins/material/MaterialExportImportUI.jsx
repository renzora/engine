import { createSignal, createEffect, Show, For } from 'solid-js';
import { 
  IconDownload, 
  IconUpload, 
  IconFolder, 
  IconFile, 
  IconTrash,
  IconClock
} from '@tabler/icons-solidjs';
import MaterialExportImport from './MaterialExportImport';

function MaterialExportImportUI(props) {
  const [showExportDialog, setShowExportDialog] = createSignal(false);
  const [showImportDialog, setShowImportDialog] = createSignal(false);
  const [showLibraryDialog, setShowLibraryDialog] = createSignal(false);
  const [materialFiles, setMaterialFiles] = createSignal([]);
  const [exportSettings, setExportSettings] = createSignal({
    name: '',
    description: '',
    author: '',
    tags: '',
    filename: ''
  });
  const [importStatus, setImportStatus] = createSignal(null);
  const [exportStatus, setExportStatus] = createSignal(null);
  const [isLoading, setIsLoading] = createSignal(false);

  const exportImport = new MaterialExportImport();

  // Load material files when library dialog opens
  createEffect(async () => {
    if (showLibraryDialog()) {
      setIsLoading(true);
      try {
        const files = await exportImport.listMaterialFiles();
        setMaterialFiles(files);
      } catch (error) {
        console.error('Failed to load material files:', error);
        setMaterialFiles([]);
      }
      setIsLoading(false);
    }
  });

  // Auto-generate filename from material name
  createEffect(() => {
    const settings = exportSettings();
    if (settings.name && !settings.filename) {
      const cleanName = settings.name.replace(/[^a-zA-Z0-9-_]/g, '_').toLowerCase();
      setExportSettings(prev => ({
        ...prev,
        filename: cleanName
      }));
    }
  });

  const handleExport = async () => {
    const settings = exportSettings();
    
    if (!settings.name || !settings.filename) {
      setExportStatus({ type: 'error', message: 'Name and filename are required' });
      return;
    }

    setIsLoading(true);
    setExportStatus(null);

    try {
      const metadata = {
        name: settings.name,
        description: settings.description,
        author: settings.author,
        tags: settings.tags.split(',').map(tag => tag.trim()).filter(Boolean)
      };

      const result = await exportImport.exportToFile(
        props.currentMaterial,
        props.nodes || [],
        props.connections || [],
        settings.filename,
        metadata
      );

      if (result.success) {
        setExportStatus({ 
          type: 'success', 
          message: `Material exported to ${result.filename}` 
        });
        
        // Reset form
        setExportSettings({
          name: '',
          description: '',
          author: '',
          tags: '',
          filename: ''
        });
        
        setTimeout(() => setShowExportDialog(false), 2000);
      } else {
        setExportStatus({ 
          type: 'error', 
          message: result.error || 'Export failed' 
        });
      }
    } catch (error) {
      setExportStatus({ 
        type: 'error', 
        message: error.message 
      });
    }

    setIsLoading(false);
  };

  const handleImport = async (filePath) => {
    setIsLoading(true);
    setImportStatus(null);

    try {
      const result = await exportImport.importFromFile(filePath, props.scene);

      if (result.success) {
        // Notify parent component about successful import
        if (props.onImport) {
          props.onImport(result);
        }

        setImportStatus({ 
          type: 'success', 
          message: `Material "${result.metadata?.name || 'Unknown'}" imported successfully` 
        });

        // Show warnings for missing assets
        if (result.missingAssets && result.missingAssets.length > 0) {
          const assetNames = result.missingAssets.map(a => a?.name || 'Unknown asset').join(', ');
          setImportStatus({ 
            type: 'warning', 
            message: `Material imported but some assets are missing: ${assetNames}` 
          });
        }

        setTimeout(() => {
          setShowImportDialog(false);
          setShowLibraryDialog(false);
        }, 2000);
      } else {
        setImportStatus({ 
          type: 'error', 
          message: result.error || 'Import failed' 
        });
      }
    } catch (error) {
      setImportStatus({ 
        type: 'error', 
        message: error.message 
      });
    }

    setIsLoading(false);
  };

  const handleFileUpload = async (event) => {
    const file = event.target.files[0];
    if (!file) return;

    try {
      const text = await file.text();
      const result = await exportImport.importMaterial(text, props.scene);

      if (props.onImport) {
        props.onImport({
          success: true,
          ...result
        });
      }

      setImportStatus({ 
        type: 'success', 
        message: `Material "${result.metadata?.name || 'Unknown'}" imported from file` 
      });
    } catch (error) {
      setImportStatus({ 
        type: 'error', 
        message: error.message 
      });
    }
  };

  const deleteMaterial = async (filePath) => {
    if (!confirm('Are you sure you want to delete this material?')) return;

    try {
      // Would need bridge service delete functionality
      console.log('Delete material:', filePath);
      // Refresh the list
      const files = await exportImport.listMaterialFiles();
      setMaterialFiles(files);
    } catch (error) {
      console.error('Failed to delete material:', error);
    }
  };

  const formatFileSize = (bytes) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  };

  return (
    <div class="flex gap-2">
      {/* Export Button */}
      <button
        class="btn btn-sm btn-outline"
        onClick={() => setShowExportDialog(true)}
        disabled={!props.currentMaterial}
        title="Export current material"
      >
        <IconDownload class="w-4 h-4" />
        Export
      </button>

      {/* Import Button */}
      <button
        class="btn btn-sm btn-outline"
        onClick={() => setShowImportDialog(true)}
        title="Import material"
      >
        <IconUpload class="w-4 h-4" />
        Import
      </button>

      {/* Library Button */}
      <button
        class="btn btn-sm btn-outline"
        onClick={() => setShowLibraryDialog(true)}
        title="Browse material library"
      >
        <IconFolder class="w-4 h-4" />
        Library
      </button>

      {/* Export Dialog */}
      <Show when={showExportDialog()}>
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div class="bg-base-100 rounded-lg p-6 w-96 max-w-[90vw]">
            <h3 class="text-lg font-semibold mb-4">Export Material</h3>
            
            <div class="space-y-4">
              <div class="form-control">
                <label class="label">
                  <span class="label-text">Material Name *</span>
                </label>
                <input
                  type="text"
                  class="input input-bordered"
                  value={exportSettings().name}
                  onChange={(e) => setExportSettings(prev => ({ ...prev, name: e.target.value }))}
                  placeholder="My Material"
                />
              </div>

              <div class="form-control">
                <label class="label">
                  <span class="label-text">Description</span>
                </label>
                <textarea
                  class="textarea textarea-bordered"
                  value={exportSettings().description}
                  onChange={(e) => setExportSettings(prev => ({ ...prev, description: e.target.value }))}
                  placeholder="Optional description..."
                  rows={3}
                />
              </div>

              <div class="form-control">
                <label class="label">
                  <span class="label-text">Author</span>
                </label>
                <input
                  type="text"
                  class="input input-bordered"
                  value={exportSettings().author}
                  onChange={(e) => setExportSettings(prev => ({ ...prev, author: e.target.value }))}
                  placeholder="Your name"
                />
              </div>

              <div class="form-control">
                <label class="label">
                  <span class="label-text">Tags (comma separated)</span>
                </label>
                <input
                  type="text"
                  class="input input-bordered"
                  value={exportSettings().tags}
                  onChange={(e) => setExportSettings(prev => ({ ...prev, tags: e.target.value }))}
                  placeholder="metal, pbr, realistic"
                />
              </div>

              <div class="form-control">
                <label class="label">
                  <span class="label-text">Filename *</span>
                </label>
                <input
                  type="text"
                  class="input input-bordered"
                  value={exportSettings().filename}
                  onChange={(e) => setExportSettings(prev => ({ ...prev, filename: e.target.value }))}
                  placeholder="my_material"
                />
                <div class="label">
                  <span class="label-text-alt">.rmat extension will be added automatically</span>
                </div>
              </div>

              <Show when={exportStatus()}>
                <div class={`alert ${exportStatus().type === 'error' ? 'alert-error' : 'alert-success'}`}>
                  {exportStatus().message}
                </div>
              </Show>
            </div>

            <div class="flex justify-end gap-2 mt-6">
              <button
                class="btn btn-ghost"
                onClick={() => setShowExportDialog(false)}
              >
                Cancel
              </button>
              <button
                class="btn btn-primary"
                onClick={handleExport}
                disabled={isLoading() || !exportSettings().name || !exportSettings().filename}
              >
                {isLoading() ? 'Exporting...' : 'Export'}
              </button>
            </div>
          </div>
        </div>
      </Show>

      {/* Import Dialog */}
      <Show when={showImportDialog()}>
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div class="bg-base-100 rounded-lg p-6 w-96 max-w-[90vw]">
            <h3 class="text-lg font-semibold mb-4">Import Material</h3>
            
            <div class="space-y-4">
              <div class="form-control">
                <label class="label">
                  <span class="label-text">Import from file</span>
                </label>
                <input
                  type="file"
                  class="file-input file-input-bordered w-full"
                  accept=".rmat,.json"
                  onChange={handleFileUpload}
                />
                <div class="label">
                  <span class="label-text-alt">Supports .rmat and .json files</span>
                </div>
              </div>

              <div class="divider">OR</div>

              <button
                class="btn btn-outline w-full"
                onClick={() => {
                  setShowImportDialog(false);
                  setShowLibraryDialog(true);
                }}
              >
                <IconFolder class="w-4 h-4" />
                Browse Material Library
              </button>

              <Show when={importStatus()}>
                <div class={`alert ${
                  importStatus().type === 'error' ? 'alert-error' : 
                  importStatus().type === 'warning' ? 'alert-warning' : 'alert-success'
                }`}>
                  {importStatus().message}
                </div>
              </Show>
            </div>

            <div class="flex justify-end gap-2 mt-6">
              <button
                class="btn btn-ghost"
                onClick={() => setShowImportDialog(false)}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      </Show>

      {/* Library Dialog */}
      <Show when={showLibraryDialog()}>
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div class="bg-base-100 rounded-lg p-6 w-[600px] max-w-[90vw] max-h-[80vh]">
            <h3 class="text-lg font-semibold mb-4">Material Library</h3>
            
            <div class="overflow-y-auto max-h-[60vh]">
              <Show when={isLoading()}>
                <div class="flex justify-center p-4">
                  <span class="loading loading-spinner loading-md"></span>
                </div>
              </Show>

              <Show when={!isLoading() && materialFiles().length === 0}>
                <div class="text-center p-8 text-base-content/60">
                  <IconFolder class="w-12 h-12 mx-auto mb-2 opacity-40" />
                  <p>No materials found in library</p>
                  <p class="text-sm mt-1">Export some materials to build your library</p>
                </div>
              </Show>

              <Show when={!isLoading() && materialFiles().length > 0}>
                <div class="space-y-2">
                  <For each={materialFiles()}>
                    {(file) => (
                      <div class="border border-base-300 rounded-lg p-3 hover:bg-base-200/50 transition-colors">
                        <div class="flex items-start justify-between">
                          <div class="flex-1">
                            <div class="flex items-center gap-2 mb-1">
                              <IconFile class="w-4 h-4 text-primary" />
                              <span class="font-medium">{file?.name?.replace('.rmat', '') || 'Unknown file'}</span>
                            </div>
                            
                            <div class="text-sm text-base-content/60 space-y-1">
                              <div class="flex items-center gap-4">
                                <span class="flex items-center gap-1">
                                  <IconClock class="w-3 h-3" />
                                  {new Date(file.modified).toLocaleDateString()}
                                </span>
                                <span>{formatFileSize(file.size)}</span>
                              </div>
                            </div>
                          </div>
                          
                          <div class="flex gap-1">
                            <button
                              class="btn btn-xs btn-primary"
                              onClick={() => handleImport(file.path)}
                              disabled={isLoading()}
                            >
                              Import
                            </button>
                            <button
                              class="btn btn-xs btn-error btn-outline"
                              onClick={() => deleteMaterial(file.path)}
                              title="Delete material"
                            >
                              <IconTrash class="w-3 h-3" />
                            </button>
                          </div>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </Show>

              <Show when={importStatus()}>
                <div class={`alert mt-4 ${
                  importStatus().type === 'error' ? 'alert-error' : 
                  importStatus().type === 'warning' ? 'alert-warning' : 'alert-success'
                }`}>
                  {importStatus().message}
                </div>
              </Show>
            </div>

            <div class="flex justify-end gap-2 mt-6">
              <button
                class="btn btn-ghost"
                onClick={() => setShowLibraryDialog(false)}
              >
                Close
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}

export default MaterialExportImportUI;