import { Show, createSignal, createEffect } from 'solid-js';
import { IconPhoto, IconCode, IconX, IconCheck, IconCode as IconCodeSlash, IconArrowRight, IconVideo, IconFolder } from '@tabler/icons-solidjs';
import { generateThumbnail } from '@/api/bridge/thumbnails';
import { getFileUrl } from '@/api/bridge/files';
import { getCurrentProject } from '@/api/bridge/projects';

const is3DModelFile = (extension) => {
  const modelExtensions = ['.glb', '.gltf', '.obj', '.fbx', '.dae', '.3ds', '.blend', '.max', '.ma', '.mb', '.stl', '.ply', '.x3d'];
  return modelExtensions.includes(extension?.toLowerCase() || '');
};

const isImageFile = (extension) => {
  const imageExtensions = ['.jpg', '.jpeg', '.png', '.gif', '.webp', '.bmp', '.tga', '.tiff', '.ico', '.svg'];
  return imageExtensions.includes(extension?.toLowerCase() || '');
};

const isScriptFile = (extension) => {
  const scriptExtensions = ['.js', '.jsx', '.ts', '.tsx', '.py', '.ren'];
  return scriptExtensions.includes(extension?.toLowerCase() || '');
};

const ModelThumbnail = ({ asset, size = 'w-full h-full' }) => {
  const [thumbnailUrl, setThumbnailUrl] = createSignal(null);
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal(false);

  createEffect(async () => {
    const currentProject = getCurrentProject();
    if (!currentProject?.name || !is3DModelFile(asset.extension)) return;

    try {
      setIsLoading(true);
      
      const assetPath = asset.path || asset.name;
      const fullAssetPath = assetPath.startsWith('assets/') ? assetPath : `assets/${assetPath}`;
      // Requesting thumbnail generation
      
      // Check if thumbnail file exists first
      const assetFilename = fullAssetPath.split('/').pop()?.replace('.glb', '') || 'thumbnail';
      const thumbnailPath = `projects/${currentProject.name}/.cache/thumbnails/${assetFilename}_256.png`;
      
      try {
        // Try to use existing thumbnail file first
        const fileUrl = getFileUrl(thumbnailPath);
        
        // Test if the file exists by trying to load it
        await new Promise((resolve, reject) => {
          const img = new Image();
          img.onload = resolve;
          img.onerror = reject;
          img.src = fileUrl;
        });
        
        setThumbnailUrl(fileUrl);
        setError(false);
        // Using cached thumbnail
      } catch {
        // Thumbnail file doesn't exist, generate it (fallback)
        // Generating new thumbnail
        const result = await generateThumbnail(fullAssetPath, 256);
        
        if (result.success && result.thumbnail_file) {
          // Use the generated thumbnail file
          const fileUrl = getFileUrl(`projects/${currentProject.name}/${result.thumbnail_file}`);
          setThumbnailUrl(fileUrl);
          setError(false);
          // Successfully generated new thumbnail
        } else {
          throw new Error(result.error || 'Failed to generate thumbnail');
        }
      }
    } catch (err) {
      console.error('Failed to get model thumbnail:', err);
      setError(true);
    } finally {
      setIsLoading(false);
    }
  });

  return (
    <Show when={!isLoading()} fallback={
      <div class={`${size} bg-base-300 rounded flex items-center justify-center`}>
        <div class="w-4 h-4 border-2 border-success border-t-transparent rounded-full animate-spin"></div>
      </div>
    }>
      <Show when={!error() && thumbnailUrl()} fallback={
        <div class={`${size} bg-base-300 rounded flex items-center justify-center`}>
          <IconPhoto class="w-10 h-10 text-base-content/40" />
        </div>
      }>
        <div class={`${size} bg-base-300 rounded overflow-hidden`}>
          <img 
            src={thumbnailUrl()}
            alt={asset.name}
            class="w-full h-full object-cover"
          />
        </div>
      </Show>
    </Show>
  );
};

const ImageThumbnail = ({ asset, size = 'w-full h-full' }) => {
  const [imageLoaded, setImageLoaded] = createSignal(false);
  const [imageError, setImageError] = createSignal(false);
  
  const getAssetThumbnailUrl = (asset) => {
    const currentProject = getCurrentProject();
    if (!currentProject?.name) return null;
    
    const assetPath = asset.name || asset.path;
    const projectPath = `projects/${currentProject.name}`;
    const fullPath = assetPath.startsWith('assets/') 
      ? `${projectPath}/${assetPath}`
      : `${projectPath}/assets/${assetPath}`;
    
    return getFileUrl(fullPath);
  };
  
  const thumbnailUrl = getAssetThumbnailUrl(asset);
  
  if (!thumbnailUrl) {
    return (
      <div class={`${size} bg-base-300 rounded flex items-center justify-center`}>
        <IconPhoto class="w-10 h-10 text-success" />
      </div>
    );
  }
  
  return (
    <div class={`${size} bg-base-300 rounded overflow-hidden relative`}>
      <Show when={!imageError()} fallback={
        <div class="w-full h-full flex items-center justify-center">
          <IconPhoto class="w-10 h-10 text-success" />
        </div>
      }>
        <img 
          src={thumbnailUrl}
          alt={asset.name}
          class={`w-full h-full object-cover transition-opacity duration-200 ${
            imageLoaded() ? 'opacity-100' : 'opacity-0'
          }`}
          onLoad={() => setImageLoaded(true)}
          onError={() => {
            setImageError(true);
            setImageLoaded(false);
          }}
        />
        <Show when={!imageLoaded()}>
          <div class="absolute inset-0 bg-base-300 flex items-center justify-center">
            <div class="w-4 h-4 border-2 border-success border-t-transparent rounded-full animate-spin"></div>
          </div>
        </Show>
      </Show>
    </div>
  );
};

function AssetItem({
  asset,
  index,
  layoutMode,
  isAssetSelected,
  hoveredItem,
  setHoveredItem,
  setTooltip,
  toggleAssetSelection,
  handleAssetDoubleClick,
  isInternalDrag,
  setIsInternalDrag,
  selectedAssets,
  setSelectedAssets,
  lastSelectedAsset,
  setLastSelectedAsset,
  filteredAssets,
  setDragOverFolder,
  setDragOverTreeFolder,
  setDragOverBreadcrumb,
  loadedAssets,
  preloadingAssets,
  failedAssets,
  setFailedAssets,
  setPreloadingAssets,
  setLoadedAssets,
  getExtensionStyle
}) {
  const [contextMenu, setContextMenu] = createSignal(null);
  const [showTooltip, setShowTooltip] = createSignal(false);
  const [isDragging, setIsDragging] = createSignal(false);
  
  let globalMouseX = 0;
  let globalMouseY = 0;
  let tooltipTimeout;
  
  const updateGlobalMousePosition = (e) => {
    globalMouseX = e.clientX;
    globalMouseY = e.clientY;
    if (showTooltip() && !isDragging()) {
      const tooltipHeight = 64;
      const tooltipWidth = 192;
      document.dispatchEvent(new CustomEvent('global:tooltip-show', { 
        detail: {
          x: Math.max(10, Math.min(globalMouseX - (tooltipWidth / 2) + 50, window.innerWidth - tooltipWidth - 10)),
          y: Math.max(10, globalMouseY - tooltipHeight - 50),
          asset: asset
        }
      }));
    }
  };
  
  const showTooltipDelayed = () => {
    if (isDragging()) return;
    tooltipTimeout = setTimeout(() => {
      setShowTooltip(true);
      const rect = document.querySelector(`[data-asset-id="${asset.id}"]`)?.getBoundingClientRect();
      if (rect) {
        const tooltipHeight = 64;
        const tooltipWidth = 192;
        document.dispatchEvent(new CustomEvent('global:tooltip-show', { 
          detail: {
            x: Math.max(10, Math.min(globalMouseX - (tooltipWidth / 2) + 50, window.innerWidth - tooltipWidth - 10)),
            y: Math.max(10, globalMouseY - tooltipHeight - 50),
            asset: asset
          }
        }));
      }
    }, 500);
  };
  
  const hideTooltip = () => {
    clearTimeout(tooltipTimeout);
    setShowTooltip(false);
    document.dispatchEvent(new CustomEvent('global:tooltip-hide'));
    document.removeEventListener('mousemove', updateGlobalMousePosition);
  };
  
  const handleContextMenu = (e) => {
    // Context menu triggered
    e.preventDefault();
    e.stopPropagation();
    
    // Check if it's a script file
    const isScript = isScriptFile(asset.extension);
    // Check if script file
    
    // Only show context menu for script files
    if (!isScript) {
      // Not a script file, skipping context menu
      return;
    }
    
    const { clientX: x, clientY: y } = e;
    // Setting context menu position
    setContextMenu({
      position: { x, y },
      asset: asset
    });
  };

  const openInViewport = (side = 'left') => {
    // Dispatch a custom event to trigger the viewport split view
    document.dispatchEvent(new CustomEvent('asset:open-in-viewport', {
      detail: {
        asset: asset,
        side: side,
        script: {
          name: asset.name,
          path: asset.path || asset.name
        }
      }
    }));
    
    setContextMenu(null);
  };

  const closeContextMenu = () => {
    setContextMenu(null);
  };

  // Close context menu when clicking elsewhere
  createEffect(() => {
    if (contextMenu()) {
      const handleClickOutside = (e) => {
        // Check if click is outside context menu
        if (!e.target.closest('.asset-context-menu')) {
          closeContextMenu();
        }
      };
      
      document.addEventListener('click', handleClickOutside);
      return () => document.removeEventListener('click', handleClickOutside);
    }
  });
  
  const getAssetCategory = (extension) => {
    const ext = extension?.toLowerCase() || '';
    if (['.glb', '.gltf', '.obj', '.fbx'].includes(ext)) return '3d-models';
    if (['.jpg', '.jpeg', '.png', '.webp', '.bmp', '.tga'].includes(ext)) return 'textures';
    if (['.mp3', '.wav', '.ogg', '.m4a'].includes(ext)) return 'audio';
    if (['.js', '.jsx', '.ts', '.tsx', '.py', '.ren'].includes(ext)) return 'scripts';
    return 'misc';
  };

  const startDrag = (e, asset) => {
    setIsInternalDrag(true);
    setIsDragging(true);
    hideTooltip();
    
    if (!isAssetSelected(asset.id)) {
      setSelectedAssets(new Set([asset.id]));
      setLastSelectedAsset(asset);
    }
    
    const selectedAssetIds = Array.from(selectedAssets());
    const allAssets = filteredAssets();
    const selectedAssetObjects = allAssets.filter(a => selectedAssetIds.includes(a.id));
    
    const dragData = {
      type: selectedAssetObjects.length > 1 ? 'multiple-assets' : 'asset',
      assetType: 'file',
      assets: selectedAssetObjects.map(a => ({
        id: a.id,
        name: a.name,
        path: a.path,
        assetType: a.type,
        fileName: a.fileName,
        extension: a.extension,
        mimeType: a.mimeType,
        category: getAssetCategory(a.extension),
        fileType: getAssetCategory(a.extension) === 'scripts' ? 'script' : getAssetCategory(a.extension)
      })),
      ...(selectedAssetObjects.length === 1 ? {
        id: asset.id,
        name: asset.name,
        path: asset.path,
        assetType: asset.type,
        fileName: asset.fileName,
        extension: asset.extension,
        mimeType: asset.mimeType,
        category: getAssetCategory(asset.extension),
        fileType: getAssetCategory(asset.extension) === 'scripts' ? 'script' : getAssetCategory(asset.extension)
      } : {})
    };
    
    // Store drag data globally for viewport access
    window._currentDragData = dragData;
    
    e.dataTransfer.setData('application/json', JSON.stringify(dragData));
    e.dataTransfer.setData('text/plain', JSON.stringify(dragData));
    e.dataTransfer.setData('application/x-asset-drag', JSON.stringify(dragData));
    e.dataTransfer.effectAllowed = 'copy';
    
    // Hide default drag image for 3D models to avoid duplicate visuals
    const extension = asset.extension?.toLowerCase();
    if (['.glb', '.gltf', '.obj'].includes(extension)) {
      // Create transparent drag image to hide default clone
      const transparentDiv = document.createElement('div');
      transparentDiv.style.opacity = '0';
      transparentDiv.style.position = 'absolute';
      transparentDiv.style.top = '-1000px';
      document.body.appendChild(transparentDiv);
      e.dataTransfer.setDragImage(transparentDiv, 0, 0);
      setTimeout(() => document.body.removeChild(transparentDiv), 0);
    } else if (selectedAssetObjects.length > 1) {
      const dragImage = document.createElement('div');
      dragImage.className = 'fixed top-[-1000px] bg-primary text-primary-content px-3 py-2 rounded-lg font-medium shadow-lg';
      dragImage.textContent = `Moving ${selectedAssetObjects.length} files`;
      document.body.appendChild(dragImage);
      e.dataTransfer.setDragImage(dragImage, 50, 25);
      setTimeout(() => document.body.removeChild(dragImage), 0);
    } else if (selectedAssetObjects.length === 1 && getAssetCategory(selectedAssetObjects[0].extension) === 'scripts') {
      // Create custom drag card for script files
      const dragCard = document.createElement('div');
      dragCard.className = 'fixed top-[-1000px] bg-success text-success-content rounded-lg p-3 shadow-lg flex items-center gap-2 min-w-[200px]';
      dragCard.innerHTML = `
        <div class="w-8 h-8 bg-success-content/20 rounded flex items-center justify-center">
          <svg class="w-4 h-4 text-success-content" fill="currentColor" viewBox="0 0 24 24">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6z"/>
            <path d="M14 2v6h6"/>
            <path d="M16 13H8"/>
            <path d="M16 17H8"/>
            <path d="M10 9H8"/>
          </svg>
        </div>
        <div class="flex flex-col">
          <span class="text-sm font-medium text-success-content">${selectedAssetObjects[0].name}</span>
          <span class="text-xs text-success-content">Script file</span>
        </div>
      `;
      document.body.appendChild(dragCard);
      e.dataTransfer.setDragImage(dragCard, 100, 25);
      setTimeout(() => document.body.removeChild(dragCard), 0);
    }
  };

  if (layoutMode() === 'list') {
    return (
      <div
        class={`group cursor-pointer transition-all duration-200 p-1 flex items-center gap-2 ${
          isAssetSelected(asset.id)
            ? 'bg-primary/20 border-l-2 border-primary hover:bg-primary/30'
            : typeof index === 'function' && index() % 2 === 0 
              ? 'bg-base-200/50 hover:bg-base-300/50' 
              : 'bg-base-300/30 hover:bg-base-300/50'
        }`}
        data-asset-id={asset.id}
        draggable={true}
        onMouseEnter={() => setHoveredItem(asset.id)}
        onMouseLeave={() => setHoveredItem(null)}
        onClick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          hideTooltip();
          toggleAssetSelection(asset, e.ctrlKey || e.metaKey, e.shiftKey);
        }}
        onDblClick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          hideTooltip();
          handleAssetDoubleClick(asset);
        }}
        onDragStart={(e) => startDrag(e, asset)}
        onDragEnd={() => {
          setIsInternalDrag(false);
          setIsDragging(false);
          setDragOverFolder(null);
          setDragOverTreeFolder(null);
          setDragOverBreadcrumb(null);
          window._currentDragData = null;
        }}
      >
        <div class="w-8 h-8 flex items-center justify-center flex-shrink-0 relative">
          <Show when={is3DModelFile(asset.extension)} fallback={
            <Show when={isImageFile(asset.extension)} fallback={
              <div class={`w-full h-full bg-base-300 rounded flex items-center justify-center ${
                  loadedAssets().includes(asset.id) 
                    ? 'opacity-100' 
                    : failedAssets().includes(asset.id) 
                      ? 'opacity-40 grayscale' 
                      : 'opacity-60'
                }`}>
                {asset.type === 'folder' ? (
                  <IconFolder class="w-5 h-5 text-warning" />
                ) : isScriptFile(asset.extension) ? (
                  <IconCodeSlash class="w-5 h-5 text-primary" />
                ) : (
                  <IconPhoto class="w-5 h-5 text-base-content/60" />
                )}
              </div>
            }>
              <ImageThumbnail asset={asset} />
            </Show>
          }>
            <ModelThumbnail asset={asset} />
          </Show>

          <div class="absolute -bottom-1 -right-1">
            <Show when={preloadingAssets().includes(asset.id)}>
              <div class="w-3 h-3 bg-warning rounded-full flex items-center justify-center">
                <div class="w-1.5 h-1.5 border border-white border-t-transparent rounded-full animate-spin"></div>
              </div>
            </Show>
            <Show when={failedAssets().includes(asset.id)}>
              <div class="w-3 h-3 bg-error rounded-full flex items-center justify-center">
                <IconX class="w-2 h-2 text-white" />
              </div>
            </Show>
            <Show when={loadedAssets().includes(asset.id)}>
              <div class="w-3 h-3 bg-success rounded-full flex items-center justify-center">
                <IconCheck class="w-2 h-2 text-white" />
              </div>
            </Show>
          </div>
        </div>
        
        <div class="flex-1 min-w-0">
          <div class="text-sm text-base-content/70 group-hover:text-base-content transition-colors truncate">
            {asset.name}
          </div>
          <div class="text-xs text-base-content/50 truncate">
            {asset.extension?.toUpperCase()} • {asset.size ? `${Math.round(asset.size / 1024)} KB` : 'Unknown size'}
          </div>
        </div>

        <Show when={asset.extension}>
          {(() => {
            const style = getExtensionStyle(asset.extension);
            return (
              <div class="flex-shrink-0">
                <div class={`${style.bgColor} ${style.textColor} text-xs font-bold px-2 py-1 rounded-full flex items-center transition-colors ${style.hoverColor} ${style.icon ? 'gap-1' : ''} shadow-sm`}>
                  {style.icon}
                  <span>{asset.extension.replace('.', '').toUpperCase()}</span>
                </div>
              </div>
            );
          })()}
        </Show>
      </div>
    );
  }

  return (
    <div
      class={`group cursor-pointer relative ${
        isAssetSelected(asset.id) 
          ? 'border border-blue-500' 
          : 'border border-transparent'
      }`}
      data-asset-id={asset.id}
      draggable={true}
      onMouseEnter={(e) => {
        setHoveredItem(asset.id);
        globalMouseX = e.clientX;
        globalMouseY = e.clientY;
        document.addEventListener('mousemove', updateGlobalMousePosition);
        showTooltipDelayed();
      }}
      onMouseLeave={() => {
        setHoveredItem(null);
        hideTooltip();
      }}
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        hideTooltip();
        
        if (failedAssets().includes(asset.id)) {
          setFailedAssets(prev => prev.filter(id => id !== asset.id));
          setPreloadingAssets(prev => [...prev, asset.id]);
          setTimeout(() => {
            setPreloadingAssets(prev => prev.filter(id => id !== asset.id));
            setLoadedAssets(prev => [...prev, asset.id]);
          }, 1000);
        } else {
          toggleAssetSelection(asset, e.ctrlKey || e.metaKey, e.shiftKey);
        }
      }}
      onDblClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        hideTooltip();
        handleAssetDoubleClick(asset);
      }}
      onDragStart={(e) => startDrag(e, asset)}
      onContextMenu={handleContextMenu}
      onDragEnd={() => {
        setIsInternalDrag(false);
        setIsDragging(false);
        setDragOverFolder(null);
        setDragOverTreeFolder(null);
        setDragOverBreadcrumb(null);
        window._currentDragData = null;
      }}
    >
      {/* Square Thumbnail */}
      <div class="w-full aspect-square bg-base-300 flex items-center justify-center relative border-t border-l border-r border-base-content/20">
        <Show when={is3DModelFile(asset.extension)} fallback={
          <Show when={isImageFile(asset.extension)} fallback={
            <div>
              {asset.type === 'folder' ? (
                <IconFolder class="w-16 h-16 text-yellow-500" />
              ) : isScriptFile(asset.extension) ? (
                <IconCodeSlash class="w-16 h-16 text-blue-500" />
              ) : (
                <IconPhoto class="w-16 h-16 text-base-content/60" />
              )}
            </div>
          }>
            <ImageThumbnail asset={asset} size="w-full h-full" />
          </Show>
        }>
          <ModelThumbnail asset={asset} size="w-full h-full" />
        </Show>
        
        {/* Colored separator between thumbnail and text */}
        <div class={`absolute bottom-0 left-0 right-0 h-0.5 ${
          asset.type === 'folder' 
            ? 'bg-yellow-500' 
            : isScriptFile(asset.extension) 
              ? 'bg-blue-500' 
              : 'bg-gray-500'
        }`}></div>
      </div>
      
      {/* Text Label */}
      <div class="w-full bg-base-200 border-x border-b border-base-content/20 p-2 text-center h-12 flex items-center justify-center">
        <div class="text-xs text-base-content leading-tight line-clamp-2" title={asset.name}>
          {asset.name}
        </div>
      </div>
      
      {/* Context Menu */}
      {(() => {
        const menu = contextMenu();
        // Context menu rendered
        return menu;
      })()}
      <Show when={contextMenu()}>
        <div 
          class="asset-context-menu fixed z-50 bg-base-200 border border-base-300 rounded-lg shadow-lg py-2 min-w-[180px]"
          style={{
            left: `${contextMenu().position.x}px`,
            top: `${contextMenu().position.y}px`
          }}
        >
          <div class="px-3 py-1 text-xs text-base-content/60 border-b border-base-300 mb-1">
            {asset.name}
          </div>
          
          <button 
            class="w-full px-3 py-2 text-left text-sm text-base-content hover:bg-base-300/50 transition-colors flex items-center gap-2"
            onClick={() => openInViewport('left')}
          >
            <IconArrowRight class="w-4 h-4 rotate-180" />
            Open in Viewport (Left)
          </button>
          
          <button 
            class="w-full px-3 py-2 text-left text-sm text-base-content hover:bg-base-300/50 transition-colors flex items-center gap-2"
            onClick={() => openInViewport('right')}
          >
            <IconArrowRight class="w-4 h-4" />
            Open in Viewport (Right)
          </button>
        </div>
      </Show>
      
    </div>
  );
}

export default AssetItem;