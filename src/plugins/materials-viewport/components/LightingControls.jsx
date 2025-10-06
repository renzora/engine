export default function LightingControls(props) {
  const {
    lightIntensity,
    setLightIntensity,
    ambientIntensity,
    setAmbientIntensity,
    shadowsEnabled,
    setShadowsEnabled
  } = props;

  return (
    <div class="mb-3">
      <div class="text-sm font-medium mb-2">Lighting</div>
      <div class="space-y-2">
        <div class="flex items-center justify-between">
          <span class="text-xs text-base-content/80">Directional</span>
          <input
            type="range"
            min="0"
            max="2"
            step="0.1"
            value={lightIntensity()}
            class="range range-xs w-20"
            onChange={(e) => setLightIntensity(parseFloat(e.target.value))}
          />
          <span class="text-xs text-base-content/60 w-8 text-right">{lightIntensity().toFixed(1)}</span>
        </div>
        <div class="flex items-center justify-between">
          <span class="text-xs text-base-content/80">Ambient</span>
          <input
            type="range"
            min="0"
            max="1"
            step="0.1"
            value={ambientIntensity()}
            class="range range-xs w-20"
            onChange={(e) => setAmbientIntensity(parseFloat(e.target.value))}
          />
          <span class="text-xs text-base-content/60 w-8 text-right">{ambientIntensity().toFixed(1)}</span>
        </div>
        <div class="flex items-center justify-between">
          <span class="text-xs text-base-content/80">Shadows</span>
          <input
            type="checkbox"
            class="toggle toggle-xs"
            checked={shadowsEnabled()}
            onChange={(e) => setShadowsEnabled(e.target.checked)}
          />
        </div>
      </div>
    </div>
  );
}