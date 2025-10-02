import { Show, createSignal, createEffect, onMount, onCleanup } from 'solid-js';
import { IconPhoto, IconCode, IconX, IconCheck, IconCode as IconCodeSlash, IconArrowRight, IconVideo, IconFolder, IconFileCode, IconCube } from '@tabler/icons-solidjs';
import { generateThumbnail } from '@/api/bridge/thumbnails';
import { getFileUrl } from '@/api/bridge/files';
import { getCurrentProject } from '@/api/bridge/projects';
import MaterialThumbnail from '@/ui/MaterialThumbnail';
import { isMaterialFile, isMaterialPath } from '@/api/bridge/materialThumbnails';

const is3DModelFile = (extension) => {
  const modelExtensions = ['.glb', '.gltf', '.obj', '.fbx', '.dae', '.3ds', '.blend', '.max', '.ma', '.mb', '.stl', '.ply', '.x3d'];
  return modelExtensions.includes(extension?.toLowerCase() || '');
};

const isImageFile = (extension) => {
  const imageExtensions = [
    '.jpg', '.jpeg', '.png', '.gif', '.webp', '.bmp', '.tga', '.tiff', '.ico', '.svg',
    '.avif', '.heic', '.heif', '.dds', '.exr', '.hdr', '.psd', '.raw', '.cr2', '.nef'
  ];
  return imageExtensions.includes(extension?.toLowerCase() || '');
};

const isScriptFile = (extension) => {
  const scriptExtensions = ['.js', '.jsx', '.ts', '.tsx', '.py', '.json'];
  return scriptExtensions.includes(extension?.toLowerCase() || '');
};

const isCodeFile = (extension) => {
  const codeExtensions = ['.ren', '.c', '.cpp', '.h', '.hpp', '.cs', '.java', '.go', '.rs', '.php'];
  return codeExtensions.includes(extension?.toLowerCase() || '');
};


const ImageThumbnail = ({ asset, size = 'w-full h-full' }) => {
  const [imageLoaded, setImageLoaded] = createSignal(false);
  const [imageError, setImageError] = createSignal(false);
  const [thumbnailUrl, setThumbnailUrl] = createSignal(null);
  
  // Check if this is a thumbnail cache file to reduce debug noise
  const isThumbCache = asset?.path?.startsWith('.cache/thumbnails/');
  
  // Check if this is an HDR/EXR file that needs thumbnail generation
  const isHdrExr = asset?.extension && ['.hdr', '.exr'].includes(asset.extension.toLowerCase());
  
  // Skip debug logging for thumbnail cache files to reduce noise
  if (asset && isImageFile(asset.extension) && !isThumbCache) {
    console.log('Rendering ImageThumbnail for:', asset.name, 'extension:', asset.extension, 'path:', asset.path);
  }
  
  const getAssetThumbnailUrl = (asset) => {
    const currentProject = getCurrentProject();
    if (!currentProject?.name) return null;
    
    // Use asset.path if available, otherwise construct from asset.name
    const assetPath = asset.path || asset.name;
    if (!assetPath) return null;
    
    const projectPath = `projects/${currentProject.name}`;
    
    // Handle different path formats
    let fullPath;
    if (assetPath.startsWith('.cache/')) {
      // This is a cache file (thumbnails), don't add assets/ prefix
      fullPath = `${projectPath}/${assetPath}`;
    } else {
      // All other paths are relative to project root - use them exactly as provided
      fullPath = `${projectPath}/${assetPath}`;
    }
    
    const fileUrl = getFileUrl(fullPath);
    
    // Only log for non-cache files to reduce noise
    if (!isThumbCache) {
      console.log('Generated image URL:', fileUrl, 'for asset:', asset.name, 'fullPath:', fullPath);
    }
    
    return fileUrl;
  };
  
  // Handle HDR/EXR files by generating thumbnails
  const [thumbnailGenerating, setThumbnailGenerating] = createSignal(false);
  
  createEffect(() => {
    if (isHdrExr && asset) {
      // Only generate if we don't already have a thumbnail URL and aren't already generating
      if (!thumbnailUrl() && !thumbnailGenerating() && !imageError()) {
        const currentProject = getCurrentProject();
        if (currentProject?.name) {
          setThumbnailGenerating(true);
          
          // Run thumbnail generation asynchronously
          (async () => {
            try {
              console.log('Generating thumbnail for HDR/EXR file:', asset.name);
              const thumbnailResponse = await generateThumbnail(asset.path || asset.name, 256);
              
              if (thumbnailResponse.success && thumbnailResponse.thumbnail_file) {
                const thumbnailPath = `projects/${currentProject.name}/${thumbnailResponse.thumbnail_file}`;
                const thumbnailFileUrl = getFileUrl(thumbnailPath);
                setThumbnailUrl(thumbnailFileUrl);
                console.log('HDR/EXR thumbnail generated:', thumbnailFileUrl);
              } else {
                console.warn('Failed to generate HDR/EXR thumbnail:', thumbnailResponse.error);
                setImageError(true);
              }
            } catch (error) {
              console.error('Error generating HDR/EXR thumbnail:', error);
              setImageError(true);
            } finally {
              setThumbnailGenerating(false);
            }
          })();
        }
      }
    } else {
      // For regular images, use direct URL
      if (!thumbnailUrl()) {
        setThumbnailUrl(getAssetThumbnailUrl(asset));
      }
    }
  });
  
  if (!thumbnailUrl()) {
    if (!isThumbCache && !isHdrExr) {
      console.warn('No thumbnail URL generated for image asset:', asset.name, asset.path);
    }
    return (
      <div class={`${size} bg-base-300 rounded flex items-center justify-center`}>
        <Show when={isHdrExr} fallback={
          <>
            <IconPhoto class="w-10 h-10 text-base-content/60" />
            <div class="absolute bottom-1 right-1 text-xs text-warning bg-warning/10 px-1 rounded">
              No URL
            </div>
          </>
        }>
          <div class="flex flex-col items-center justify-center">
            <IconPhoto class="w-10 h-10 text-orange-500" />
            <div class="text-xs text-orange-500 mt-1">HDR/EXR</div>
            <div class="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin mt-2"></div>
          </div>
        </Show>
      </div>
    );
  }
  
  return (
    <div class={`${size} bg-base-300 rounded overflow-hidden relative`}>
      <Show when={!imageError()} fallback={
        <div class="w-full h-full flex items-center justify-center">
          <IconPhoto class="w-10 h-10 text-base-content/60" />
          <div class="absolute bottom-1 right-1 text-xs text-error bg-error/10 px-1 rounded">
            Error
          </div>
        </div>
      }>
        <img 
          src={thumbnailUrl()}
          alt={asset.name}
          class={`w-full h-full object-cover transition-all duration-300 ${
            imageLoaded() ? 'opacity-100 scale-100' : 'opacity-0 scale-95'
          } hover:scale-105 cursor-pointer`}
          style={{
            "image-rendering": "auto",
            "background-color": "var(--fallback-bc,oklch(var(--bc)/0.2))"
          }}
          onLoad={() => {
            if (!isThumbCache) {
              console.log('Successfully loaded image:', asset.name);
            }
            setImageLoaded(true);
            setImageError(false);
          }}
          onError={(e) => {
            if (!isThumbCache) {
              console.warn('Failed to load image:', thumbnailUrl(), 'for asset:', asset.name, 'Error:', e);
            }
            setImageError(true);
            setImageLoaded(false);
          }}
        />
        <Show when={!imageLoaded() && !imageError()}>
          <div class="absolute inset-0 bg-base-300 flex items-center justify-center">
            <div class="w-4 h-4 border-2 border-primary border-t-transparent rounded-full animate-spin"></div>
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
  
  let tooltipTimeout;
  
  // Hide tooltips on global shortcuts
  const handleGlobalKeydown = (e) => {
    // Hide tooltip when spacebar (bottom panel toggle) or other shortcuts are pressed
    if (e.code === 'Space' || e.key === 'p' || e.key === 'Escape') {
      hideTooltip();
    }
  };
  
  // Add global keydown listener
  onMount(() => {
    document.addEventListener('keydown', handleGlobalKeydown);
  });
  
  onCleanup(() => {
    document.removeEventListener('keydown', handleGlobalKeydown);
    clearTimeout(tooltipTimeout);
  });
  
  const showTooltipDelayed = () => {
    if (isDragging()) return;
    
    // Clear any existing timeout
    clearTimeout(tooltipTimeout);
    
    tooltipTimeout = setTimeout(() => {
      if (hoveredItem() === asset.id) { // Only show if still hovering this item
        setShowTooltip(true);
        const rect = document.querySelector(`[data-asset-id="${asset.id}"]`)?.getBoundingClientRect();
        if (rect) {
          const tooltipHeight = 64;
          const tooltipWidth = 192;
          // Position tooltip near the center of the asset item
          const centerX = rect.left + (rect.width / 2);
          const centerY = rect.top;
          
          document.dispatchEvent(new CustomEvent('global:tooltip-show', { 
            detail: {
              x: Math.max(10, Math.min(centerX - (tooltipWidth / 2), window.innerWidth - tooltipWidth - 10)),
              y: Math.max(10, centerY - tooltipHeight - 10),
              asset: asset
            }
          }));
        }
      }
    }, 300); // Reduced delay for more responsive tooltips
  };
  
  const hideTooltip = () => {
    clearTimeout(tooltipTimeout);
    setShowTooltip(false);
    
    // Hide tooltip events
    document.dispatchEvent(new CustomEvent('global:tooltip-hide'));
    document.dispatchEvent(new CustomEvent('global:tooltip-force-hide'));
    
    // Additional cleanup - remove any lingering tooltip elements
    setTimeout(() => {
      const tooltips = document.querySelectorAll('[data-tooltip], .tooltip-container, .asset-tooltip');
      tooltips.forEach(el => el.remove());
    }, 10);
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
    if (['.js', '.jsx', '.ts', '.tsx', '.py', '.json'].includes(ext)) return 'scripts';
    if (['.ren', '.c', '.cpp', '.h', '.hpp', '.cs', '.java', '.go', '.rs', '.php'].includes(ext)) return 'code';
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
        fileType: getAssetCategory(a.extension) === 'scripts' ? 'script' : getAssetCategory(a.extension) === 'code' ? 'code' : getAssetCategory(a.extension)
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
        fileType: getAssetCategory(asset.extension) === 'scripts' ? 'script' : getAssetCategory(asset.extension) === 'code' ? 'code' : getAssetCategory(asset.extension)
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
        onMouseEnter={() => {
          setHoveredItem(asset.id);
          showTooltipDelayed();
        }}
        onMouseLeave={() => {
          setHoveredItem(null);
          hideTooltip();
        }}
        onMouseDown={() => {
          // Hide tooltip immediately on mouse down to prevent it from sticking
          hideTooltip();
        }}
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
              <Show when={isMaterialFile(asset.extension) || isMaterialPath(asset.path)} fallback={
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
                    <IconFileCode class="w-5 h-5 text-primary" />
                  ) : isCodeFile(asset.extension) ? (
                    <IconCode class="w-5 h-5 text-success" />
                  ) : (
                    <IconPhoto class="w-5 h-5 text-base-content/60" />
                  )}
                </div>
              }>
                <MaterialThumbnail asset={asset} size="w-full h-full" />
              </Show>
            }>
              <ImageThumbnail asset={asset} />
            </Show>
          }>
            <div class="w-full h-full bg-base-300 rounded flex items-center justify-center">
              <IconCube class="w-5 h-5 text-purple-500" />
            </div>
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
      onMouseEnter={() => {
        setHoveredItem(asset.id);
        showTooltipDelayed();
      }}
      onMouseLeave={() => {
        setHoveredItem(null);
        hideTooltip();
      }}
      onMouseDown={() => {
        // Hide tooltip immediately on mouse down to prevent it from sticking
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
            <Show when={isMaterialFile(asset.extension) || isMaterialPath(asset.path)} fallback={
              <div>
                {asset.type === 'folder' ? (
                  <IconFolder class="w-16 h-16 text-yellow-500" />
                ) : isScriptFile(asset.extension) ? (
                  <IconFileCode class="w-16 h-16 text-blue-500" />
                ) : isCodeFile(asset.extension) ? (
                  <IconCode class="w-16 h-16 text-green-500" />
                ) : (
                  <IconPhoto class="w-16 h-16 text-base-content/60" />
                )}
              </div>
            }>
              <MaterialThumbnail asset={asset} size="w-full h-full" />
            </Show>
          }>
            <ImageThumbnail asset={asset} size="w-full h-full" />
          </Show>
        }>
          <div class="w-full h-full bg-base-300 flex items-center justify-center">
            <IconCube class="w-16 h-16 text-purple-500" />
          </div>
        </Show>
        
        {/* Colored separator between thumbnail and text */}
        <div class={`absolute bottom-0 left-0 right-0 h-0.5 ${
          asset.type === 'folder' 
            ? 'bg-yellow-500' 
            : isImageFile(asset.extension)
              ? 'bg-green-500'
            : isScriptFile(asset.extension) 
              ? 'bg-blue-500'
            : isCodeFile(asset.extension)
              ? 'bg-green-500'
            : is3DModelFile(asset.extension)
              ? 'bg-purple-500'
            : (isMaterialFile(asset.extension) || isMaterialPath(asset.path))
              ? 'bg-orange-500'
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