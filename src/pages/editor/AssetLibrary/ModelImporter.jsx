import { createSignal, createEffect, Show, For } from 'solid-js';
import { IconX, IconChevronDown, IconChevronRight, IconUpload, IconSettings, IconDownload, IconFolderOpen, IconAlertTriangle, IconCheckCircle, IconCircle } from '@tabler/icons-solidjs';
import { bridgeService } from '@/plugins/core/bridge';
import { getCurrentProject } from '@/api/bridge/projects';
import { modelProcessor } from './ModelProcessor';

function ModelImporter({ isOpen, onClose, onImportComplete, context }) {
  const [selectedFiles, setSelectedFiles] = createSignal([]);
  const [isImporting, setIsImporting] = createSignal(false);
  const [importProgress, setImportProgress] = createSignal(0);
  const [expandedSections, setExpandedSections] = createSignal(new Set(['general', 'skeletalMeshes', 'animations', 'materials', 'advanced']));
  const [importSettings, setImportSettings] = createSignal({
    general: {
      importMode: 'separate', // 'separate' (Unreal-style) or 'combined'
      useSourceName: true,
      sceneNameSubFolder: false,
      assetTypeSubFolders: true,
      offsetTranslation: { x: 0, y: 0, z: 0 },
      offsetRotation: { x: 0, y: 0, z: 0 },
      offsetUniformScale: 1.0,
      forceAllMeshType: 'none',
      autoDetectMeshType: true,
      importLods: true,
      bakeMeshes: false,
      bakePivotMeshes: false,
      keepSectionsSeparate: true,
      vertexColorImport: 'replace',
      vertexOverrideColor: '#ffffff',
      importSockets: false,
      build: true
    },
    skeletalMeshes: {
      importSkeletalMeshes: true,
      importContentType: 'geometry_and_skin_weights',
      importMorphTargets: true,
      mergeMorphTargetsWithSameName: false,
      importVertexAttributes: true,
      updateSkeletonReferencePose: false,
      createPhysicsAsset: false,
      importMeshesInBoneHierarchy: false,
      addCurveMetadataToSkeleton: false,
      convertStaticWithMorphToSkeletal: false
    },
    staticMeshes: {
      importStaticMeshes: true,
      combineStaticMeshes: false,
      lodGroup: 'none',
      autoComputeLodScreenSizes: true,
      generateCollision: false
    },
    animations: {
      importAnimations: true,
      importBoneTracks: true,
      animationLength: 'source_timeline',
      frameImportRange: { start: 0, end: -1 },
      use30HzToBakeBoneAnimation: false,
      customBoneAnimationSampleRate: 30,
      snapToClosestFrameBoundary: false,
      importCurves: true,
      animationOnly: false,
      skeleton: 'create_new'
    },
    materials: {
      importTextures: true,
      detectNormalMapTexture: true,
      flipNormalMapTexture: false,
      flipNormalMapGreenChannel: false,
      importUDIMs: false,
      importSparsVolumeTextures: false,
      importAnimatedSparseVolumeTextures: false,
      fileExtensionsForLongLatCubemap: 'hdr,exr',
      preferCompressedSourceData: true,
      allowNonPowerOfTwo: true,
      dracoCompression: true,
      tmfEncoding: false
    },
    advanced: {
      fileUnits: 'meters',
      fileAxisDirection: 'y_up',
      useSettingsForSubsequentFiles: true
    }
  });

  let fileInputRef;

  const toggleSection = (sectionName) => {
    console.log('Toggling section:', sectionName);
    const current = expandedSections();
    console.log('Current expanded sections:', current);
    const newSet = new Set(current);
    if (newSet.has(sectionName)) {
      newSet.delete(sectionName);
      console.log('Collapsed section:', sectionName);
    } else {
      newSet.add(sectionName);
      console.log('Expanded section:', sectionName);
    }
    setExpandedSections(newSet);
    console.log('New expanded sections:', newSet);
  };

  const updateSetting = (category, key, value) => {
    setImportSettings(prev => ({
      ...prev,
      [category]: {
        ...prev[category],
        [key]: value
      }
    }));
  };

  const handleFileSelect = () => {
    fileInputRef?.click();
  };

  const handleFileInputChange = (e) => {
    const files = Array.from(e.target.files);
    setSelectedFiles(files);
    e.target.value = '';
  };

  const handleDrop = (e) => {
    e.preventDefault();
    const files = Array.from(e.dataTransfer.files);
    const supportedFiles = files.filter(file => {
      const ext = file.name.toLowerCase().match(/\.[^.]+$/)?.[0];
      return ['.fbx', '.obj', '.gltf', '.glb', '.dae', '.3ds', '.blend', '.max', '.stl', '.ply', '.x3d', '.md2', '.md3', '.md5', '.lwo', '.ac', '.ms3d', '.cob', '.ifc', '.xgl', '.csm', '.bvh', '.b3d', '.ndo', '.dxf'].includes(ext);
    });
    setSelectedFiles(supportedFiles);
  };

  const handleDragOver = (e) => {
    e.preventDefault();
  };

  const processFiles = async () => {
    const currentProject = getCurrentProject();
    if (!currentProject?.name) {
      console.error('No project loaded');
      return;
    }

    setIsImporting(true);
    setImportProgress(0);

    try {
      const settings = importSettings();
      const totalFiles = selectedFiles().length;
      
      for (let i = 0; i < totalFiles; i++) {
        const file = selectedFiles()[i];
        const fileProgress = (i / totalFiles) * 100;
        
        // Use advanced model processor for supported 3D formats
        const ext = file.name.toLowerCase().match(/\.[^.]+$/)?.[0] || '';
        const is3DModel = ['.fbx', '.obj', '.gltf', '.glb', '.dae', '.3ds', '.blend', '.max', '.stl', '.ply', '.x3d', '.md2', '.md3', '.md5', '.lwo', '.ac', '.ms3d', '.cob', '.ifc', '.xgl', '.csm', '.bvh', '.b3d', '.ndo', '.dxf'].includes(ext);
        
        if (is3DModel) {
          // Use GLB conversion and extraction with current path context
          const currentPath = context()?.currentPath || '';
          await modelProcessor.convertToGlbAndExtract(
            file,
            settings,
            currentProject.name,
            settings.general.importMode,
            currentPath,
            (progress) => {
              const overallProgress = fileProgress + (progress.progress / totalFiles);
              setImportProgress(overallProgress);
            }
          );
        } else {
          // Simple file upload for non-3D assets
          setImportProgress(fileProgress + 50);
          
          const fileName = file.name;
          const fileNameWithoutExt = fileName.replace(/\.[^/.]+$/, "");
          
          // Use context current path exactly, or project root if not specified
          const currentPath = context()?.currentPath || '';
          const targetPath = currentPath 
            ? `projects/${currentProject.name}/${currentPath}/${fileName}`
            : `projects/${currentProject.name}/${fileName}`;
          
          const reader = new FileReader();
          const base64 = await new Promise((resolve, reject) => {
            reader.onload = () => resolve(reader.result.split(',')[1]);
            reader.onerror = reject;
            reader.readAsDataURL(file);
          });
          
          await bridgeService.writeBinaryFile(targetPath, base64);
          setImportProgress(fileProgress + 100);
        }
      }
      
      setImportProgress(100);
      
      // Notify completion
      if (onImportComplete) {
        onImportComplete();
      }
      
      // Close dialog after short delay
      setTimeout(() => {
        onClose();
        setSelectedFiles([]);
        setIsImporting(false);
        setImportProgress(0);
      }, 1000);
      
    } catch (error) {
      console.error('Import failed:', error);
      setIsImporting(false);
    } finally {
      modelProcessor.dispose();
    }
  };

  return (
    <Show when={isOpen()}>
      <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
        <div class="bg-base-200 rounded-xl shadow-2xl w-[80vw] h-[90vh] max-w-[900px] flex flex-col">
          {/* Header */}
          <div class="flex items-center justify-between p-4 border-b border-base-300">
            <h2 class="text-lg font-semibold text-base-content">Model Importer</h2>
            <button
              onClick={onClose}
              class="p-1 hover:bg-base-300 rounded transition-colors"
            >
              <IconX class="w-5 h-5" />
            </button>
          </div>

          <div class="flex flex-1 overflow-hidden">
            {/* Left Panel - File Selection */}
            <div class="w-1/3 p-4 border-r border-base-300 flex flex-col">
              <h3 class="text-sm font-medium text-base-content mb-3">Files to Import</h3>
              
              <div
                class="flex-1 border-2 border-dashed border-base-300 rounded-lg p-4 flex flex-col hover:border-primary/50 transition-colors cursor-pointer overflow-hidden"
                onClick={handleFileSelect}
                onDrop={handleDrop}
                onDragOver={handleDragOver}
              >
                <Show when={selectedFiles().length === 0}>
                  <div class="flex-1 flex flex-col items-center justify-center text-center">
                    <IconFolderOpen class="w-12 h-12 text-base-content/40 mb-3" />
                    <p class="text-sm text-base-content/60 mb-2">
                      Drop files here or click to browse
                    </p>
                    <p class="text-xs text-base-content/40">
                      Supports: FBX, OBJ, GLTF, GLB, DAE, 3DS, Blend, Max, STL, PLY, X3D, MD2/3/5, LWO, AC, IFC, and more
                    </p>
                  </div>
                </Show>
                
                <Show when={selectedFiles().length > 0}>
                  <div class="flex flex-col h-full text-left">
                    <p class="text-sm font-medium text-base-content mb-2 flex-shrink-0">
                      {selectedFiles().length} file(s) selected:
                    </p>
                    <div class="flex-1 space-y-1 overflow-y-auto min-h-0">
                      <For each={selectedFiles()}>
                        {(file) => (
                          <div class="flex items-center justify-between text-xs bg-base-100 p-2 rounded">
                            <span class="truncate">{file.name}</span>
                            <span class="text-base-content/50 ml-2">
                              {(() => {
                                const sizeInMB = file.size / 1024 / 1024;
                                if (sizeInMB < 1) {
                                  return `${Math.round(file.size / 1024)}KB`;
                                } else {
                                  return `${sizeInMB.toFixed(1)}MB`;
                                }
                              })()}
                            </span>
                          </div>
                        )}
                      </For>
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        setSelectedFiles([]);
                      }}
                      class="text-xs text-error hover:underline mt-2 flex-shrink-0"
                    >
                      Clear selection
                    </button>
                  </div>
                </Show>
              </div>

              {/* Import Actions */}
              <div class="mt-4 space-y-2">
                <button
                  onClick={processFiles}
                  disabled={selectedFiles().length === 0 || isImporting()}
                  class="w-full bg-primary text-primary-content px-4 py-2 rounded hover:bg-primary/80 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center justify-center gap-2"
                >
                  <Show when={isImporting()} fallback={
                    <>
                      <IconUpload class="w-4 h-4" />
                      Import Selected
                    </>
                  }>
                    <div class="w-4 h-4 border-2 border-primary-content border-t-transparent rounded-full animate-spin" />
                    Importing... {Math.round(importProgress())}%
                  </Show>
                </button>
                
                <Show when={isImporting()}>
                  <div class="w-full bg-base-300 rounded-full h-2">
                    <div 
                      class="bg-primary h-2 rounded-full transition-all duration-300"
                      style={{ width: `${importProgress()}%` }}
                    />
                  </div>
                </Show>
              </div>
            </div>

            {/* Right Panel - Import Settings */}
            <div class="flex-1 p-4 overflow-y-auto">
              <div class="space-y-3">
                {/* General Settings */}
                <ImportSection
                  title="General"
                  isExpanded={() => expandedSections().has('general')}
                  onToggle={() => toggleSection('general')}
                >
                  <div class="space-y-3">
                    <SelectSetting
                      label="Import Mode"
                      value={importSettings().general.importMode}
                      options={[
                        { value: 'separate', label: 'Separate Assets' },
                        { value: 'combined', label: 'Single Mesh' }
                      ]}
                      onChange={(value) => updateSetting('general', 'importMode', value)}
                    />
                    
                    <CheckboxSetting
                      label="Use source name for asset"
                      checked={importSettings().general.useSourceName}
                      onChange={(checked) => updateSetting('general', 'useSourceName', checked)}
                    />
                    <CheckboxSetting
                      label="Scene name sub folder"
                      checked={importSettings().general.sceneNameSubFolder}
                      onChange={(checked) => updateSetting('general', 'sceneNameSubFolder', checked)}
                    />
                    <CheckboxSetting
                      label="Asset type sub folders"
                      checked={importSettings().general.assetTypeSubFolders}
                      onChange={(checked) => updateSetting('general', 'assetTypeSubFolders', checked)}
                    />
                    
                    <div class="space-y-2">
                      <label class="text-xs font-medium text-base-content">Offset Translation</label>
                      <div class="grid grid-cols-3 gap-2">
                        <NumberInput
                          placeholder="X"
                          value={importSettings().general.offsetTranslation.x}
                          onChange={(value) => updateSetting('general', 'offsetTranslation', { ...importSettings().general.offsetTranslation, x: value })}
                        />
                        <NumberInput
                          placeholder="Y"
                          value={importSettings().general.offsetTranslation.y}
                          onChange={(value) => updateSetting('general', 'offsetTranslation', { ...importSettings().general.offsetTranslation, y: value })}
                        />
                        <NumberInput
                          placeholder="Z"
                          value={importSettings().general.offsetTranslation.z}
                          onChange={(value) => updateSetting('general', 'offsetTranslation', { ...importSettings().general.offsetTranslation, z: value })}
                        />
                      </div>
                    </div>

                    <div class="space-y-2">
                      <label class="text-xs font-medium text-base-content">Offset Rotation</label>
                      <div class="grid grid-cols-3 gap-2">
                        <NumberInput
                          placeholder="X"
                          value={importSettings().general.offsetRotation.x}
                          onChange={(value) => updateSetting('general', 'offsetRotation', { ...importSettings().general.offsetRotation, x: value })}
                        />
                        <NumberInput
                          placeholder="Y"
                          value={importSettings().general.offsetRotation.y}
                          onChange={(value) => updateSetting('general', 'offsetRotation', { ...importSettings().general.offsetRotation, y: value })}
                        />
                        <NumberInput
                          placeholder="Z"
                          value={importSettings().general.offsetRotation.z}
                          onChange={(value) => updateSetting('general', 'offsetRotation', { ...importSettings().general.offsetRotation, z: value })}
                        />
                      </div>
                    </div>

                    <NumberSetting
                      label="Offset uniform scale"
                      value={importSettings().general.offsetUniformScale}
                      onChange={(value) => updateSetting('general', 'offsetUniformScale', value)}
                      min={0.01}
                      max={100}
                      step={0.1}
                    />

                    <SelectSetting
                      label="Force all mesh as type"
                      value={importSettings().general.forceAllMeshType}
                      options={[
                        { value: 'none', label: 'None' },
                        { value: 'static_mesh', label: 'Static Mesh' },
                        { value: 'skeletal_mesh', label: 'Skeletal Mesh' }
                      ]}
                      onChange={(value) => updateSetting('general', 'forceAllMeshType', value)}
                    />

                    <CheckboxSetting
                      label="Auto detect mesh type"
                      checked={importSettings().general.autoDetectMeshType}
                      onChange={(checked) => updateSetting('general', 'autoDetectMeshType', checked)}
                    />
                  </div>
                </ImportSection>

                {/* Skeletal Meshes */}
                <ImportSection
                  title="Skeletal Meshes"
                  isExpanded={() => expandedSections().has('skeletalMeshes')}
                  onToggle={() => toggleSection('skeletalMeshes')}
                >
                  <div class="space-y-3">
                    <CheckboxSetting
                      label="Import skeletal meshes"
                      checked={importSettings().skeletalMeshes.importSkeletalMeshes}
                      onChange={(checked) => updateSetting('skeletalMeshes', 'importSkeletalMeshes', checked)}
                    />
                    
                    <SelectSetting
                      label="Import content type"
                      value={importSettings().skeletalMeshes.importContentType}
                      options={[
                        { value: 'geometry_and_skin_weights', label: 'Geometry and Skin Weights' },
                        { value: 'geometry_only', label: 'Geometry Only' },
                        { value: 'skin_weights_only', label: 'Skin Weights Only' }
                      ]}
                      onChange={(value) => updateSetting('skeletalMeshes', 'importContentType', value)}
                    />

                    <CheckboxSetting
                      label="Import morph targets"
                      checked={importSettings().skeletalMeshes.importMorphTargets}
                      onChange={(checked) => updateSetting('skeletalMeshes', 'importMorphTargets', checked)}
                    />

                    <CheckboxSetting
                      label="Create physics asset"
                      checked={importSettings().skeletalMeshes.createPhysicsAsset}
                      onChange={(checked) => updateSetting('skeletalMeshes', 'createPhysicsAsset', checked)}
                    />
                  </div>
                </ImportSection>

                {/* Animations */}
                <ImportSection
                  title="Animations"
                  isExpanded={() => expandedSections().has('animations')}
                  onToggle={() => toggleSection('animations')}
                >
                  <div class="space-y-3">
                    <CheckboxSetting
                      label="Import animations"
                      checked={importSettings().animations.importAnimations}
                      onChange={(checked) => updateSetting('animations', 'importAnimations', checked)}
                    />

                    <SelectSetting
                      label="Animation length"
                      value={importSettings().animations.animationLength}
                      options={[
                        { value: 'source_timeline', label: 'Source Timeline' },
                        { value: 'animated_range', label: 'Animated Range' },
                        { value: 'set_range', label: 'Set Range' }
                      ]}
                      onChange={(value) => updateSetting('animations', 'animationLength', value)}
                    />

                    <NumberSetting
                      label="Custom bone animation sample rate"
                      value={importSettings().animations.customBoneAnimationSampleRate}
                      onChange={(value) => updateSetting('animations', 'customBoneAnimationSampleRate', value)}
                      min={1}
                      max={120}
                    />

                    <CheckboxSetting
                      label="Import only animations"
                      checked={importSettings().animations.animationOnly}
                      onChange={(checked) => updateSetting('animations', 'animationOnly', checked)}
                    />
                  </div>
                </ImportSection>

                {/* Materials & Textures */}
                <ImportSection
                  title="Materials & Textures"
                  isExpanded={() => expandedSections().has('materials')}
                  onToggle={() => toggleSection('materials')}
                >
                  <div class="space-y-3">
                    <CheckboxSetting
                      label="Import textures"
                      checked={importSettings().materials.importTextures}
                      onChange={(checked) => updateSetting('materials', 'importTextures', checked)}
                    />

                    <CheckboxSetting
                      label="Detect normal map texture"
                      checked={importSettings().materials.detectNormalMapTexture}
                      onChange={(checked) => updateSetting('materials', 'detectNormalMapTexture', checked)}
                    />

                    <CheckboxSetting
                      label="Draco compression"
                      description="Compress 3D mesh geometry for smaller file sizes (recommended for large models)"
                      checked={importSettings().materials.dracoCompression}
                      onChange={(checked) => updateSetting('materials', 'dracoCompression', checked)}
                    />

                    <CheckboxSetting
                      label="TMF compression"
                      description="Apply TMF compression to 3D models for optimized file size and faster loading"
                      checked={importSettings().materials.tmfEncoding}
                      onChange={(checked) => updateSetting('materials', 'tmfEncoding', checked)}
                    />

                    <CheckboxSetting
                      label="Allow non power of two"
                      checked={importSettings().materials.allowNonPowerOfTwo}
                      onChange={(checked) => updateSetting('materials', 'allowNonPowerOfTwo', checked)}
                    />
                  </div>
                </ImportSection>

                {/* Advanced */}
                <ImportSection
                  title="Advanced"
                  isExpanded={() => expandedSections().has('advanced')}
                  onToggle={() => toggleSection('advanced')}
                >
                  <div class="space-y-3">
                    <SelectSetting
                      label="File units"
                      value={importSettings().advanced.fileUnits}
                      options={[
                        { value: 'meters', label: 'Meters' },
                        { value: 'centimeters', label: 'Centimeters' },
                        { value: 'inches', label: 'Inches' },
                        { value: 'feet', label: 'Feet' }
                      ]}
                      onChange={(value) => updateSetting('advanced', 'fileUnits', value)}
                    />

                    <SelectSetting
                      label="File axis direction"
                      value={importSettings().advanced.fileAxisDirection}
                      options={[
                        { value: 'y_up', label: 'Y Up' },
                        { value: 'z_up', label: 'Z Up' }
                      ]}
                      onChange={(value) => updateSetting('advanced', 'fileAxisDirection', value)}
                    />

                    <CheckboxSetting
                      label="Use the same settings for subsequent files"
                      checked={importSettings().advanced.useSettingsForSubsequentFiles}
                      onChange={(checked) => updateSetting('advanced', 'useSettingsForSubsequentFiles', checked)}
                    />
                  </div>
                </ImportSection>
              </div>
            </div>
          </div>

          <input
            ref={fileInputRef}
            type="file"
            multiple
            accept=".fbx,.obj,.gltf,.glb,.dae,.3ds,.blend,.max,.stl,.ply,.x3d,.md2,.md3,.md5,.lwo,.ac,.ms3d,.cob,.ifc,.xgl,.csm,.bvh,.b3d,.ndo,.dxf"
            onChange={handleFileInputChange}
            style={{ display: 'none' }}
          />
        </div>
      </div>
    </Show>
  );
}

// Helper Components
function ImportSection({ title, isExpanded, onToggle, children }) {
  const expanded = () => typeof isExpanded === 'function' ? isExpanded() : isExpanded;
  
  return (
    <div class="border border-base-300 rounded-lg">
      <button
        onClick={onToggle}
        class="w-full p-3 text-left flex items-center justify-between bg-primary/10 hover:bg-primary/20 transition-colors rounded-t-lg border-b border-primary/20"
      >
        <span class="font-semibold text-sm text-primary">{title}</span>
        <Show when={expanded()} fallback={<IconChevronRight class="w-4 h-4" />}>
          <IconChevronDown class="w-4 h-4" />
        </Show>
      </button>
      <Show when={expanded()}>
        <div class="p-3 border-t border-base-300">
          {children}
        </div>
      </Show>
    </div>
  );
}

function CheckboxSetting({ label, description, checked, onChange }) {
  return (
    <label class="flex items-start gap-2 cursor-pointer">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        class="checkbox checkbox-primary checkbox-sm mt-0.5"
      />
      <div class="flex flex-col">
        <span class="text-sm text-base-content">{label}</span>
        {description && (
          <span class="text-xs text-base-content/60">{description}</span>
        )}
      </div>
    </label>
  );
}

function NumberSetting({ label, value, onChange, min = 0, max = 100, step = 1 }) {
  return (
    <div class="space-y-1">
      <label class="text-xs font-medium text-base-content">{label}</label>
      <input
        type="number"
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value) || 0)}
        min={min}
        max={max}
        step={step}
        class="input input-sm input-bordered w-full"
      />
    </div>
  );
}

function NumberInput({ placeholder, value, onChange }) {
  return (
    <input
      type="number"
      placeholder={placeholder}
      value={value}
      onChange={(e) => onChange(parseFloat(e.target.value) || 0)}
      class="input input-sm input-bordered w-full"
    />
  );
}

function SelectSetting({ label, value, options, onChange }) {
  return (
    <div class="space-y-1">
      <label class="text-xs font-medium text-base-content">{label}</label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        class="select select-sm select-bordered w-full"
      >
        <For each={options}>
          {(option) => (
            <option value={option.value}>{option.label}</option>
          )}
        </For>
      </select>
    </div>
  );
}

export default ModelImporter;