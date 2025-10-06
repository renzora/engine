import { createMemo } from 'solid-js';

export default function TexturePreview(props) {
  const imageSrc = createMemo(() => {
    const asset = props.asset;
    if (!asset) return '';
    
    // Try different thumbnail URL formats based on what we see in the console
    if (asset.thumbnailUrl) {
      return asset.thumbnailUrl;
    } else if (asset.path) {
      // Construct thumbnail URL based on the path pattern we see in console logs
      const pathWithoutExt = asset.path.replace(/\.[^/.]+$/, ""); // Remove extension
      const cleanPath = pathWithoutExt.replace(/[\/\\]/g, '_'); // Replace slashes with underscores
      return `http://localhost:3001/file/projects/test/.cache/thumbnails/${cleanPath}_${asset.extension}_256.png`;
    } else if (asset.name) {
      // Fallback using name
      const nameWithoutExt = asset.name.replace(/\.[^/.]+$/, "");
      const cleanName = nameWithoutExt.replace(/[\/\\]/g, '_');
      return `http://localhost:3001/file/projects/test/.cache/thumbnails/${cleanName}_${asset.extension || 'jpg'}_256.png`;
    }
    
    // Last resort - try the original API format
    return `/api/assets/thumbnail/${asset.id}`;
  });
  
  const imageAlt = createMemo(() => props.asset?.name || 'Texture');
  
  return (
    <div class="relative overflow-hidden rounded border border-base-300 bg-base-200 h-16 mb-2">
      <img 
        src={imageSrc()}
        alt={imageAlt()}
        class="w-full h-full object-cover"
        onLoad={() => {
          console.log('✅ Texture preview loaded:', imageSrc());
        }}
        onError={(e) => {
          console.log('❌ Texture preview failed to load:', imageSrc());
          console.log('Asset data:', props.asset);
          e.target.style.display = 'none';
          e.target.nextElementSibling.style.display = 'flex';
        }}
      />
      <div class="absolute inset-0 bg-base-300 flex items-center justify-center text-base-content/40 text-xs hidden">
        No Preview
      </div>
      <div class="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/80 to-transparent px-1 py-0.5">
        <div class="text-white text-xs truncate">{imageAlt()}</div>
      </div>
    </div>
  );
}