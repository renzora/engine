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
  let containerRef;

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
            [/[{}()\[\]]/, '@brackets'],
            [/[<>=!&|+\-*\/]/, 'operator'],
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
      symbols: /[=><!~?:&|+\-*\/\^%]+/,
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
          [/[A-Z][\w\$]*/, 'type.identifier'],
          
          [/[{}()\[\]]/, '@brackets'],
          [/[<>](?!@symbols)/, '@brackets'],
          [/@symbols/, {
            cases: {
              '@operators': 'operator',
              '@default': ''
            }
          }],
          
          [/\d*\.\d+([eE][\-+]?\d+)?/, 'number.float'],
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
          [/[^\/*]+/, 'comment'],
          [/\/\*/, 'comment', '@push'],
          ["\\*/", 'comment', '@pop'],
          [/[\/*]/, 'comment']
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

  onMount(async () => {
    try {
      // Configure Monaco loader to minimize bundle size
      loader.config({
        paths: {
          vs: 'https://cdn.jsdelivr.net/npm/monaco-editor@0.52.2/min/vs'
        }
      });

      const monaco = await loader.init();
      
      // Register custom language features
      registerScriptingLanguage(monaco);
      registerRenScriptLanguage(monaco);

      const editorInstance = monaco.editor.create(containerRef, {
        value: value || '',
        language,
        theme,
        ...defaultOptions
      });

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
        resizeObserver.disconnect();
        editorInstance.dispose();
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