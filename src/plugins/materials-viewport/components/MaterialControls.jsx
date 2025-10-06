import { IconSettings } from '@tabler/icons-solidjs';
import LightingControls from './LightingControls.jsx';
import EnvironmentControls from './EnvironmentControls.jsx';

export default function MaterialControls(props) {
  const { 
    usePBR, 
    setUsePBR, 
    previewCamera, 
    setCameraDistance, 
    cameraDistance,
    handleAssetDrop,
    handleDragOver,
    lightIntensity,
    setLightIntensity,
    ambientIntensity,
    setAmbientIntensity,
    shadowsEnabled,
    setShadowsEnabled,
    backgroundType,
    setBackgroundType,
    backgroundColor,
    setBackgroundColor,
    hdrBackground,
    setHdrBackground,
    handleHDRFileUpload
  } = props;

  return (
    <div 
      class="flex-1 overflow-y-auto p-4 border-b border-base-300"
      onDrop={handleAssetDrop}
      onDragOver={handleDragOver}
    >
      <h3 class="text-md font-semibold mb-3">Material Preview</h3>

      {/* Material Type */}
      <div class="mb-3">
        <div class="flex items-center justify-between">
          <span class="text-sm font-medium">Material Type</span>
          <div class="flex items-center gap-2">
            <span class="text-xs text-base-content/60">Standard</span>
            <input
              type="checkbox"
              class="toggle toggle-xs"
              checked={usePBR()}
              onChange={(e) => setUsePBR(e.target.checked)}
            />
            <span class="text-xs text-base-content/60">PBR</span>
          </div>
        </div>
      </div>

      {/* Camera Controls */}
      <div class="mb-3">
        <div class="flex items-center justify-between mb-2">
          <button
            class="btn btn-xs btn-ghost"
            onClick={() => {
              if (previewCamera) {
                previewCamera.alpha = Math.PI / 4;
                previewCamera.beta = Math.PI / 3;
                previewCamera.radius = 6;
                setCameraDistance(6);
              }
            }}
            title="Reset Camera"
          >
            <IconSettings class="w-3 h-3" />
          </button>
          <div class="text-xs text-base-content/60">
            Distance: {Math.round((previewCamera?.radius || cameraDistance()) * 10) / 10}
          </div>
        </div>
      </div>

      <LightingControls 
        lightIntensity={lightIntensity}
        setLightIntensity={setLightIntensity}
        ambientIntensity={ambientIntensity}
        setAmbientIntensity={setAmbientIntensity}
        shadowsEnabled={shadowsEnabled}
        setShadowsEnabled={setShadowsEnabled}
      />

      <EnvironmentControls 
        backgroundType={backgroundType}
        setBackgroundType={setBackgroundType}
        backgroundColor={backgroundColor}
        setBackgroundColor={setBackgroundColor}
        hdrBackground={hdrBackground}
        setHdrBackground={setHdrBackground}
        handleHDRFileUpload={handleHDRFileUpload}
      />
    </div>
  );
}