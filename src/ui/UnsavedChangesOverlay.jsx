import { createSignal, Show } from 'solid-js';
import { IconAlertTriangle, IconDeviceFloppy, IconTrash, IconX } from '@tabler/icons-solidjs';

export default function UnsavedChangesOverlay({ 
  isOpen, 
  onClose, 
  onSave, 
  onDiscard, 
  projectName,
  changes = [] 
}) {
  const [saving, setSaving] = createSignal(false);

  const handleOverlayClick = (e) => {
    if (e.target === e.currentTarget) {
      onClose();
    }
  };

  const handleSave = async () => {
    if (saving()) return;
    
    setSaving(true);
    try {
      await onSave();
      onClose();
    } catch (error) {
      console.error('Failed to save changes:', error);
      // Keep overlay open on error
    } finally {
      setSaving(false);
    }
  };

  const handleDiscard = () => {
    onDiscard();
    onClose();
  };

  return (
    <Show when={isOpen()}>
      <div 
        className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50"
        onClick={handleOverlayClick}
      >
        <div className="bg-base-100 rounded-lg shadow-2xl border border-base-300 w-full max-w-md mx-4 animate-in zoom-in-95 duration-200">
          {/* Header */}
          <div className="flex items-center justify-between p-6 border-b border-base-300">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-warning/20 rounded-lg">
                <IconAlertTriangle className="w-5 h-5 text-warning" />
              </div>
              <div>
                <h2 className="text-lg font-semibold text-base-content">
                  Unsaved Changes
                </h2>
                <p className="text-sm text-base-content/70">
                  Project: {projectName || 'Current Project'}
                </p>
              </div>
            </div>
            <button
              onClick={onClose}
              className="btn btn-ghost btn-sm btn-circle"
              disabled={saving()}
            >
              <IconX className="w-4 h-4" />
            </button>
          </div>

          {/* Content */}
          <div className="p-6">
            <p className="text-base-content/80 mb-4">
              You have unsaved changes that will be lost if you continue without saving.
            </p>

            {/* Show specific changes if provided */}
            <Show when={changes.length > 0}>
              <div className="mb-4">
                <p className="text-sm font-medium text-base-content/70 mb-2">
                  Changes to save:
                </p>
                <div className="bg-base-200 rounded-lg p-3 max-h-32 overflow-y-auto">
                  <ul className="text-sm space-y-1">
                    {changes.map((change, index) => (
                      <li key={index} className="flex items-center gap-2 text-base-content/70">
                        <div className="w-1 h-1 bg-primary rounded-full flex-shrink-0"></div>
                        {change}
                      </li>
                    ))}
                  </ul>
                </div>
              </div>
            </Show>

            <p className="text-sm text-base-content/60">
              What would you like to do?
            </p>
          </div>

          {/* Actions */}
          <div className="flex gap-3 p-6 pt-0">
            <button
              onClick={handleSave}
              disabled={saving()}
              className="btn btn-primary flex-1"
            >
              <Show when={saving()} fallback={<IconDeviceFloppy className="w-4 h-4" />}>
                <span className="loading loading-spinner loading-sm"></span>
              </Show>
              {saving() ? 'Saving...' : 'Save Changes'}
            </button>
            
            <button
              onClick={handleDiscard}
              disabled={saving()}
              className="btn btn-ghost flex-1"
            >
              <IconTrash className="w-4 h-4" />
              Discard Changes
            </button>
          </div>
        </div>
      </div>
    </Show>
  );
}