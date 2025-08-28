import { createSignal, For } from 'solid-js';
import { renderStore, renderActions } from '@/render/store.jsx';
import { Grid, Close } from '@/ui/icons';

export default function GizmoDropdownContent() {
  const [snapEnabled, setSnapEnabled] = createSignal(true);
  const [snapAmount, setSnapAmount] = createSignal(1);
  
  const snapPresets = [
    { value: 0.5, label: '0.5' },
    { value: 1, label: '1' },
    { value: 2, label: '2' },
    { value: 3, label: '3' },
    { value: 4, label: '4' },
    { value: 5, label: '5' }
  ];

  const handleSnapToggle = () => {
    const enabled = !snapEnabled();
    setSnapEnabled(enabled);
    
    // Apply to gizmo manager if available
    const gizmoManager = renderStore.gizmoManager;
    if (gizmoManager) {
      // Enable/disable snapping on all gizmos
      if (gizmoManager.gizmos.positionGizmo) {
        gizmoManager.gizmos.positionGizmo.snapDistance = enabled ? snapAmount() : 0;
      }
      if (gizmoManager.gizmos.rotationGizmo) {
        gizmoManager.gizmos.rotationGizmo.snapDistance = enabled ? Math.PI / 12 : 0; // 15 degrees
      }
      if (gizmoManager.gizmos.scaleGizmo) {
        gizmoManager.gizmos.scaleGizmo.snapDistance = enabled ? 0.1 : 0;
      }
    }
    
    console.log(`🎯 Gizmo snapping ${enabled ? 'enabled' : 'disabled'}`);
  };

  const handleSnapAmountChange = (amount) => {
    setSnapAmount(amount);
    
    // Apply to gizmo manager if snapping is enabled
    if (snapEnabled()) {
      const gizmoManager = renderStore.gizmoManager;
      if (gizmoManager && gizmoManager.gizmos.positionGizmo) {
        gizmoManager.gizmos.positionGizmo.snapDistance = amount;
      }
    }
    
    console.log(`🎯 Gizmo snap amount set to ${amount}`);
  };

  return (
    <div class="w-56 space-y-4 p-4 bg-base-200 text-base-content">
      <div>
        <label class="block font-medium text-base-content mb-2">
          Grid Snapping
        </label>
        
        <div class="flex items-center gap-2 mb-3">
          <button
            onClick={handleSnapToggle}
            class={`btn btn-sm flex items-center gap-2 ${
              snapEnabled() ? 'btn-primary' : 'btn-ghost'
            }`}
          >
            {snapEnabled() ? (
              <Grid class="w-4 h-4" />
            ) : (
              <Close class="w-4 h-4" />
            )}
            <span>{snapEnabled() ? 'Snap On' : 'Snap Off'}</span>
          </button>
        </div>
        
        <div class={`space-y-3 ${!snapEnabled() ? 'opacity-50' : ''}`}>
          <div>
            <label class="block text-sm text-base-content/80 mb-2">
              Snap Amount: {snapAmount()}
            </label>
            
            <div class="grid grid-cols-6 gap-1 mb-2">
              <For each={snapPresets}>
                {(preset) => (
                  <button
                    onClick={() => handleSnapAmountChange(preset.value)}
                    disabled={!snapEnabled()}
                    class={`btn btn-xs ${
                      snapAmount() === preset.value && snapEnabled()
                        ? 'btn-primary'
                        : 'btn-ghost'
                    }`}
                  >
                    {preset.label}
                  </button>
                )}
              </For>
            </div>
            
            <input
              type="range"
              min="0.5"
              max="5"
              step="0.5"
              value={snapAmount()}
              onInput={(e) => handleSnapAmountChange(parseFloat(e.target.value))}
              disabled={!snapEnabled()}
              class="range range-primary w-full"
            />
          </div>
        </div>
      </div>
      
      <div class="text-xs text-base-content/60 space-y-1">
        <div>• Position gizmo snaps to grid</div>
        <div>• Rotation gizmo snaps to 15° increments</div>
        <div>• Scale gizmo snaps to 0.1 increments</div>
      </div>
    </div>
  );
}