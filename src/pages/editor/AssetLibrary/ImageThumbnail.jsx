import { Show, createSignal, createMemo } from 'solid-js';
import { IconPhoto } from '@tabler/icons-solidjs';
import { getFileUrl } from '@/api/bridge/files';
import { getCurrentProject } from '@/api/bridge/projects';

const isImageFile = (extension) => {
  const imageExtensions = [
    '.jpg', '.jpeg', '.png', '.gif', '.webp', '.bmp', '.tga', '.tiff', '.ico', '.svg',
    '.avif', '.heic', '.heif', '.dds', '.exr', '.hdr', '.psd', '.raw', '.cr2', '.nef'
  ];
  return imageExtensions.includes(extension?.toLowerCase() || '');
};

const ImageThumbnail = ({ asset, size = 'w-full h-full' }) => {
  const [imageLoaded, setImageLoaded] = createSignal(false);
  const [imageError, setImageError] = createSignal(false);
  
  // Use direct file URL for images instead of thumbnail generation
  const imageUrl = createMemo(() => {
    if (!isImageFile(asset?.extension)) return null;
    
    const currentProject = getCurrentProject();
    if (!currentProject?.name) return null;
    
    return getFileUrl(`projects/${currentProject.name}/${asset.path}`);
  });
  
  if (!imageUrl()) {
    return (
      <div class={`${size} bg-base-300 flex items-center justify-center`}>
        <IconPhoto class="w-10 h-10 text-base-content/60" />
        <div class="absolute bottom-1 right-1 bg-base-100/90 rounded-sm px-1 py-0.5">
          <span class="text-xs font-semibold text-base-content/80">
            {asset.extension?.replace('.', '').toUpperCase()}
          </span>
        </div>
      </div>
    );
  }
  
  return (
    <div class={`${size} bg-base-300 overflow-hidden relative`}>
      <Show when={!imageError()} fallback={
        <div class="w-full h-full flex items-center justify-center">
          <IconPhoto class="w-10 h-10 text-base-content/60" />
          <div class="absolute bottom-1 right-1 bg-base-100/90 rounded-sm px-1 py-0.5">
            <span class="text-xs font-semibold text-base-content/80">
              {asset.extension?.replace('.', '').toUpperCase()}
            </span>
          </div>
        </div>
      }>
        <img 
          src={imageUrl()}
          alt={asset.name}
          class={`w-full h-full object-cover transition-all duration-300 ${
            imageLoaded() ? 'opacity-100 scale-100' : 'opacity-0 scale-95'
          } hover:scale-105 cursor-pointer`}
          style={{
            "image-rendering": "auto",
            "background-color": "var(--fallback-bc,oklch(var(--bc)/0.2))"
          }}
          onLoad={() => {
            setImageLoaded(true);
            setImageError(false);
          }}
          onError={() => {
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

export default ImageThumbnail;