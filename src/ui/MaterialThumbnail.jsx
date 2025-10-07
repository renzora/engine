import { Show, createSignal, createEffect } from 'solid-js';
import { IconPalette } from '@tabler/icons-solidjs';
import { materialThumbnailAPI, isMaterialFile, isMaterialPath } from '@/api/bridge/materialThumbnails';
import { getFileUrl } from '@/api/bridge/files';
import { getCurrentProject } from '@/api/bridge/projects';

const MaterialThumbnail = ({ asset, size = 'w-full h-full' }) => {
  const [thumbnailUrl, setThumbnailUrl] = createSignal(null);
  const [isLoading, setIsLoading] = createSignal(true);
  const [error, setError] = createSignal(false);

  createEffect(async () => {
    const currentProject = getCurrentProject();
    if (!currentProject?.name) return;

    // Check if this is a material file - either by extension or path
    const isMaterialByExtension = isMaterialFile(asset.extension);
    const isMaterialByPath = isMaterialPath(asset.path);
    const isMaterial = isMaterialByExtension || isMaterialByPath;
    
    if (!isMaterial) {
      setIsLoading(false);
      setError(true);
      return;
    }

    try {
      setIsLoading(true);
      setError(false);
      
      const assetPath = asset.path || asset.name;
      console.log('MaterialThumbnail: Original asset path:', assetPath);
      console.log('MaterialThumbnail: Asset object:', asset);
      
      // Use the path as-is, let the backend handle proper path resolution
      const fullAssetPath = assetPath;
      
      // Check if thumbnail file exists first
      const assetFilename = fullAssetPath.split('/').pop() || 'material';
      const materialFilename = assetFilename.replace(/\.(json|material|mat)$/i, '');
      const thumbnailPath = `projects/${currentProject.name}/.cache/thumbnails/${materialFilename}_material_256.png`;
      
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
      } catch {
        // Thumbnail file doesn't exist, generate it
        console.log('Generating material thumbnail for:', fullAssetPath);
        const result = await materialThumbnailAPI.generateMaterialThumbnail(
          currentProject.name,
          fullAssetPath,
          256
        );
        
        if (result.success && result.thumbnail_file) {
          // Use the generated thumbnail file
          const fileUrl = getFileUrl(`projects/${currentProject.name}/${result.thumbnail_file}`);
          setThumbnailUrl(fileUrl);
          setError(false);
        } else {
          throw new Error(result.error || 'Failed to generate material thumbnail');
        }
      }
    } catch (err) {
      console.error('Failed to get material thumbnail:', err);
      setError(true);
    } finally {
      setIsLoading(false);
    }
  });

  return (
    <Show when={!isLoading()} fallback={
      <div class={`${size} bg-base-300 rounded flex items-center justify-center`}>
        <div class="w-4 h-4 border-2 border-primary border-t-transparent rounded-full animate-spin"></div>
      </div>
    }>
      <Show when={!error() && thumbnailUrl()} fallback={
        <div class={`${size} bg-base-300 rounded flex items-center justify-center`}>
          <IconPalette class="w-10 h-10 text-base-content/40" />
        </div>
      }>
        <div class={`${size} bg-base-300 rounded overflow-hidden`}>
          <img 
            src={thumbnailUrl()}
            alt={`Material: ${asset.name}`}
            class="w-full h-full object-cover"
            style="image-rendering: -webkit-optimize-contrast; image-rendering: crisp-edges;"
          />
        </div>
      </Show>
    </Show>
  );
};

export default MaterialThumbnail;