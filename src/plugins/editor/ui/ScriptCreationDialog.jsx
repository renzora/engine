import { createSignal, createEffect } from 'solid-js';
import { IconFileText, IconX, IconPlus } from '@tabler/icons-solidjs';

const ScriptCreationDialog = (props) => {
  const [scriptName, setScriptName] = createSignal('');
  const [isLoading, setIsLoading] = createSignal(false);

  createEffect(() => {
    if (props.isOpen) {
      setScriptName('');
      setIsLoading(false);
    }
  });

  const handleSubmit = async (e) => {
    e.preventDefault();
    
    if (!scriptName().trim()) {
      return;
    }

    setIsLoading(true);
    try {
      await props.onConfirm(scriptName().trim());
      props.onClose();
    } catch (error) {
      console.error('Failed to create script:', error);
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyDown = (e) => {
    if (e.key === 'Escape') {
      props.onClose();
    }
  };

  return (
    <>
      {props.isOpen && (
        <div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50">
          <div class="bg-slate-800 rounded-lg shadow-2xl max-w-md w-full mx-4">
            <div class="flex items-center justify-between p-4 border-b border-slate-700">
              <div class="flex items-center gap-3">
                <IconFileText class="w-5 h-5 text-blue-400" />
                <div>
                  <h2 class="text-lg font-semibold text-white">Create Script</h2>
                  <p class="text-sm text-gray-400">Enter a name for your new script</p>
                </div>
              </div>
              <button
                onClick={props.onClose}
                class="p-2 hover:bg-slate-700 rounded transition-colors"
                disabled={isLoading()}
              >
                <IconX class="w-4 h-4 text-gray-400" />
              </button>
            </div>

            <form onSubmit={handleSubmit} class="p-4 space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-300 mb-2">
                  Script Name:
                </label>
                <input
                  type="text"
                  value={scriptName()}
                  onInput={(e) => setScriptName(e.target.value)}
                  onKeyDown={handleKeyDown}
                  placeholder="MyScript"
                  class="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded-lg text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500"
                  disabled={isLoading()}
                  autofocus
                />
                <div class="text-xs text-gray-400 mt-1">
                  Will be saved as "{scriptName() || 'MyScript'}.js" (extension added automatically)
                </div>
              </div>

              <div class="flex items-center justify-end gap-3 pt-2">
                <button
                  type="button"
                  onClick={props.onClose}
                  class="px-4 py-2 text-sm text-gray-300 hover:text-white transition-colors"
                  disabled={isLoading()}
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={!scriptName().trim() || isLoading()}
                  class="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-600 disabled:cursor-not-allowed text-white text-sm rounded-lg transition-colors flex items-center gap-2"
                >
                  {isLoading() ? (
                    <>
                      <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
                      Creating...
                    </>
                  ) : (
                    <>
                      <IconPlus class="w-4 h-4" />
                      Create Script
                    </>
                  )}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </>
  );
};

export default ScriptCreationDialog;