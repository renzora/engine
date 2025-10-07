import { } from 'solid-js';
import { weatherStore, weatherActions } from '@/stores/WeatherStore';
import { IconStars } from '@tabler/icons-solidjs';

export default function StarsPanel() {
  // Note: Stars rendering is now handled by the centralized WeatherRenderer
  // This panel now only provides UI controls for the weather store

  return (
    <div class="h-full flex flex-col">
      <div class="flex-1 p-4">
        <div class="space-y-4">
          <div class="flex items-center justify-between">
            <label class="text-sm font-medium text-base-content flex items-center gap-2">
              <IconStars class="w-4 h-4" />
              Enable Stars
            </label>
            <input
              type="checkbox"
              checked={weatherStore.stars.enabled}
              onChange={(e) => weatherActions.setStarsEnabled(e.target.checked)}
              class="toggle toggle-primary"
            />
          </div>
          
          {weatherStore.stars.enabled && (
            <div class="space-y-3">
              <div>
                <label class="block text-sm text-base-content/80 mb-1">
                  Brightness: {weatherStore.stars.brightness.toFixed(1)}
                </label>
                <input
                  type="range"
                  min={0.1}
                  max={2.0}
                  step={0.1}
                  value={weatherStore.stars.brightness}
                  onInput={(e) => weatherActions.setStarsBrightness(parseFloat(e.target.value))}
                  class="range range-primary w-full range-sm"
                />
              </div>
              
              <div>
                <label class="block text-sm text-base-content/80 mb-1">
                  Star Count: {weatherStore.stars.density}
                </label>
                <input
                  type="range"
                  min={100}
                  max={2000}
                  step={100}
                  value={weatherStore.stars.density}
                  onInput={(e) => weatherActions.setStarsDensity(parseInt(e.target.value))}
                  class="range range-primary w-full range-sm"
                />
              </div>
              
              <div class="flex items-center justify-between">
                <label class="text-sm text-base-content/80">
                  Twinkling Effect
                </label>
                <input
                  type="checkbox"
                  checked={weatherStore.stars.twinkle}
                  onChange={(e) => weatherActions.setStarsTwinkle(e.target.checked)}
                  class="toggle toggle-primary toggle-sm"
                />
              </div>
            </div>
          )}
          
          <p class="text-xs text-base-content/60">
            Control the starry night sky appearance and intensity
          </p>
        </div>
      </div>
    </div>
  );
}