import { Show } from 'solid-js';
import { IconUpload, IconFolderOpen, IconFolder, IconPhoto, IconCube, IconCode } from '@tabler/icons-solidjs';

function AssetUploadArea({ 
  isDragOver, 
  isUploading,
  loading,
  error,
  filteredAssets,
  searchQuery,
  viewMode,
  selectedCategory,
  assetCategories,
  fileInputRef,
  folderInputRef,
  onFileInputChange,
  onFolderInputChange
}) {
  const getCategoryIcon = (categoryId) => {
    const iconMap = {
      '3d-models': IconCube,
      'textures': IconPhoto,
      'scripts': IconCode,
      'misc': IconFolder
    };
    return iconMap[categoryId] || IconFolder;
  };

  return (
    <>
      <Show when={loading()}>
        <div class="flex-1 flex items-center justify-center">
          <div class="text-center text-base-content/60">
            <p class="text-sm">Loading assets...</p>
          </div>
        </div>
      </Show>
      
      <Show when={error()}>
        <div class="flex-1 flex items-center justify-center">
          <div class="text-center text-error">
            <p class="text-sm">Error: {error()}</p>
          </div>
        </div>
      </Show>
      
      <Show when={isUploading()}>
        <div class="flex-1 flex items-center justify-center">
          <div class="text-center text-primary">
            <div class="flex items-center justify-center gap-2">
              <div class="w-4 h-4 border-2 border-primary border-t-transparent rounded-full animate-spin"></div>
              <p class="text-sm">Uploading files...</p>
            </div>
          </div>
        </div>
      </Show>
      
      <Show when={isDragOver()}>
        <div class="absolute inset-0 flex items-center justify-center bg-primary/20 backdrop-blur-sm z-10">
          <div class="text-center">
            <div class="w-16 h-16 mx-auto mb-4 border-2 border-primary border-dashed rounded-lg flex items-center justify-center">
              <IconUpload class="w-8 h-8 text-primary" />
            </div>
            <p class="text-lg font-medium text-primary">Drop files to upload</p>
            <p class="text-sm text-primary/80">Supports 3D models, textures, audio, and more</p>
          </div>
        </div>
      </Show>
      
      <Show when={!loading() && !error() && !isUploading()}>
        <Show when={filteredAssets().length === 0}>
          <div class="flex-1 flex items-center justify-center">
            <Show when={searchQuery()} fallback={
              <div class="text-center">
                <div class="w-16 h-16 sm:w-20 sm:h-20 mx-auto mb-4 sm:mb-6 border-2 border-base-content/40 border-dashed rounded-xl flex items-center justify-center bg-base-200/30">
                  {(() => {
                    if (viewMode() === 'folder') {
                      return <IconFolderOpen class="w-8 h-8 sm:w-10 sm:h-10 text-base-content/50" />;
                    } else {
                      const CategoryIcon = getCategoryIcon(selectedCategory());
                      return <CategoryIcon class="w-8 h-8 sm:w-10 sm:h-10 text-base-content/50" />;
                    }
                  })()}
                </div>
                
                <h3 class="text-base sm:text-lg font-medium text-base-content/70 mb-2">
                  {viewMode() === 'folder' 
                    ? 'Empty folder'
                    : `No ${assetCategories()?.[selectedCategory()]?.name?.toLowerCase() || 'assets'} found`
                  }
                </h3>
                
                <Show when={viewMode() === 'folder'}>
                  <div class="flex flex-col sm:flex-row gap-3 mb-3 sm:mb-4">
                    <button
                      onClick={() => fileInputRef?.click()}
                      class="flex items-center justify-center gap-2 px-4 py-2 bg-primary hover:bg-primary/80 text-primary-content text-sm font-medium rounded-lg transition-colors min-w-[120px]"
                    >
                      <IconUpload class="w-4 h-4" />
                      Upload Files
                    </button>
                    
                    <button
                      onClick={() => folderInputRef?.click()}
                      class="flex items-center justify-center gap-2 px-4 py-2 border border-base-300 hover:border-base-content/50 hover:bg-base-200/50 text-base-content/70 text-sm font-medium rounded-lg transition-colors min-w-[120px]"
                    >
                      <IconFolder class="w-4 h-4" />
                      Upload Folder
                    </button>
                  </div>
                  
                  <p class="text-xs text-base-content/50">
                    Or drag and drop files anywhere in this area
                  </p>
                </Show>
              </div>
            }>
              <div class="text-center text-base-content/50">
                <p class="text-sm">No assets found matching "{searchQuery()}"</p>
                <p class="text-xs text-base-content/40 mt-2">Try adjusting your search or upload new assets</p>
              </div>
            </Show>
          </div>
        </Show>
      </Show>
    </>
  );
}

export default AssetUploadArea;