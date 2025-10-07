import { createSignal, onMount, onCleanup, createEffect } from 'solid-js';
import loader from '@monaco-editor/loader';
import { keyboardShortcuts } from '@/components/KeyboardShortcuts';

function MonacoEditor({ 
  value, 
  onChange, 
  language = 'javascript', 
  theme = 'vs-dark',
  height = '100%',
  width = '100%',
  options = {},
  onMount: onMountCallback
}) {
  const [editor, setEditor] = createSignal(null);
  let containerRef = null;

  const defaultOptions = {
    automaticLayout: true,
    fontSize: 14,
    lineNumbers: 'on',
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    wordWrap: 'on',
    tabSize: 2,
    insertSpaces: true,
    folding: true,
    lineDecorationsWidth: 10,
    lineNumbersMinChars: 3,
    renderWhitespace: 'boundary',
    // Disable keyboard shortcuts that might conflict with app shortcuts
    multiCursorModifier: 'ctrlCmd',
    wordSeparators: '`~!@#$%^&*()=+[{]}\\|;:\'",.<>/?',
    ...options
  };

  const scriptingKeywords = [
    // Renzora Engine API
    'Engine', 'Scene', 'Camera', 'Light', 'Mesh', 'Material', 'Texture',
    'Vector3', 'Quaternion', 'Matrix', 'Color3', 'Color4',
    'PhysicsEngine', 'Animation', 'ActionManager',
    'createBox', 'createSphere', 'createGround', 'createPlane',
    'loadMesh', 'loadTexture', 'loadAnimation',
    'setPosition', 'setRotation', 'setScale', 'setVisible',
    'getPosition', 'getRotation', 'getScale', 'isVisible',
    'addLight', 'addCamera', 'addMesh', 'removeMesh',
    'playAnimation', 'stopAnimation', 'pauseAnimation',
    'onPointerDown', 'onPointerUp', 'onPointerMove',
    'onKeyDown', 'onKeyUp', 'onCollision',
    'registerBeforeRender', 'unregisterBeforeRender',
    'dispose', 'clone', 'intersectsMesh'
  ];

  const registerRenScriptLanguage = (monaco) => {
    try {
      // Register RenScript language
      monaco.languages.register({ id: 'renscript' });

      // Set language configuration
      monaco.languages.setLanguageConfiguration('renscript', {
        comments: {
          lineComment: '#'
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
          { open: '"', close: '"', notIn: ['string'] },
          { open: "'", close: "'", notIn: ['string'] }
        ]
      });

      // Simplified monarch tokens provider
      monaco.languages.setMonarchTokensProvider('renscript', {
        keywords: [
          'script', 'camera', 'light', 'mesh', 'scene', 'transform', 'props',
          'start', 'update', 'destroy', 'on_collision', 'on_trigger',
          'if', 'else', 'for', 'while', 'return', 'break', 'continue',
          'function', 'const', 'let', 'var', 'null', 'true', 'false'
        ],
        
        typeKeywords: [
          'boolean', 'float', 'number', 'string', 'vector3', 'color'
        ],
        
        builtinFunctions: [
          'get_position', 'set_position', 'get_rotation', 'set_rotation',
          'log', 'sin', 'cos', 'random', 'clamp', 'lerp'
        ],

        tokenizer: {
          root: [
            // Comments
            [/#.*$/, 'comment'],
            
            // Strings (simplified - no lazy matching)
            [/"[^"]*"/, 'string'],
            [/'[^']*'/, 'string'],
            
            // Numbers
            [/\d*\.\d+/, 'number.float'],
            [/\d+/, 'number'],
            
            // Keywords and identifiers
            [/\b(?:script|camera|light|mesh|scene|transform|props|start|update|destroy|on_collision|on_trigger|if|else|for|while|return|break|continue|function|const|let|var|null|true|false)\b/, 'keyword'],
            [/\b(?:boolean|float|number|string|vector3|color)\b/, 'type'],
            [/\b(?:get_position|set_position|get_rotation|set_rotation|log|sin|cos|random|clamp|lerp)\b/, 'support.function'],
            [/[a-zA-Z_$][\w$]*/, 'identifier'],
            
            // Operators and punctuation  
            [/[{}()[\]]/, '@brackets'],
            [/[<>=!&|+\-*/]/, 'operator'],
            [/[;,.]/, 'delimiter'],
            
            // Whitespace
            [/\s+/, 'white']
          ]
        }
      });

      // Simple completion provider
      monaco.languages.registerCompletionItemProvider('renscript', {
        provideCompletionItems: () => {
          const suggestions = [
            {
              label: 'script',
              kind: monaco.languages.CompletionItemKind.Keyword,
              insertText: 'script ScriptName {\n\tstart {\n\t\t\n\t}\n\t\n\tupdate(dt) {\n\t\t\n\t}\n}',
              documentation: 'Create a new RenScript script'
            },
            {
              label: 'get_position',
              kind: monaco.languages.CompletionItemKind.Function,
              insertText: 'get_position()',
              documentation: 'Get object position'
            },
            {
              label: 'set_position',
              kind: monaco.languages.CompletionItemKind.Function,
              insertText: 'set_position(x, y, z)',
              documentation: 'Set object position'
            }
          ];
          return { suggestions };
        }
      });

    } catch (error) {
      console.error('Failed to register RenScript language:', error);
    }
  };

  const registerScriptingLanguage = (monaco) => {
    // Register tokens provider for syntax highlighting
    monaco.languages.setMonarchTokensProvider('javascript', {
      symbols: /[=><!~?:&|+\-*/^%]+/,
      keywords: [
        'break', 'case', 'catch', 'class', 'const', 'continue', 'debugger',
        'default', 'delete', 'do', 'else', 'export', 'extends', 'false', 'finally',
        'for', 'from', 'function', 'get', 'if', 'import', 'in', 'instanceof', 'let',
        'new', 'null', 'return', 'set', 'super', 'switch', 'this', 'throw', 'true',
        'try', 'typeof', 'undefined', 'var', 'void', 'while', 'with', 'yield'
      ],
      typeKeywords: [
        'boolean', 'double', 'byte', 'int', 'short', 'char', 'void', 'long', 'float'
      ],
      operators: [
        '=', '>', '<', '!', '~', '?', ':', '==', '<=', '>=', '!=',
        '&&', '||', '++', '--', '+', '-', '*', '/', '&', '|', '^', '%',
        '<<', '>>', '>>>', '+=', '-=', '*=', '/=', '&=', '|=', '^=',
        '%=', '<<=', '>>=', '>>>='
      ],
      tokenizer: {
        root: [
          [/\b(Engine|Scene|Camera|Light|Mesh|Material|Texture|Vector3|Quaternion|Matrix|Color3|Color4)\b/, 'type.identifier'],
          [/\b(createBox|createSphere|createGround|createPlane|loadMesh|loadTexture|loadAnimation)\b/, 'keyword.other'],
          [/\b(setPosition|setRotation|setScale|setVisible|getPosition|getRotation|getScale|isVisible)\b/, 'support.function'],
          [/\b(addLight|addCamera|addMesh|removeMesh|playAnimation|stopAnimation|pauseAnimation)\b/, 'support.function'],
          [/\b(onPointerDown|onPointerUp|onPointerMove|onKeyDown|onKeyUp|onCollision)\b/, 'support.function.event'],
          [/\b(registerBeforeRender|unregisterBeforeRender|dispose|clone|intersectsMesh)\b/, 'support.function'],
          
          [/[a-z_$][\w$]*/, {
            cases: {
              '@typeKeywords': 'keyword',
              '@keywords': 'keyword',
              '@default': 'identifier'
            }
          }],
          [/[A-Z][\w$]*/, 'type.identifier'],
          
          [/[{}()[\]]/, '@brackets'],
          [/[<>](?!@symbols)/, '@brackets'],
          [/@symbols/, {
            cases: {
              '@operators': 'operator',
              '@default': ''
            }
          }],
          
          [/\d*\.\d+([eE][-+]?\d+)?/, 'number.float'],
          [/0[xX][0-9a-fA-F]+/, 'number.hex'],
          [/\d+/, 'number'],
          [/[;,.]/, 'delimiter'],
          [/"([^"\\]|\\.)*$/, 'string.invalid'],
          [/'([^'\\]|\\.)*$/, 'string.invalid'],
          [/"/, { token: 'string.quote', bracket: '@open', next: '@string' }],
          [/'/, { token: 'string.quote', bracket: '@open', next: '@stringsingle' }],
          [/\/\*/, 'comment', '@comment'],
          [/\/\/.*$/, 'comment'],
        ],
        comment: [
          [/[^/*]+/, 'comment'],
          [/\/\*/, 'comment', '@push'],
          ["\\*/", 'comment', '@pop'],
          [/[/*]/, 'comment']
        ],
        string: [
          [/[^\\"]+/, 'string'],
          [/\\./, 'string.escape.invalid'],
          [/"/, { token: 'string.quote', bracket: '@close', next: '@pop' }]
        ],
        stringsingle: [
          [/[^\\']+/, 'string'],
          [/\\./, 'string.escape.invalid'],
          [/'/, { token: 'string.quote', bracket: '@close', next: '@pop' }]
        ],
      },
    });

    // Add completion provider for scripting API - with higher priority
    const customSuggestions = [
      {
        label: 'props',
        kind: monaco.languages.CompletionItemKind.Variable,
        documentation: 'Component properties',
        insertText: 'props',
        sortText: '0000'  // High priority
      },
      {
        label: 'children',
        kind: monaco.languages.CompletionItemKind.Property,
        documentation: 'Component children',
        insertText: 'children',
        sortText: '0001'
      },
      {
        label: 'onClick',
        kind: monaco.languages.CompletionItemKind.Property,
        documentation: 'Click event handler',
        insertText: 'onClick',
        sortText: '0002'
      },
      {
        label: 'onMount',
        kind: monaco.languages.CompletionItemKind.Function,
        documentation: 'SolidJS onMount lifecycle',
        insertText: 'onMount',
        sortText: '0003'
      },
      {
        label: 'createSignal',
        kind: monaco.languages.CompletionItemKind.Function,
        documentation: 'SolidJS reactive signal',
        insertText: 'createSignal',
        sortText: '0004'
      },
      {
        label: 'createEffect',
        kind: monaco.languages.CompletionItemKind.Function,
        documentation: 'SolidJS reactive effect',
        insertText: 'createEffect',
        sortText: '0005'
      },
      ...scriptingKeywords.map((keyword, index) => ({
        label: keyword,
        kind: monaco.languages.CompletionItemKind.Function,
        documentation: `Renzora Engine API: ${keyword}`,
        insertText: keyword,
        sortText: `1${index.toString().padStart(3, '0')}`
      }))
    ];

    monaco.languages.registerCompletionItemProvider('javascript', {
      triggerCharacters: ['.', ' '],
      provideCompletionItems: async (model, position) => {
        const word = model.getWordUntilPosition(position);
        const range = {
          startLineNumber: position.lineNumber,
          endLineNumber: position.lineNumber,
          startColumn: word.startColumn,
          endColumn: word.endColumn
        };

        // Add range to all suggestions
        const suggestions = customSuggestions.map(suggestion => ({
          ...suggestion,
          range
        }));

        return { suggestions };
      }
    });

    monaco.languages.registerCompletionItemProvider('typescript', {
      triggerCharacters: ['.', ' '],
      provideCompletionItems: async (model, position) => {
        const word = model.getWordUntilPosition(position);
        const range = {
          startLineNumber: position.lineNumber,
          endLineNumber: position.lineNumber,
          startColumn: word.startColumn,
          endColumn: word.endColumn
        };

        // Add range to all suggestions
        const suggestions = customSuggestions.map(suggestion => ({
          ...suggestion,
          range
        }));

        return { suggestions };
      }
    });
  };

  // Helper function to get CSS custom property values and convert to hex
  const getCSSVariable = (name, fallback = '000000') => {
    if (typeof window === 'undefined') return fallback;
    
    try {
      const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
      console.log(`CSS Variable ${name}:`, value);
      
      if (!value) return fallback;
      
      // DaisyUI variables are usually in HSL format like "200 6% 10%"
      // We need to convert them to hex
      if (value.includes('%')) {
        // Parse HSL values like "200 6% 10%"
        const parts = value.split(' ');
        if (parts.length >= 3) {
          const h = parseInt(parts[0]) || 0;
          const s = parseInt(parts[1]) || 0;
          const l = parseInt(parts[2]) || 50;
          return hslToHex(h, s, l);
        }
      }
      
      // If it's already a hex color, use it
      if (value.startsWith('#')) {
        return value.replace('#', '');
      }
      
      // If it's RGB, convert it
      if (value.startsWith('rgb')) {
        const matches = value.match(/\d+/g);
        if (matches && matches.length >= 3) {
          const r = parseInt(matches[0]);
          const g = parseInt(matches[1]);
          const b = parseInt(matches[2]);
          return rgbToHex(r, g, b);
        }
      }
      
      return fallback;
    } catch (error) {
      console.warn(`Failed to get CSS variable ${name}:`, error);
      return fallback;
    }
  };

  // Convert HSL to Hex
  const hslToHex = (h, s, l) => {
    l /= 100;
    const a = s * Math.min(l, 1 - l) / 100;
    const f = n => {
      const k = (n + h / 30) % 12;
      const color = l - a * Math.max(Math.min(k - 3, 9 - k, 1), -1);
      return Math.round(255 * color).toString(16).padStart(2, '0');
    };
    return `${f(0)}${f(8)}${f(4)}`;
  };

  // Convert RGB to Hex
  const rgbToHex = (r, g, b) => {
    return ((r << 16) | (g << 8) | b).toString(16).padStart(6, '0');
  };

  // Define DaisyUI theme
  const createDaisyUITheme = (monaco) => {
    console.log('Creating DaisyUI theme...');
    
    // Get current theme colors from CSS custom properties
    const baseContent = getCSSVariable('--bc', 'A6ADBA');
    const base100 = getCSSVariable('--b1', '2A303C');
    const base200 = getCSSVariable('--b2', '242933');
    const base300 = getCSSVariable('--b3', '1D232A');
    const primary = getCSSVariable('--p', '3ABFF8');
    const secondary = getCSSVariable('--s', '828DF8');
    const accent = getCSSVariable('--a', 'F471B5');
    const _neutral = getCSSVariable('--n', '1B1D1D');
    const info = getCSSVariable('--in', '3ABFF8');
    const success = getCSSVariable('--su', '36D399');
    const warning = getCSSVariable('--wa', 'FBBD23');
    const error = getCSSVariable('--er', 'F87272');

    console.log('Theme colors:', { baseContent, base100, base200, base300, primary });

    monaco.editor.defineTheme('daisyui-theme', {
      base: 'vs-dark',
      inherit: true,
      rules: [
        // General
        { token: '', foreground: baseContent },
        { token: 'comment', foreground: '6B7280', fontStyle: 'italic' },
        { token: 'keyword', foreground: primary, fontStyle: 'bold' },
        { token: 'identifier', foreground: baseContent },
        
        // Types and classes
        { token: 'type', foreground: secondary },
        { token: 'type.identifier', foreground: accent },
        
        // Strings and numbers
        { token: 'string', foreground: success },
        { token: 'string.quote', foreground: success },
        { token: 'number', foreground: warning },
        { token: 'number.float', foreground: warning },
        { token: 'number.hex', foreground: warning },
        
        // Functions and methods
        { token: 'support.function', foreground: info },
        { token: 'support.function.event', foreground: accent },
        { token: 'keyword.other', foreground: secondary },
        
        // Operators and delimiters
        { token: 'operator', foreground: baseContent },
        { token: 'delimiter', foreground: '9CA3AF' },
        
        // Invalid/errors
        { token: 'string.invalid', foreground: error },
        { token: 'string.escape.invalid', foreground: error },
        
        // Brackets
        { token: 'delimiter.bracket', foreground: baseContent },
      ],
      colors: {
        // Editor background
        'editor.background': base100,
        'editor.foreground': baseContent,
        
        // Line numbers
        'editorLineNumber.foreground': '6B7280',
        'editorLineNumber.activeForeground': primary,
        
        // Cursor
        'editorCursor.foreground': primary,
        
        // Selection
        'editor.selectionBackground': primary + '40',
        'editor.selectionHighlightBackground': primary + '20',
        
        // Current line
        'editor.lineHighlightBackground': base200,
        
        // Indentation guides
        'editorIndentGuide.background': base300,
        'editorIndentGuide.activeBackground': primary + '60',
        
        // Gutter
        'editorGutter.background': base100,
        
        // Scrollbar
        'scrollbar.shadow': '00000020',
        'scrollbarSlider.background': base300 + '80',
        'scrollbarSlider.hoverBackground': base300,
        'scrollbarSlider.activeBackground': primary + '80',
        
        // Find/replace
        'editor.findMatchBackground': warning + '40',
        'editor.findMatchHighlightBackground': warning + '20',
        
        // Brackets matching
        'editorBracketMatch.background': primary + '20',
        'editorBracketMatch.border': primary,
        
        // Overview ruler
        'editorOverviewRuler.border': base300,
        
        // Widget backgrounds
        'editorWidget.background': base200,
        'editorWidget.border': base300,
        'editorSuggestWidget.background': base200,
        'editorSuggestWidget.border': base300,
        'editorSuggestWidget.selectedBackground': primary + '40',
        
        // Input
        'input.background': base200,
        'input.border': base300,
        'input.foreground': baseContent,
        
        // Dropdown
        'dropdown.background': base200,
        'dropdown.border': base300,
        'dropdown.foreground': baseContent,
      }
    });
  };

  onMount(async () => {
    try {
      // Configure Monaco loader to minimize bundle size
      loader.config({
        paths: {
          vs: 'https://cdn.jsdelivr.net/npm/monaco-editor@0.52.2/min/vs'
        }
      });

      const monaco = await loader.init();
      
      // Create and register DaisyUI theme
      createDaisyUITheme(monaco);
      
      // Register custom language features
      registerScriptingLanguage(monaco);
      registerRenScriptLanguage(monaco);

      const editorInstance = monaco.editor.create(containerRef, {
        value: value || '',
        language,
        theme,
        ...defaultOptions,
        // Disable drag and drop to prevent conflicts with asset drag/drop
        dragAndDrop: false,
        dropIntoEditor: { enabled: false }
      });

      // Explicitly disable drag/drop events on the editor container
      const editorContainer = editorInstance.getDomNode();
      let dragEventListeners = [];
      
      if (editorContainer) {
        const preventDragDrop = (e) => {
          e.preventDefault();
          e.stopPropagation();
        };
        
        // Store listener references for cleanup
        const addDragListener = (element, event, handler) => {
          element.addEventListener(event, handler, true);
          dragEventListeners.push({ element, event, handler });
        };
        
        // Prevent all drag/drop events on container
        addDragListener(editorContainer, 'dragover', preventDragDrop);
        addDragListener(editorContainer, 'dragenter', preventDragDrop);  
        addDragListener(editorContainer, 'dragleave', preventDragDrop);
        addDragListener(editorContainer, 'drop', preventDragDrop);
        
        // Also prevent on the inner editor elements
        const setupEditableAreaListeners = () => {
          const editableArea = editorContainer.querySelector('.monaco-editor .overflow-guard .monaco-scrollable-element');
          if (editableArea) {
            addDragListener(editableArea, 'dragover', preventDragDrop);
            addDragListener(editableArea, 'dragenter', preventDragDrop);
            addDragListener(editableArea, 'dragleave', preventDragDrop);
            addDragListener(editableArea, 'drop', preventDragDrop);
          }
        };
        
        // Apply immediately and after a small delay to ensure DOM is ready
        setupEditableAreaListeners();
        setTimeout(setupEditableAreaListeners, 100);
      }

      setEditor(editorInstance);

      // Disable application shortcuts when Monaco Editor is focused
      editorInstance.onDidFocusEditorText(() => {
        keyboardShortcuts.disable();
        console.log('[MonacoEditor] Editor focused - shortcuts disabled');
      });

      editorInstance.onDidBlurEditorText(() => {
        keyboardShortcuts.enable();
        console.log('[MonacoEditor] Editor blurred - shortcuts enabled');
      });

      // Handle value changes
      if (onChange) {
        editorInstance.onDidChangeModelContent(() => {
          onChange(editorInstance.getValue());
        });
      }

      // Call onMount callback if provided
      if (onMountCallback) {
        onMountCallback(editorInstance, monaco);
      }

      // Handle container resize
      const resizeObserver = new ResizeObserver(() => {
        editorInstance.layout();
      });
      resizeObserver.observe(containerRef);

      onCleanup(() => {
        console.log('[MonacoEditor] Cleaning up editor instance');
        
        // Clean up drag/drop event listeners
        if (dragEventListeners) {
          dragEventListeners.forEach(({ element, event, handler }) => {
            try {
              element.removeEventListener(event, handler, true);
            } catch (error) {
              console.warn('[MonacoEditor] Error removing event listener:', error);
            }
          });
          dragEventListeners = [];
        }
        
        // Clean up resize observer
        resizeObserver.disconnect();
        
        // Re-enable shortcuts if they were disabled
        keyboardShortcuts.enable();
        
        // Dispose of editor instance
        try {
          editorInstance.dispose();
        } catch (error) {
          console.warn('[MonacoEditor] Error disposing editor:', error);
        }
      });

    } catch (error) {
      console.error('Failed to initialize Monaco Editor:', error);
    }
  });

  // Update editor value when prop changes
  createEffect(() => {
    const editorInstance = editor();
    if (editorInstance && value !== undefined) {
      const currentValue = editorInstance.getValue();
      if (currentValue !== value) {
        editorInstance.setValue(value);
      }
    }
  });

  // Update editor language when prop changes
  createEffect(() => {
    const editorInstance = editor();
    if (editorInstance && language) {
      const model = editorInstance.getModel();
      if (model) {
        loader.init().then(monaco => {
          monaco.editor.setModelLanguage(model, language);
        });
      }
    }
  });

  // Watch for theme changes using MutationObserver
  onMount(() => {
    if (theme === 'daisyui-theme') {
      const observer = new MutationObserver((mutations) => {
        mutations.forEach((mutation) => {
          if (mutation.type === 'attributes' && mutation.attributeName === 'data-theme') {
            // Theme changed, update Monaco theme
            const editorInstance = editor();
            if (editorInstance) {
              loader.init().then(monaco => {
                createDaisyUITheme(monaco);
                monaco.editor.setTheme('daisyui-theme');
              });
            }
          }
        });
      });

      // Observe theme changes on html element
      observer.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['data-theme', 'class']
      });

      onCleanup(() => {
        observer.disconnect();
      });
    }
  });

  return (
    <div 
      ref={containerRef}
      style={{ 
        width: typeof width === 'number' ? `${width}px` : width,
        height: typeof height === 'number' ? `${height}px` : height
      }}
      class="monaco-editor-container"
    />
  );
}

export default MonacoEditor;