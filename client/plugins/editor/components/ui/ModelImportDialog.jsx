import React, { useState, useEffect } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';

const ModelImportDialog = ({ 
  isOpen, 
  onClose, 
  onImport, 
  modelName, 
  modelAnalysis = null 
}) => {
  const [importMode, setImportMode] = useState('smart');
  const [maxObjects, setMaxObjects] = useState(50);
  const [hierarchyDepth, setHierarchyDepth] = useState(3);
  const [importMaterials, setImportMaterials] = useState(true);
  const [importAnimations, setImportAnimations] = useState(true);
  const [importCameras, setImportCameras] = useState(false);
  const [importLights, setImportLights] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);

  const importModes = [
    {
      id: 'smart',
      name: 'Smart (Recommended)',
      description: 'Auto-group similar objects and limit hierarchy depth',
      icon: <Icons.Sparkles className="w-5 h-5" />
    },
    {
      id: 'simplified',
      name: 'Simplified',
      description: 'Combine objects by material type',
      icon: <Icons.Square2Stack className="w-5 h-5" />
    },
    {
      id: 'individual',
      name: 'Individual',
      description: 'Keep all objects separate (may create many objects)',
      icon: <Icons.QueueList className="w-5 h-5" />
    },
    {
      id: 'single',
      name: 'Single Mesh',
      description: 'Merge everything into one object',
      icon: <Icons.Cube className="w-5 h-5" />
    }
  ];

  const handleImport = () => {
    const importSettings = {
      mode: importMode,
      maxObjects,
      hierarchyDepth,
      importMaterials,
      importAnimations,
      importCameras,
      importLights
    };
    
    onImport(importSettings);
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50">
      <div className="bg-slate-800 rounded-lg shadow-2xl max-w-md w-full mx-4 max-h-[90vh] overflow-hidden">
        <div className="flex items-center justify-between p-4 border-b border-slate-700">
          <div className="flex items-center gap-3">
            <Icons.Cube3D className="w-5 h-5 text-blue-400" />
            <div>
              <h2 className="text-lg font-semibold text-white">Import Model</h2>
              <p className="text-sm text-gray-400">{modelName}</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 hover:bg-slate-700 rounded transition-colors"
          >
            <Icons.XMark className="w-4 h-4 text-gray-400" />
          </button>
        </div>

        <div className="p-4 space-y-4 max-h-[60vh] overflow-y-auto">
          {modelAnalysis && (
            <div className="bg-slate-900/50 rounded-lg p-3 text-sm">
              <div className="text-gray-300 mb-2">Model Analysis:</div>
              <div className="grid grid-cols-2 gap-2 text-xs">
                <div className="text-gray-400">Objects: <span className="text-white">{modelAnalysis.totalObjects}</span></div>
                <div className="text-gray-400">Materials: <span className="text-white">{modelAnalysis.totalMaterials}</span></div>
                <div className="text-gray-400">Meshes: <span className="text-white">{modelAnalysis.totalMeshes}</span></div>
                <div className="text-gray-400">Depth: <span className="text-white">{modelAnalysis.maxDepth} levels</span></div>
              </div>
              {modelAnalysis.totalObjects > 100 && (
                <div className="mt-2 text-xs text-amber-400 flex items-center gap-1">
                  <Icons.ExclamationTriangle className="w-3 h-3" />
                  Complex model detected - Smart mode recommended
                </div>
              )}
            </div>
          )}

          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2">Import Mode:</label>
            <div className="space-y-2">
              {importModes.map((mode) => (
                <button
                  key={mode.id}
                  onClick={() => setImportMode(mode.id)}
                  className={`w-full p-3 rounded-lg border text-left transition-colors ${
                    importMode === mode.id
                      ? 'border-blue-500 bg-blue-500/10 text-blue-300'
                      : 'border-slate-600 bg-slate-700/50 text-gray-300 hover:bg-slate-700'
                  }`}
                >
                  <div className="flex items-start gap-3">
                    <div className={`mt-0.5 ${importMode === mode.id ? 'text-blue-400' : 'text-gray-400'}`}>
                      {mode.icon}
                    </div>
                    <div>
                      <div className="font-medium">{mode.name}</div>
                      <div className="text-xs text-gray-400 mt-1">{mode.description}</div>
                    </div>
                  </div>
                </button>
              ))}
            </div>
          </div>

          {importMode === 'smart' && (
            <div className="space-y-3">
              <div>
                <label className="block text-sm font-medium text-gray-300 mb-2">
                  Hierarchy Depth: {hierarchyDepth}
                </label>
                <input
                  type="range"
                  min="1"
                  max="5"
                  value={hierarchyDepth}
                  onChange={(e) => setHierarchyDepth(parseInt(e.target.value))}
                  className="w-full h-2 bg-slate-600 rounded-lg appearance-none cursor-pointer"
                />
                <div className="flex justify-between text-xs text-gray-400 mt-1">
                  <span>Flat</span>
                  <span>Deep</span>
                </div>
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-300 mb-2">
                  Max Objects: {maxObjects}
                </label>
                <input
                  type="range"
                  min="10"
                  max="200"
                  value={maxObjects}
                  onChange={(e) => setMaxObjects(parseInt(e.target.value))}
                  className="w-full h-2 bg-slate-600 rounded-lg appearance-none cursor-pointer"
                />
                <div className="flex justify-between text-xs text-gray-400 mt-1">
                  <span>Simple</span>
                  <span>Detailed</span>
                </div>
              </div>
            </div>
          )}

          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2">Options:</label>
            <div className="space-y-2">
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={importMaterials}
                  onChange={(e) => setImportMaterials(e.target.checked)}
                  className="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500/50"
                />
                <span className="text-gray-300">Import Materials</span>
              </label>
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={importAnimations}
                  onChange={(e) => setImportAnimations(e.target.checked)}
                  className="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500/50"
                />
                <span className="text-gray-300">Import Animations</span>
              </label>
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={importCameras}
                  onChange={(e) => setImportCameras(e.target.checked)}
                  className="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500/50"
                />
                <span className="text-gray-300">Import Cameras</span>
              </label>
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={importLights}
                  onChange={(e) => setImportLights(e.target.checked)}
                  className="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500/50"
                />
                <span className="text-gray-300">Import Lights</span>
              </label>
            </div>
          </div>

          <button
            onClick={() => setShowAdvanced(!showAdvanced)}
            className="flex items-center gap-2 text-sm text-blue-400 hover:text-blue-300 transition-colors"
          >
            <Icons.ChevronRight className={`w-3 h-3 transition-transform ${showAdvanced ? 'rotate-90' : ''}`} />
            Advanced Options
          </button>

          {showAdvanced && (
            <div className="bg-slate-900/30 rounded-lg p-3 space-y-2">
              <div className="text-xs text-gray-400">
                Advanced settings for fine-tuning import behavior
              </div>
            </div>
          )}
        </div>

        <div className="flex items-center justify-end gap-3 p-4 border-t border-slate-700">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm text-gray-300 hover:text-white transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleImport}
            className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm rounded-lg transition-colors"
          >
            Import Model
          </button>
        </div>
      </div>
    </div>
  );
};

export default ModelImportDialog;