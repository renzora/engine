import { Show } from 'solid-js';
import { IconPhoto, IconX } from '@tabler/icons-solidjs';

export default function EnvironmentControls(props) {
  const {
    backgroundType,
    setBackgroundType,
    backgroundColor,
    setBackgroundColor,
    hdrBackground,
    setHdrBackground,
    handleHDRFileUpload
  } = props;

  return (
    <div class="mb-3">
      <div class="text-sm font-medium mb-2">Environment</div>
      <div class="space-y-2">
        <div class="flex items-center justify-between">
          <span class="text-xs text-base-content/80">Type</span>
          <div class="flex items-center gap-2">
            <button
              class={`btn btn-xs ${backgroundType() === 'color' ? 'btn-primary' : 'btn-ghost'}`}
              onClick={() => setBackgroundType('color')}
            >
              Color
            </button>
            <button
              class={`btn btn-xs ${backgroundType() === 'hdr' ? 'btn-primary' : 'btn-ghost'}`}
              onClick={() => setBackgroundType('hdr')}
            >
              HDR
            </button>
          </div>
        </div>
        
        {/* Color Background */}
        <Show when={backgroundType() === 'color'}>
          <div class="flex items-center justify-between">
            <span class="text-xs text-base-content/80">Color</span>
            <input
              type="color"
              value={backgroundColor()}
              class="w-8 h-6 rounded border border-base-300 cursor-pointer"
              onChange={(e) => setBackgroundColor(e.target.value)}
            />
          </div>
        </Show>
        
        {/* HDR Background */}
        <Show when={backgroundType() === 'hdr'}>
          <div class="space-y-2">
            <div class="flex items-center justify-between">
              <span class="text-xs text-base-content/80">HDR Image</span>
              <div class="flex gap-1">
                <button
                  class="btn btn-xs btn-ghost"
                  onClick={() => document.getElementById('hdr-file-input').click()}
                  title="Upload HDR file"
                >
                  <IconPhoto class="w-3 h-3" />
                </button>
                <button
                  class="btn btn-xs btn-ghost"
                  onClick={() => setHdrBackground(null)}
                  disabled={!hdrBackground()}
                  title="Clear HDR"
                >
                  <IconX class="w-3 h-3" />
                </button>
              </div>
            </div>
            
            {/* Hidden file input */}
            <input
              id="hdr-file-input"
              type="file"
              accept=".hdr,.exr,.dds,.ktx"
              style={{ display: 'none' }}
              onChange={handleHDRFileUpload}
            />
            
            <Show when={hdrBackground()}>
              <div class="text-xs text-base-content/60 bg-base-200 p-2 rounded">
                {hdrBackground().name}
              </div>
            </Show>
            <Show when={!hdrBackground()}>
              <div class="text-xs text-base-content/40 italic text-center p-2 border-2 border-dashed border-base-300 rounded">
                Click 📷 to upload HDR/EXR file
                <br />
                or drag from assets
              </div>
            </Show>
          </div>
        </Show>
      </div>
    </div>
  );
}