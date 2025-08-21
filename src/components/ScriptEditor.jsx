import { createSignal, onMount, createEffect, onCleanup, For, Show } from 'solid-js';
import Editor from '@monaco-editor/react';
import { scriptEditorStore, scriptEditorActions } from '../layout/stores/ScriptEditorStore.js';
import { getCurrentProject } from '@/api/bridge/projects';
import { readFile, writeFile } from '@/api/bridge/files';

// RenScript language definition for Monaco
const RENSCRIPT_LANGUAGE_ID = 'renscript';

const setupRenScriptLanguage = (monaco) => {
  // Register the language
  monaco.languages.register({ id: RENSCRIPT_LANGUAGE_ID });

  // Define tokens for syntax highlighting
  monaco.languages.setMonarchTokensProvider(RENSCRIPT_LANGUAGE_ID, {
    keywords: [
      'script', 'camera', 'light', 'mesh', 'scene', 'transform',
      'props', 'start', 'update', 'destroy', 'on_collision', 'on_trigger',
      'if', 'else', 'for', 'while', 'return', 'break', 'continue',
      'true', 'false', 'null'
    ],
    
    typeKeywords: [
      'boolean', 'float', 'number', 'string', 'vector3', 'color'
    ],
    
    operators: [
      '=', '+', '-', '*', '/', '==', '!=', '<', '>', '<=', '>=',
      '&&', '||', '!'
    ],
    
    builtinFunctions: [
      'get_position', 'set_position', 'get_rotation', 'set_rotation',
      'get_scale', 'set_scale', 'move_by', 'rotate_by', 'look_at',
      'set_color', 'get_color', 'animate', 'stop_animation',
      'clone_object', 'dispose_object', 'set_metadata', 'get_metadata',
      'add_tag', 'remove_tag', 'has_tag', 'find_object_by_name',
      'find_objects_by_tag', 'raycast', 'get_objects_in_radius',
      'play_sound', 'stop_sound', 'set_sound_volume',
      'is_key_pressed', 'is_mouse_button_pressed', 'get_mouse_position',
      'get_gamepads', 'get_left_stick', 'get_right_stick',
      'get_left_stick_x', 'get_left_stick_y', 'get_right_stick_x', 'get_right_stick_y',
      'is_gamepad_button_pressed', 'get_gamepad_trigger',
      'get_time', 'get_delta_time', 'log',
      'sin', 'cos', 'tan', 'abs', 'sqrt', 'pow', 'min', 'max',
      'floor', 'ceil', 'round', 'random', 'clamp', 'lerp',
      'to_radians', 'to_degrees', 'distance', 'normalize', 'dot', 'cross',
      'is_camera', 'is_light', 'is_mesh',
      'detach_camera_controls', 'attach_camera_controls', 'set_camera_target',
      'set_light_intensity', 'set_light_color', 'get_light_intensity', 'get_light_color'
    ],

    // Tokenizer
    tokenizer: {
      root: [
        // Comments
        [/#.*$/, 'comment'],
        
        // Script declarations
        [/\b(script|camera|light|mesh|scene|transform)\s+\w+/, 'keyword.declaration'],
        
        // Keywords
        [/\b(props|start|update|destroy|on_collision|on_trigger)\b/, 'keyword.lifecycle'],
        
        // Identifiers and keywords
        [/[a-z_$][\w$]*/, {
          cases: {
            '@keywords': 'keyword',
            '@typeKeywords': 'type',
            '@builtinFunctions': 'support.function',
            '@default': 'identifier'
          }
        }],
        
        // Numbers
        [/\b\d+\.\d+\b/, 'number.float'],
        [/\b\d+\b/, 'number'],
        
        // Strings
        [/"([^"\\]|\\.)*$/, 'string.invalid'],
        [/'([^'\\]|\\.)*$/, 'string.invalid'],
        [/"/, 'string', '@string_double'],
        [/'/, 'string', '@string_single'],
        
        // Operators
        [/[{}()\[\]]/, 'delimiter.bracket'],
        [/[<>](?!@symbols)/, 'delimiter.bracket'],
        [/@symbols/, {
          cases: {
            '@operators': 'operator',
            '@default': ''
          }
        }],
        
        // Delimiters
        [/[;,.]/, 'delimiter'],
      ],
      
      string_double: [
        [/[^\\"]+/, 'string'],
        [/\\./, 'string.escape'],
        [/"/, 'string', '@pop']
      ],
      
      string_single: [
        [/[^\\']+/, 'string'],
        [/\\./, 'string.escape'],
        [/'/, 'string', '@pop']
      ],
    }
  });

  // Define the theme
  monaco.editor.defineTheme('renscript-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'keyword', foreground: '569cd6' },
      { token: 'keyword.declaration', foreground: 'c586c0' },
      { token: 'keyword.lifecycle', foreground: 'dcdcaa' },
      { token: 'type', foreground: '4ec9b0' },
      { token: 'support.function', foreground: 'dcdcaa' },
      { token: 'string', foreground: 'ce9178' },
      { token: 'number', foreground: 'b5cea8' },
      { token: 'number.float', foreground: 'b5cea8' },
      { token: 'comment', foreground: '6a9955' },
      { token: 'operator', foreground: 'd4d4d4' },
      { token: 'delimiter', foreground: 'd4d4d4' },
      { token: 'delimiter.bracket', foreground: 'ffd700' },
    ],
    colors: {
      'editor.background': '#1e1e1e',
      'editor.foreground': '#d4d4d4',
      'editor.lineHighlightBackground': '#2a2a2a',
      'editorLineNumber.foreground': '#858585',
      'editorCursor.foreground': '#aeafad',
      'editor.selectionBackground': '#264f78',
      'editor.inactiveSelectionBackground': '#3a3d41',
    }
  });

  // Configure language features
  monaco.languages.setLanguageConfiguration(RENSCRIPT_LANGUAGE_ID, {
    comments: {
      lineComment: '#',
    },
    brackets: [
      ['{', '}'],
      ['[', ']'],
      ['(', ')']
    ],
    autoClosingPairs: [
      { open: '{', close: '}' },
      { open: '[', close: ']' },
      { open: '(', close: ')' },
      { open: '"', close: '"' },
      { open: "'", close: "'" },
    ],
    surroundingPairs: [
      { open: '{', close: '}' },
      { open: '[', close: ']' },
      { open: '(', close: ')' },
      { open: '"', close: '"' },
      { open: "'", close: "'" },
    ],
  });
};

function ScriptEditor() {
  const [isSaving, setIsSaving] = createSignal(false);
  let editorRef = null;
  let monacoRef = null;

  const activeScript = () => scriptEditorStore.activeScript;
  const openScripts = () => Array.from(scriptEditorStore.openScripts.entries());
  const currentScriptData = () => {
    const active = activeScript();
    return active ? scriptEditorStore.openScripts.get(active) : null;
  };

  // Load script content when active script changes
  createEffect(() => {
    const active = activeScript();
    if (active && currentScriptData() && !currentScriptData().content) {
      loadScript(active);
    }
  });

  const loadScript = async (filePath) => {
    try {
      const currentProject = getCurrentProject();
      if (!currentProject) {
        console.error('No project loaded');
        return;
      }

      console.log('Loading script:', filePath);
      console.log('Current project:', currentProject.name);

      // Use the bridge API
      const fullPath = `projects/${currentProject.name}/${filePath}`;
      const content = await readFile(fullPath);
      scriptEditorActions.updateScriptContent(filePath, content, false);
    } catch (error) {
      console.error('Error loading script:', error);
    }
  };

  const saveScript = async () => {
    const active = activeScript();
    const scriptData = currentScriptData();
    if (!active || !scriptData?.isDirty) return;
    
    setIsSaving(true);
    try {
      const currentProject = getCurrentProject();
      if (!currentProject) {
        console.error('No project loaded');
        return;
      }

      // Construct the full path for writing
      // active already includes 'assets/', so we use it directly under the project
      const fullPath = `projects/${currentProject.name}/${active}`;
      await writeFile(fullPath, scriptData.content);
      scriptEditorActions.markScriptSaved(active);
    } catch (error) {
      console.error('Error saving script:', error);
    } finally {
      setIsSaving(false);
    }
  };

  const handleEditorDidMount = (editor, monaco) => {
    editorRef = editor;
    monacoRef = monaco;
    
    // Setup RenScript language if not already done
    if (!monaco.languages.getLanguages().some(lang => lang.id === RENSCRIPT_LANGUAGE_ID)) {
      setupRenScriptLanguage(monaco);
    }
    
    // Set theme
    monaco.editor.setTheme('renscript-dark');
    
    // Add keyboard shortcuts
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
      saveScript();
    });
  };

  const handleEditorChange = (value) => {
    const active = activeScript();
    if (active) {
      scriptEditorActions.updateScriptContent(active, value || '', true);
    }
  };

  // Auto-save on Ctrl+S
  onMount(() => {
    const handleKeyDown = (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 's') {
        e.preventDefault();
        saveScript();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    onCleanup(() => document.removeEventListener('keydown', handleKeyDown));
  });

  if (!scriptEditorStore.isVisible || openScripts().length === 0) {
    return (
      <div class="flex items-center justify-center h-full bg-base-200 text-base-content/50">
        <div class="text-center">
          <div class="text-4xl mb-2">📝</div>
          <div class="text-lg mb-1">No scripts open</div>
          <div class="text-sm">Double-click a .ren file to start editing</div>
        </div>
      </div>
    );
  }

  return (
    <div class="flex flex-col h-full bg-base-200">
        {/* Tab bar */}
        <Show when={openScripts().length > 1}>
          <div class="flex bg-base-300 border-b border-base-content/10 overflow-x-auto">
            <For each={openScripts()}>
              {([filePath, scriptData]) => (
                <button
                  class={`px-3 py-2 text-sm border-r border-base-content/10 flex items-center gap-2 hover:bg-base-content/10 ${
                    activeScript() === filePath ? 'bg-base-200 text-base-content' : 'text-base-content/70'
                  }`}
                  onClick={() => scriptEditorActions.setActiveScript(filePath)}
                >
                  <span>{scriptData.fileName}</span>
                  <Show when={scriptData.isDirty}>
                    <div class="w-2 h-2 rounded-full bg-warning"></div>
                  </Show>
                  <span
                    class="ml-1 hover:text-error cursor-pointer"
                    onClick={(e) => {
                      e.stopPropagation();
                      scriptEditorActions.closeScript(filePath);
                    }}
                  >
                    ✕
                  </span>
                </button>
              )}
            </For>
          </div>
        </Show>

        {/* Toolbar */}
        <div class="flex items-center justify-between p-2 bg-base-300 border-b border-base-content/10">
          <div class="flex items-center gap-2">
            <span class="text-sm font-semibold text-base-content">
              {currentScriptData()?.fileName || 'Script Editor'}
            </span>
            <Show when={currentScriptData()?.isDirty}>
              <span class="badge badge-warning badge-xs">Modified</span>
            </Show>
          </div>
          
          <div class="flex items-center gap-2">
            <Show when={currentScriptData()?.lastSaved}>
              <span class="text-xs text-base-content/50">
                Last saved: {currentScriptData()?.lastSaved}
              </span>
            </Show>
            
            <button
              class={`btn btn-primary btn-xs ${isSaving() ? 'loading' : ''}`}
              onClick={saveScript}
              disabled={!currentScriptData()?.isDirty || isSaving()}
            >
              {isSaving() ? 'Saving...' : 'Save (Ctrl+S)'}
            </button>
            
            <button
              class="btn btn-ghost btn-xs"
              onClick={() => scriptEditorActions.hideEditor()}
            >
              ✕
            </button>
          </div>
        </div>

        {/* Editor */}
        <div class="flex-1 overflow-hidden">
          <Editor
            height="100%"
            defaultLanguage={RENSCRIPT_LANGUAGE_ID}
            value={currentScriptData()?.content || ''}
            onChange={handleEditorChange}
            onMount={handleEditorDidMount}
            options={{
              minimap: { enabled: false },
              fontSize: 14,
              lineNumbers: 'on',
              roundedSelection: false,
              scrollBeyondLastLine: false,
              readOnly: false,
              automaticLayout: true,
              tabSize: 2,
              wordWrap: 'on',
              folding: true,
              foldingStrategy: 'indentation',
              showFoldingControls: 'mouseover',
              scrollbar: {
                vertical: 'auto',
                horizontal: 'auto',
                useShadows: false,
                verticalScrollbarSize: 10,
                horizontalScrollbarSize: 10,
              },
              padding: {
                top: 10,
                bottom: 10,
              },
            }}
          />
        </div>

        {/* Status bar */}
        <div class="flex items-center justify-between px-2 py-1 bg-base-300 border-t border-base-content/10 text-xs">
          <span class="text-base-content/50">
            RenScript • Line {editorRef?.getPosition()?.lineNumber || 1}, Column {editorRef?.getPosition()?.column || 1}
          </span>
          <span class="text-base-content/50">
            {(currentScriptData()?.content || '').split('\n').length} lines
          </span>
        </div>
      </div>
    );
}

export default ScriptEditor;