import { createSignal, Show } from 'solid-js';
import { IconAlertTriangle, IconDeviceFloppy, IconDoorExit, IconX } from '@tabler/icons-solidjs';

export default function CloseConfirmationOverlay({ 
  isOpen, 
  onClose, 
  onSaveAndClose, 
  onCloseWithoutSaving,
  projectName,
  changes = [] 
}) {
  const [saving, setSaving] = createSignal(false);

  const handleOverlayClick = (e) => {
    if (e.target === e.currentTarget) {
      const callback = typeof onClose === 'function' ? onClose() : onClose;
      if (typeof callback === 'function') {
        callback();
      }
    }
  };

  const handleSaveAndClose = async () => {
    console.log('Save and Close button clicked');
    if (saving()) {
      console.log('Already saving, ignoring click');
      return;
    }
    
    setSaving(true);
    try {
      const callback = typeof onSaveAndClose === 'function' ? onSaveAndClose() : onSaveAndClose;
      if (typeof callback === 'function') {
        console.log('Calling onSaveAndClose function');
        await callback();
        console.log('onSaveAndClose completed successfully');
      } else {
        console.log('onSaveAndClose callback is not a function:', typeof callback);
      }
    } catch (error) {
      console.error('Failed to save and close:', error);
      // Keep overlay open on error
      setSaving(false);
    }
  };

  const handleCloseWithoutSaving = () => {
    console.log('Close Without Saving button clicked');
    const callback = typeof onCloseWithoutSaving === 'function' ? onCloseWithoutSaving() : onCloseWithoutSaving;
    if (typeof callback === 'function') {
      console.log('Calling onCloseWithoutSaving function');
      callback();
    } else {
      console.log('onCloseWithoutSaving callback is not a function:', typeof callback);
    }
  };

  const handleCancel = () => {
    console.log('Cancel button clicked');
    // Cancel should always just close the overlay without doing anything else
    const callback = typeof onClose === 'function' ? onClose() : onClose;
    if (typeof callback === 'function') {
      console.log('Calling onClose function');
      callback();
    } else {
      console.log('onClose callback is not a function, using fallback');
      // Fallback - import and call hide directly
      import('@/stores/CloseConfirmationStore.jsx').then(({ closeConfirmationActions }) => {
        closeConfirmationActions.hide();
      });
    }
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
                  Close Application
                </h2>
                <p className="text-sm text-base-content/70">
                  Project: {typeof projectName === 'function' ? projectName() : projectName || 'Current Project'}
                </p>
              </div>
            </div>
            <button
              onClick={handleCancel}
              className="btn btn-ghost btn-sm btn-circle"
              disabled={saving()}
            >
              <IconX className="w-4 h-4" />
            </button>
          </div>

          {/* Content */}
          <div className="p-6">
            <p className="text-base-content/80 mb-4">
              You have unsaved changes that will be lost if you close without saving.
            </p>

            {/* Show specific changes if provided */}
            <Show when={() => {
              const changesList = typeof changes === 'function' ? changes() : changes;
              return changesList && changesList.length > 0;
            }}>
              <div className="mb-4">
                <p className="text-sm font-medium text-base-content/70 mb-2">
                  Unsaved changes:
                </p>
                <div className="bg-base-200 rounded-lg p-3 max-h-32 overflow-y-auto">
                  <ul className="text-sm space-y-1">
                    {(() => {
                      const changesList = typeof changes === 'function' ? changes() : changes;
                      return changesList?.map((change, index) => (
                        <li key={index} className="flex items-center gap-2 text-base-content/70">
                          <div className="w-1 h-1 bg-primary rounded-full flex-shrink-0"></div>
                          {change}
                        </li>
                      ));
                    })()}
                  </ul>
                </div>
              </div>
            </Show>

            <p className="text-sm text-base-content/60">
              Would you like to save your changes before closing?
            </p>
          </div>

          {/* Actions */}
          <div className="flex flex-col gap-3 p-6 pt-0">
            <button
              onClick={handleSaveAndClose}
              disabled={saving()}
              className="btn btn-primary"
            >
              <Show when={saving()} fallback={<IconDeviceFloppy className="w-4 h-4" />}>
                <span className="loading loading-spinner loading-sm"></span>
              </Show>
              {saving() ? 'Saving and closing...' : 'Save and Close'}
            </button>
            
            <button
              onClick={handleCloseWithoutSaving}
              disabled={saving()}
              className="btn btn-error btn-outline"
            >
              <IconDoorExit className="w-4 h-4" />
              Close Without Saving
            </button>

            <button
              onClick={handleCancel}
              disabled={saving()}
              className="btn btn-ghost"
            >
              Cancel
            </button>
          </div>
        </div>
      </div>
    </Show>
  );
}