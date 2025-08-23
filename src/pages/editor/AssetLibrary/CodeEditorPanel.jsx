import { createSignal, createEffect, Show } from 'solid-js';
import { X, Save, FileText } from '@/ui/icons';
import MonacoEditor from '@/components/MonacoEditor';
import { readFile, writeFile, deleteFile } from '@/api/bridge/files';
import { getCurrentProject } from '@/api/bridge/projects';

function CodeEditorPanel({ 
  isOpen, 
  onClose, 
  selectedFile, 
  width = 400 
}) {
  const [editorValue, setEditorValue] = createSignal('');
  const [loading, setLoading] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal(null);
  const [hasChanges, setHasChanges] = createSignal(false);
  const [originalValue, setOriginalValue] = createSignal('');
  const [fileName, setFileName] = createSignal('untitled.ren');
  const [originalFileName, setOriginalFileName] = createSignal('');

  const getLanguageFromExtension = (fileName) => {
    const ext = fileName.toLowerCase().split('.').pop();
    const languageMap = {
      'js': 'javascript',
      'jsx': 'javascript',
      'ts': 'typescript',
      'tsx': 'typescript',
      'json': 'json',
      'html': 'html',
      'css': 'css',
      'md': 'markdown',
      'txt': 'plaintext',
      'ren': 'renscript', // Use custom RenScript language
      'xml': 'xml',
      'yaml': 'yaml',
      'yml': 'yaml'
    };
    return languageMap[ext] || 'plaintext';
  };

  const loadFile = async (file) => {
    if (!file) {
      // No file selected - start with empty editor
      setFileName('untitled.ren');
      setOriginalFileName('');
      setEditorValue('');
      setOriginalValue('');
      setHasChanges(false);
      return;
    }
    
    setLoading(true);
    setError(null);
    
    try {
      const currentProject = getCurrentProject();
      if (!currentProject?.name) {
        throw new Error('No project selected');
      }

      const filePath = `projects/${currentProject.name}/${file.path}`;
      const content = await readFile(filePath);
      
      setFileName(file.name);
      setOriginalFileName(file.name);
      setEditorValue(content);
      setOriginalValue(content);
      setHasChanges(false);
    } catch (err) {
      console.error('Failed to load file:', err);
      setError(`Failed to load file: ${err.message}`);
      setEditorValue('');
      setOriginalValue('');
    } finally {
      setLoading(false);
    }
  };

  const saveFile = async () => {
    const currentProject = getCurrentProject();
    if (!currentProject?.name) {
      setError('No project selected');
      return;
    }

    if (!fileName().trim()) {
      setError('File name cannot be empty');
      return;
    }

    setSaving(true);
    setError(null);
    
    try {
      // Determine the file path in the current directory
      // For now, we'll assume we want to save in the root of the project
      // TODO: You might want to get the current directory from the asset library state
      const newFilePath = `projects/${currentProject.name}/${fileName()}`;
      
      // If this is a rename (original file exists and name changed)
      if (originalFileName() && originalFileName() !== fileName() && selectedFile()) {
        // Delete the old file
        const oldFilePath = `projects/${currentProject.name}/${selectedFile().path}`;
        try {
          await deleteFile(oldFilePath);
        } catch (deleteErr) {
          console.warn('Could not delete old file:', deleteErr);
          // Continue anyway - the old file might not exist
        }
      }
      
      // Write the new/updated file
      await writeFile(newFilePath, editorValue());
      
      setOriginalValue(editorValue());
      setOriginalFileName(fileName());
      setHasChanges(false);
      
      // Dispatch event to trigger file change detection and refresh the asset list
      document.dispatchEvent(new CustomEvent('engine:file-saved', {
        detail: { path: newFilePath, content: editorValue() }
      }));
      
    } catch (err) {
      console.error('Failed to save file:', err);
      setError(`Failed to save file: ${err.message}`);
    } finally {
      setSaving(false);
    }
  };

  const handleEditorChange = (value) => {
    setEditorValue(value);
    updateHasChanges(value, fileName());
  };

  const handleFileNameChange = (name) => {
    setFileName(name);
    updateHasChanges(editorValue(), name);
  };

  const updateHasChanges = (content, name) => {
    const contentChanged = content !== originalValue();
    const nameChanged = name !== originalFileName();
    setHasChanges(contentChanged || nameChanged);
  };

  const handleClose = () => {
    if (hasChanges()) {
      if (confirm('You have unsaved changes. Are you sure you want to close?')) {
        onClose();
      }
    } else {
      onClose();
    }
  };

  const handleKeyDown = (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 's') {
      e.preventDefault();
      saveFile();
    }
  };

  // Load file when selectedFile changes or when editor opens
  createEffect(() => {
    if (isOpen()) {
      const file = selectedFile();
      loadFile(file);
    }
  });

  // Add keyboard event listener
  createEffect(() => {
    if (isOpen()) {
      document.addEventListener('keydown', handleKeyDown);
      return () => document.removeEventListener('keydown', handleKeyDown);
    }
  });

  return (
    <div class="h-full flex flex-col bg-base-100">
        {/* Header */}
        <div class="flex items-center justify-between p-3 border-b border-base-300 bg-base-200">
          <div class="flex items-center gap-2 min-w-0 flex-1">
            <FileText class="w-4 h-4 text-primary flex-shrink-0" />
            <input
              type="text"
              value={fileName()}
              onInput={(e) => handleFileNameChange(e.target.value)}
              class="text-sm font-medium bg-transparent border-none outline-none focus:bg-base-100 focus:px-2 focus:py-1 focus:rounded focus:border focus:border-primary/20 transition-all flex-1 min-w-0"
              placeholder="filename.js"
            />
            <Show when={hasChanges()}>
              <div class="w-2 h-2 bg-warning rounded-full flex-shrink-0" title="Unsaved changes" />
            </Show>
          </div>
          
          <div class="flex items-center gap-2">
            <button
              onClick={saveFile}
              disabled={!hasChanges() || saving()}
              class={`px-2 py-1 text-xs rounded transition-colors ${
                hasChanges() && !saving()
                  ? 'bg-primary text-primary-content hover:bg-primary/80'
                  : 'bg-base-300 text-base-content/50 cursor-not-allowed'
              }`}
              title="Save (Ctrl+S)"
            >
              <Show when={saving()} fallback={<Save class="w-3 h-3" />}>
                <div class="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
              </Show>
            </button>
            
            <button
              onClick={handleClose}
              class="px-2 py-1 text-xs rounded bg-base-300 text-base-content/60 hover:text-base-content hover:bg-base-300/80 transition-colors"
              title="Close"
            >
              <X class="w-3 h-3" />
            </button>
          </div>
        </div>

        {/* Content */}
        <div class="flex-1 flex flex-col">
          <Show when={error()}>
            <div class="p-3 bg-error/10 border-b border-error/20">
              <div class="text-sm text-error">{error()}</div>
            </div>
          </Show>

          <Show when={loading()}>
            <div class="flex-1 flex items-center justify-center">
              <div class="flex items-center gap-2 text-base-content/60">
                <div class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
                <span class="text-sm">Loading file...</span>
              </div>
            </div>
          </Show>

          <Show when={!loading()}>
            <div class="flex-1">
              <MonacoEditor
                value={editorValue()}
                onChange={handleEditorChange}
                language={getLanguageFromExtension(fileName())}
                theme="vs-dark"
                height="100%"
                options={{
                  fontSize: 13,
                  lineHeight: 18,
                  minimap: { enabled: false },
                  scrollBeyondLastLine: false,
                  wordWrap: 'on',
                  automaticLayout: true,
                  folding: true,
                  renderWhitespace: 'boundary',
                  tabSize: 2,
                  insertSpaces: true
                }}
              />
            </div>
          </Show>
        </div>

        {/* Status Bar */}
        <div class="px-3 py-2 bg-base-200 border-t border-base-300 text-xs text-base-content/60">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-3">
              <span>{getLanguageFromExtension(fileName()).toUpperCase()}</span>
              <Show when={selectedFile()?.size}>
                <span>{Math.round(selectedFile().size / 1024)}KB</span>
              </Show>
            </div>
            <Show when={hasChanges()}>
              <span class="text-warning">• Modified</span>
            </Show>
          </div>
        </div>
      </div>
  );
}

export default CodeEditorPanel;