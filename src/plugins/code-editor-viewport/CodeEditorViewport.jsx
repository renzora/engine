import { createSignal, createEffect, onCleanup, Show, For } from 'solid-js';
import { IconX, IconDeviceFloppy, IconFileText, IconCode } from '@tabler/icons-solidjs';
import MonacoEditor from '@/components/MonacoEditor';
import { readFile, writeFile, deleteFile } from '@/api/bridge/files';
import { getCurrentProject } from '@/api/bridge/projects';
import { getScriptRuntime } from '@/api/script';
import { editorStore } from '@/layout/stores/EditorStore';

function CodeEditorViewport({ 
  tab = null,
  onClose
}) {
  const [editorValue, setEditorValue] = createSignal('');
  const [loading, setLoading] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal(null);
  const [hasChanges, setHasChanges] = createSignal(false);
  const [originalValue, setOriginalValue] = createSignal('');
  const [fileName, setFileName] = createSignal('untitled.ren');
  const [originalFileName, setOriginalFileName] = createSignal('');
  const [selectedFile, setSelectedFile] = createSignal(tab?.initialFile || null);
  const [parsedScript, setParsedScript] = createSignal(null);
  const [parseError, setParseError] = createSignal(null);
  const [parseErrors, setParseErrors] = createSignal([]); // Array of errors with line numbers
  const [previousProperties, setPreviousProperties] = createSignal([]);
  const [previousScriptContent, setPreviousScriptContent] = createSignal(''); // Track full script content

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

  // Direct parsing - no debounce needed with efficient signals
  const parseScriptImmediately = (content) => {
    parseRenScript(content);
  };

  // Parse RenScript content and extract properties
  const parseRenScript = (content) => {
    if (!fileName().endsWith('.ren')) {
      setParsedScript(null);
      setParseError(null);
      return;
    }
    
    // Handle empty or whitespace-only content
    if (!content || content.trim() === '') {
      console.log('🗑️ Empty script content detected, triggering script removal');
      setParsedScript({
        type: 'Script',
        name: 'EmptyScript',
        objectType: 'script',
        properties: [],
        variables: [],
        methods: [],
        errors: [],
        isEmpty: true
      });
      setParseError(null);
      setParseErrors([]);
      return;
    }

    try {
      // Server-side compilation is now used - client-side compiler removed
      
      // We need to extract the AST data. Since compile() returns JavaScript code,
      // we'll need to parse the source code directly to extract properties.
      // Let's create a simple property extractor for now
      const ast = extractPropertiesFromRenScript(content);
      
      // Validate the AST
      if (!ast || typeof ast !== 'object') {
        throw new Error('Invalid script structure');
      }
      
      // Validate properties
      if (ast.properties) {
        ast.properties.forEach((prop, index) => {
          if (!prop.name || typeof prop.name !== 'string') {
            throw new Error(`Property at index ${index} missing valid name`);
          }
          if (!prop.propType || typeof prop.propType !== 'string') {
            throw new Error(`Property '${prop.name}' missing valid type`);
          }
        });
      }
      
      setParsedScript(ast);
      
      // Set errors from parsing
      if (ast.errors && ast.errors.length > 0) {
        setParseErrors(ast.errors);
        setParseError(`${ast.errors.length} error(s) found`);
      } else {
        setParseErrors([]);
        setParseError(null);
      }
      
      // Trigger property updates for live scripts or handle empty scripts
      if (ast && (ast.properties || ast.isEmpty)) {
        updateLiveScriptProperties(ast);
      }
    } catch (error) {
      console.warn('RenScript parsing failed:', error);
      setParseError(error.message);
      setParseErrors([{
        line: 1,
        column: 1,
        message: error.message,
        severity: 'error'
      }]);
      setParsedScript(null);
    }
  };

  // Enhanced property extractor with better error handling and line tracking
  const extractPropertiesFromRenScript = (content) => {
    const properties = [];
    const errors = [];
    let scriptName = 'Script';
    let objectType = 'script';
    
    // Helper function to get line number for a position in content
    const getLineNumber = (position) => {
      return content.substring(0, position).split('\n').length;
    };
    
    try {
      // Extract script name and type with better validation
      const scriptMatch = content.match(/(script|camera|light|mesh|scene|transform)\s+([a-zA-Z_][a-zA-Z0-9_]*)/);
      if (scriptMatch) {
        objectType = scriptMatch[1];
        scriptName = scriptMatch[2];
        
        // Validate script name
        if (!/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(scriptName)) {
          const lineNum = getLineNumber(scriptMatch.index);
          errors.push({
            line: lineNum,
            column: scriptMatch.index - content.lastIndexOf('\n', scriptMatch.index) - 1,
            message: `Invalid script name '${scriptName}'. Script names must start with a letter or underscore.`,
            severity: 'error'
          });
        }
      }
      
      return {
        type: 'Script',
        name: scriptName,
        objectType,
        properties,
        variables: [], // Could extract these too
        methods: [], // Could extract these too
        errors: errors // Include parsing errors
      };
      
    } catch (extractError) {
      // Add the general error to the errors array
      errors.push({
        line: 1,
        column: 1,
        message: `Property extraction failed: ${extractError.message}`,
        severity: 'error'
      });
      
      return {
        type: 'Script',
        name: scriptName,
        objectType,
        properties,
        variables: [],
        methods: [],
        errors: errors
      };
    }
  };

  // Update properties on live script instances and handle code changes
  const updateLiveScriptProperties = (ast) => {
    const runtime = getScriptRuntime();
    const currentFile = selectedFile();
    
    if (!runtime || !currentFile) return;

    // Check if the script is now empty - remove it completely
    if (ast.isEmpty) {
      console.log('🗑️ Empty script detected, removing script from all objects', currentFile.path);
      triggerScriptRemoval(currentFile.path);
      return;
    }

    // Dispatch a custom event that the object properties system can listen to
    document.dispatchEvent(new CustomEvent('engine:script-properties-updated', {
      detail: { 
        scriptPath: currentFile.path,
        properties: ast.properties || [],
        scriptName: ast.name,
        objectType: ast.objectType
      }
    }));
  };

  // Trigger full script reload when code changes
  const triggerScriptReload = async (scriptPath, newContent) => {
    try {
      const runtime = getScriptRuntime();
      const currentProject = getCurrentProject();
      
      if (!runtime || !currentProject) return;
      
      console.log('🔄 Triggering script reload for', scriptPath);
      
      // Save the file first
      const filePath = `projects/${currentProject.name}/${scriptPath}`;
      await writeFile(filePath, newContent);
      
      // Use the runtime's reload method to properly reload the script
      await runtime.reloadScript(scriptPath);
      
      console.log('✅ Script reloaded successfully');
      
      // Dispatch event to notify UI components that script was fully reloaded
      document.dispatchEvent(new CustomEvent('engine:script-reloaded', {
        detail: { 
          scriptPath,
          action: 'full_reload'
        }
      }));
      
    } catch (error) {
      console.error('❌ Failed to reload script:', error);
    }
  };

  // Remove script from all objects when script becomes empty
  const triggerScriptRemoval = async (scriptPath) => {
    try {
      const runtime = getScriptRuntime();
      const currentProject = getCurrentProject();
      
      if (!runtime || !currentProject) return;
      
      console.log('🗑️ Removing script from all objects:', scriptPath);
      
      // Save empty file (or delete it)
      const filePath = `projects/${currentProject.name}/${scriptPath}`;
      await writeFile(filePath, '// Empty script file\n');
      
      // Dispatch event to notify UI that script was removed
      document.dispatchEvent(new CustomEvent('engine:script-removed', {
        detail: { 
          scriptPath,
          action: 'script_removed'
        }
      }));
      
      console.log('✅ Script removed successfully');
      
    } catch (error) {
      console.error('❌ Failed to remove script:', error);
    }
  };

  const loadFile = async (file) => {
    if (!file) {
      // No file selected - start with empty editor
      setFileName('untitled.ren');
      setOriginalFileName('');
      setEditorValue('');
      setOriginalValue('');
      setHasChanges(false);
      setParsedScript(null);
      setPreviousProperties([]);
      setParseError(null);
      setParseErrors([]);
      setPreviousScriptContent('');
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
      
      // Initialize property tracking for .ren files
      if (file.name.endsWith('.ren')) {
        const ast = extractPropertiesFromRenScript(content);
        setParsedScript(ast);
        setPreviousProperties(ast.properties || []);
        setPreviousScriptContent(content); // Initialize script content tracking
        
        // Set errors from parsing
        if (ast.errors && ast.errors.length > 0) {
          setParseErrors(ast.errors);
          setParseError(`${ast.errors.length} error(s) found`);
        } else {
          setParseErrors([]);
          setParseError(null);
        }
      }
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

  // Debounce timeout for script parsing
  let parseTimeout = null;
  
  // Cleanup timeout on component unmount
  onCleanup(() => {
    if (parseTimeout) {
      clearTimeout(parseTimeout);
    }
  });
  
  const handleEditorChange = (value) => {
    setEditorValue(value);
    updateHasChanges(value, fileName());
    
    // Add debounced parsing for .ren files to prevent script reload on every keystroke
    if (fileName().endsWith('.ren')) {
      if (parseTimeout) {
        clearTimeout(parseTimeout);
      }
      
      const debounceMs = editorStore.settings.editor.scriptReloadDebounceMs || 500;
      parseTimeout = setTimeout(() => {
        parseScriptImmediately(value);
      }, debounceMs);
    }
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
        if (onClose) onClose();
      }
    } else {
      if (onClose) onClose();
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
    const file = selectedFile();
    loadFile(file);
  });

  // Add keyboard event listener
  createEffect(() => {
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  });

  // Listen for external file changes
  createEffect(() => {
    const handleExternalFileChange = (event) => {
      const { file } = event.detail;
      if (file) {
        setSelectedFile(file);
      }
    };

    document.addEventListener('engine:open-code-editor', handleExternalFileChange);
    return () => document.removeEventListener('engine:open-code-editor', handleExternalFileChange);
  });

  // Initialize with tab data
  createEffect(() => {
    if (tab?.initialFile) {
      setSelectedFile(tab.initialFile);
    }
  });

  // Ensure Monaco Editor responds to layout changes
  createEffect(() => {
    // Add a small delay to ensure layout has settled
    const timer = setTimeout(() => {
      // Trigger Monaco layout update when the viewport size changes
      window.dispatchEvent(new Event('resize'));
    }, 100);
    
    return () => clearTimeout(timer);
  });

  return (
    <div class="h-full w-full flex flex-col bg-base-100 relative">

        {/* Content */}
        <div class="flex-1 flex flex-col min-h-0">
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
            <div class="flex-1 flex flex-col min-h-0">
              <div class="flex-1 min-h-0 relative">
                <MonacoEditor
                  value={editorValue()}
                  onChange={handleEditorChange}
                  language={getLanguageFromExtension(fileName())}
                  theme="vs-dark"
                  height="100%"
                  width="100%"
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
              
              {/* Error Panel */}
              <Show when={fileName().endsWith('.ren') && parseErrors().length > 0}>
                <div class="border-t border-base-300 bg-error/5 max-h-32 overflow-y-auto">
                  <div class="p-2">
                    <div class="text-xs font-medium text-error mb-2 flex items-center">
                      <span class="w-2 h-2 bg-error rounded-full mr-2"></span>
                      RenScript Errors ({parseErrors().length})
                    </div>
                    <For each={parseErrors()}>
                      {(error) => (
                        <div class="mb-2 last:mb-0">
                          <div class="flex items-start gap-2 text-xs">
                            <span class={`px-1.5 py-0.5 rounded text-xs font-mono ${
                              error.severity === 'error' 
                                ? 'bg-error text-error-content' 
                                : 'bg-warning text-warning-content'
                            }`}>
                              {error.line}:{error.column}
                            </span>
                            <div class="flex-1">
                              <div class={`${error.severity === 'error' ? 'text-error' : 'text-warning'} font-medium`}>
                                {error.message}
                              </div>
                              <Show when={error.suggestion}>
                                <div class="text-info mt-1">
                                  💡 Try: {error.suggestion}
                                </div>
                              </Show>
                            </div>
                          </div>
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              </Show>
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
              <Show when={fileName().endsWith('.ren') && parsedScript()}>
                <span class="text-success">
                  • {parsedScript().properties?.length || 0} props
                </span>
              </Show>
              <Show when={fileName().endsWith('.ren') && parseErrors().length > 0}>
                <span class={`${parseErrors().some(e => e.severity === 'error') ? 'text-error' : 'text-warning'}`}>
                  • {parseErrors().filter(e => e.severity === 'error').length} errors, {parseErrors().filter(e => e.severity === 'warning').length} warnings
                </span>
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

export default CodeEditorViewport;