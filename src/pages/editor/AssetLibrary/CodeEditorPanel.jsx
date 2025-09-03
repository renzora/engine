import { createSignal, createEffect, onCleanup, Show, For } from 'solid-js';
import { IconX, IconDeviceFloppy, IconFileText, IconCode, IconChevronRight, IconChevronLeft } from '@tabler/icons-solidjs';
import MonacoEditor from '@/components/MonacoEditor';
import { readFile, writeFile, deleteFile } from '@/api/bridge/files';
import { getCurrentProject } from '@/api/bridge/projects';
import { getScriptRuntime } from '@/api/script';
import { editorStore } from '@/layout/stores/EditorStore';

function CodeEditorPanel({ 
  isOpen, 
  onClose, 
  selectedFile, 
  width = 400,
  onToggleSide,
  currentSide = 'left'
}) {
  const [editorValue, setEditorValue] = createSignal('');
  const [loading, setLoading] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal(null);
  const [hasChanges, setHasChanges] = createSignal(false);
  const [originalValue, setOriginalValue] = createSignal('');
  const [fileName, setFileName] = createSignal('untitled.ren');
  const [originalFileName, setOriginalFileName] = createSignal('');
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
    
    // Helper function to get line content
    const getLineContent = (lineNum) => {
      const lines = content.split('\n');
      return lines[lineNum - 1] || '';
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
      
      // Check for common typos - this will catch "pops" instead of "props"
      const typoMatches = [...content.matchAll(/\b(pops|prop|porps|prpos)\s*(?:[a-zA-Z_][a-zA-Z0-9_]*)?\s*\{/g)];
      typoMatches.forEach(match => {
        const lineNum = getLineNumber(match.index);
        errors.push({
          line: lineNum,
          column: match.index - content.lastIndexOf('\n', match.index) - 1,
          message: `Did you mean 'props'? Found '${match[1]}'`,
          severity: 'error',
          suggestion: `props`
        });
      });
      
      // Extract properties from props sections with better error handling
      // This regex handles nested braces by matching balanced braces
      const propsRegex = /props\s*([a-zA-Z_][a-zA-Z0-9_]*)?\s*\{((?:[^{}]*\{[^}]*\}[^{}]*)*[^{}]*)\}/g;
      let propsMatch;
      const usedPropertyNames = new Set();
      
      while ((propsMatch = propsRegex.exec(content)) !== null) {
        let sectionName = propsMatch[1] || 'General';
        const propsContent = propsMatch[2];
        
        // Skip empty props sections
        if (!propsContent || propsContent.trim() === '') {
          continue;
        }
        
        // Validate section name
        if (sectionName !== 'General' && !/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(sectionName)) {
          const lineNum = getLineNumber(propsMatch.index);
          errors.push({
            line: lineNum,
            column: propsMatch.index - content.lastIndexOf('\n', propsMatch.index) - 1,
            message: `Invalid section name '${sectionName}'. Section names must start with a letter or underscore.`,
            severity: 'warning'
          });
          sectionName = 'General';
        }
        
        // Extract individual properties with better validation
        // This regex handles property options with nested braces
        const propRegex = /([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*([a-zA-Z_][a-zA-Z0-9_]*)(?:\s*\{([^{}]*(?:\{[^}]*\}[^{}]*)*)\})?/g;
        let propMatch;
        
        while ((propMatch = propRegex.exec(propsContent)) !== null) {
          const propName = propMatch[1];
          const propType = propMatch[2];
          const propOptions = propMatch[3] || '';
          
          // Check for duplicate property names
          if (usedPropertyNames.has(propName)) {
            const lineNum = getLineNumber(propMatch.index);
            errors.push({
              line: lineNum,
              column: propMatch.index - content.lastIndexOf('\n', propMatch.index) - 1,
              message: `Duplicate property name '${propName}'. Property names must be unique across all sections.`,
              severity: 'error'
            });
            continue; // Skip this property
          }
          usedPropertyNames.add(propName);
          
          // Validate property type
          const validTypes = ['boolean', 'number', 'float', 'string', 'range', 'select'];
          if (!validTypes.includes(propType.toLowerCase())) {
            const lineNum = getLineNumber(propMatch.index);
            errors.push({
              line: lineNum,
              column: propMatch.index - content.lastIndexOf('\n', propMatch.index) - 1,
              message: `Unknown property type '${propType}' for property '${propName}'. Supported types: ${validTypes.join(', ')}`,
              severity: 'warning'
            });
          }
          
          // Extract property options with validation
          let defaultValue = null;
          let min = null;
          let max = null;
          let description = null;
          let options = null;
          
          if (propOptions) {
            try {
              const defaultMatch = propOptions.match(/default\s*:\s*([^,}]+)/);
              const minMatch = propOptions.match(/min\s*:\s*([-\d.]+)/);
              const maxMatch = propOptions.match(/max\s*:\s*([-\d.]+)/);
              const descMatch = propOptions.match(/description\s*:\s*"([^"]+)"/);
              const optionsMatch = propOptions.match(/options\s*:\s*\[([^\]]+)\]/);
              
              if (defaultMatch) {
                defaultValue = defaultMatch[1].trim();
                // Clean up quotes for string defaults
                if (defaultValue.startsWith('"') && defaultValue.endsWith('"')) {
                  defaultValue = defaultValue.slice(1, -1);
                }
              }
              
              if (minMatch) {
                min = parseFloat(minMatch[1]);
                if (isNaN(min)) {
                  console.warn(`Invalid min value for property '${propName}': ${minMatch[1]}, ignoring`);
                  min = null;
                }
              }
              
              if (maxMatch) {
                max = parseFloat(maxMatch[1]);
                if (isNaN(max)) {
                  console.warn(`Invalid max value for property '${propName}': ${maxMatch[1]}, ignoring`);
                  max = null;
                }
              }
              
              if (min !== null && max !== null && min > max) {
                console.warn(`Property '${propName}': min value (${min}) cannot be greater than max value (${max}), swapping values`);
                [min, max] = [max, min];
              }
              
              if (descMatch) {
                description = descMatch[1];
              }
              
              if (optionsMatch) {
                // Parse options array for select type
                const optionsStr = optionsMatch[1];
                options = optionsStr.split(',').map(opt => {
                  const trimmed = opt.trim();
                  // Remove quotes if present
                  if (trimmed.startsWith('"') && trimmed.endsWith('"')) {
                    return trimmed.slice(1, -1);
                  }
                  return trimmed;
                });
              }
              
            } catch (optionError) {
              console.warn(`Error parsing options for property '${propName}': ${optionError.message}, using defaults`);
            }
          }
          
          properties.push({
            type: 'PropertyDeclaration',
            name: propName,
            propType: propType,
            section: sectionName,
            defaultValue,
            min,
            max,
            description,
            options
          });
        }
        
        // Check for malformed properties in this section - but be more lenient with braces
        const remainingContent = propsContent.replace(propRegex, '').trim();
        if (remainingContent && !remainingContent.match(/^\s*$/)) {
          const lines = remainingContent.split('\n').filter(line => line.trim() && !line.trim().match(/^[{}]\s*$/));
          if (lines.length > 0) {
            console.warn(`Potentially malformed syntax in section '${sectionName}': "${lines[0].trim()}" - continuing anyway`);
            // Don't throw an error for now, just warn
          }
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

  // Property diffing system
  const diffProperties = (oldProps, newProps) => {
    const changes = {
      added: [],
      removed: [],
      modified: [],
      renamed: []
    };
    
    const oldPropMap = new Map(oldProps.map(prop => [prop.name, prop]));
    const newPropMap = new Map(newProps.map(prop => [prop.name, prop]));
    
    // Find added and modified properties
    for (const [name, newProp] of newPropMap) {
      const oldProp = oldPropMap.get(name);
      
      if (!oldProp) {
        changes.added.push(newProp);
      } else if (!arePropertiesEqual(oldProp, newProp)) {
        changes.modified.push({
          old: oldProp,
          new: newProp,
          changes: getPropertyChanges(oldProp, newProp)
        });
      }
    }
    
    // Find removed properties
    for (const [name, oldProp] of oldPropMap) {
      if (!newPropMap.has(name)) {
        changes.removed.push(oldProp);
      }
    }
    
    // Simple rename detection: if we have one removed and one added with same type,
    // it might be a rename
    if (changes.removed.length === 1 && changes.added.length === 1 && 
        changes.removed[0].propType === changes.added[0].propType) {
      changes.renamed.push({
        from: changes.removed[0],
        to: changes.added[0]
      });
      changes.removed = [];
      changes.added = [];
    }
    
    return changes;
  };

  const arePropertiesEqual = (prop1, prop2) => {
    return prop1.name === prop2.name &&
           prop1.propType === prop2.propType &&
           prop1.defaultValue === prop2.defaultValue &&
           prop1.section === prop2.section &&
           prop1.min === prop2.min &&
           prop1.max === prop2.max &&
           prop1.description === prop2.description;
  };

  const getPropertyChanges = (oldProp, newProp) => {
    const changes = [];
    if (oldProp.propType !== newProp.propType) changes.push('type');
    if (oldProp.defaultValue !== newProp.defaultValue) changes.push('defaultValue');
    if (oldProp.section !== newProp.section) changes.push('section');
    if (oldProp.min !== newProp.min) changes.push('min');
    if (oldProp.max !== newProp.max) changes.push('max');
    if (oldProp.description !== newProp.description) changes.push('description');
    return changes;
  };

  // Determine if content change is only property metadata vs actual code changes
  const isOnlyPropertyMetadataChange = (oldContent, newContent) => {
    try {
      // Remove all property definitions from both versions
      const stripProperties = (content) => {
        // Remove everything inside props blocks
        return content.replace(/props\s*\w*\s*\{[^{}]*(?:\{[^}]*\}[^{}]*)*\}/g, 'props{}');
      };
      
      const oldStripped = stripProperties(oldContent);
      const newStripped = stripProperties(newContent);
      
      const isPropertyOnlyChange = oldStripped === newStripped;
      if (!isPropertyOnlyChange) {
        console.log('🔄 Code structure changed - will reload script');
      }
      
      // If the structure is the same after removing prop details, it's only property changes
      return isPropertyOnlyChange;
      
    } catch (error) {
      console.warn('Failed to analyze property changes, defaulting to full reload:', error);
      return false; // If analysis fails, be safe and do full reload
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

    // Check if this is a structural code change vs just property changes
    const currentContent = editorValue();
    const previousContent = previousScriptContent();
    const contentChanged = currentContent !== previousContent;
    
    if (contentChanged) {
      console.log('🔍 Content changed, analyzing type of change...');
      
      // Check if this is just property changes vs actual code changes
      const isOnlyPropertyChanges = isOnlyPropertyMetadataChange(previousContent, currentContent);
      
      if (isOnlyPropertyChanges) {
        console.log('✅ Only property metadata changed - updating properties without reloading script');
        setPreviousScriptContent(currentContent);
        // Continue to property-only update logic below
      } else {
        console.log('🔄 Structural code changes detected, reloading script', currentFile.path);
        setPreviousScriptContent(currentContent);
        
        // Trigger full script reload when actual code structure changes
        triggerScriptReload(currentFile.path, currentContent);
        return; // Exit early - script reload will handle everything
      }
    }

    // Only handle property changes if code hasn't changed
    const oldProps = previousProperties();
    const newProps = ast.properties || [];
    
    const propertyChanges = diffProperties(oldProps, newProps);
    
    // Only log if there are actual changes
    if (propertyChanges.added.length > 0 || propertyChanges.removed.length > 0 || 
        propertyChanges.modified.length > 0 || propertyChanges.renamed.length > 0) {
      console.log('🔧 Property changes:', {
        added: propertyChanges.added.map(p => p.name),
        removed: propertyChanges.removed.map(p => p.name), 
        modified: propertyChanges.modified.map(c => `${c.new.name}:[${c.changes.join(',')}]`),
        renamed: propertyChanges.renamed.map(r => `${r.from.name}->${r.to.name}`)
      });
    }
    
    // Update previous properties for next comparison
    setPreviousProperties(newProps);
    
    // Find all objects using this script
    const scriptPath = currentFile.path;
    
    // Get the script manager stats to find active instances
    const stats = runtime.getStats();
    
    // Only log if there are actual changes
    if (propertyChanges.added.length > 0 || propertyChanges.removed.length > 0 || 
        propertyChanges.modified.length > 0 || propertyChanges.renamed.length > 0) {
      console.log('🔧 Live Script Update: Property changes detected for', scriptPath);
      console.log('  Added:', propertyChanges.added.map(p => p.name));
      console.log('  Removed:', propertyChanges.removed.map(p => p.name));
      console.log('  Modified:', propertyChanges.modified.map(p => p.new.name));
      console.log('  Renamed:', propertyChanges.renamed.map(r => `${r.from.name} -> ${r.to.name}`));
    }
    
    // Dispatch a custom event that the object properties system can listen to
    document.dispatchEvent(new CustomEvent('engine:script-properties-updated', {
      detail: { 
        scriptPath,
        properties: newProps,
        propertyChanges,
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
      
      // Find all objects that have this script attached
      const stats = runtime.getStats();
      const affectedObjects = [];
      
      // Search through active scripts to find objects using this script
      if (runtime.scriptManager && runtime.scriptManager.activeScripts) {
        runtime.scriptManager.activeScripts.forEach((scripts, objectId) => {
          scripts.forEach(script => {
            if (script._scriptPath === scriptPath) {
              affectedObjects.push(objectId);
            }
          });
        });
      }
      
      console.log('🗑️ Found', affectedObjects.length, 'objects using this script');
      
      // Remove script from each affected object
      affectedObjects.forEach(objectId => {
        console.log('🗑️ Removing script from object:', objectId);
        runtime.detachScript(objectId, scriptPath);
      });
      
      // Save empty file (or delete it)
      const filePath = `projects/${currentProject.name}/${scriptPath}`;
      await writeFile(filePath, '// Empty script file\n');
      
      // Dispatch event to notify UI that script was removed
      document.dispatchEvent(new CustomEvent('engine:script-removed', {
        detail: { 
          scriptPath,
          affectedObjects,
          action: 'script_removed'
        }
      }));
      
      console.log('✅ Script removed successfully from', affectedObjects.length, 'objects');
      
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
        <div class="flex items-center justify-between py-2 pl-3 pr-1 border-b border-base-300 bg-base-200">
          <div class="flex items-center gap-1.5 min-w-0 flex-1">
            <IconFileText class="w-4 h-4 text-primary flex-shrink-0" />
            <Show when={hasChanges()}>
              <div class="w-2 h-2 bg-warning rounded-full flex-shrink-0" title="Unsaved changes" />
            </Show>
            <input
              type="text"
              value={fileName()}
              onInput={(e) => handleFileNameChange(e.target.value)}
              class="text-xs font-medium bg-transparent border-none outline-none focus:bg-base-100 focus:px-1 focus:py-0.5 focus:rounded focus:border focus:border-primary/20 transition-all flex-1 min-w-0"
              placeholder="filename.js"
            />
          </div>
          
          <div class="flex items-center gap-1">
            <Show when={selectedFile() && (fileName().endsWith('.js') || fileName().endsWith('.jsx') || fileName().endsWith('.ts') || fileName().endsWith('.tsx') || fileName().endsWith('.ren'))}>
              <button
                draggable="true"
                onDragStart={(e) => {
                  const file = selectedFile();
                  if (file) {
                    e.dataTransfer.setData('text/plain', JSON.stringify({
                      type: 'asset',
                      fileType: 'script',
                      name: file.name,
                      path: file.path
                    }));
                    e.dataTransfer.effectAllowed = 'copy';
                    
                    // Create custom drag image
                    const dragCard = document.createElement('div');
                    dragCard.className = 'fixed top-[-1000px] bg-success text-success-content rounded-lg p-3 shadow-lg flex items-center gap-2 min-w-[200px]';
                    dragCard.innerHTML = `
                      <div class="w-8 h-8 bg-success-content/20 rounded flex items-center justify-center">
                        <svg class="w-4 h-4 text-success-content" fill="currentColor" viewBox="0 0 24 24">
                          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6z"/>
                          <path d="M14 2v6h6"/>
                          <path d="M16 13H8"/>
                          <path d="M16 17H8"/>
                          <path d="M10 9H8"/>
                        </svg>
                      </div>
                      <div class="flex flex-col">
                        <span class="text-sm font-medium text-success-content">${file.name}</span>
                        <span class="text-xs text-success-content">Script file</span>
                      </div>
                    `;
                    document.body.appendChild(dragCard);
                    e.dataTransfer.setDragImage(dragCard, 100, 25);
                    setTimeout(() => document.body.removeChild(dragCard), 0);
                  }
                }}
                class="px-1.5 py-0.5 text-xs rounded text-base-content/60 hover:text-base-content hover:bg-base-300/60 transition-colors cursor-grab active:cursor-grabbing"
                title="Drag to attach script to object"
              >
                <IconCode class="w-3 h-3" />
              </button>
            </Show>
            
            <Show when={onToggleSide}>
              <button
                onClick={onToggleSide}
                class="px-1.5 py-0.5 text-xs rounded text-base-content/60 hover:text-base-content hover:bg-base-300/60 transition-colors cursor-pointer"
                title={`Move editor to ${currentSide === 'left' ? 'right' : 'left'} side`}
              >
                {currentSide === 'left' ? <IconChevronRight class="w-3 h-3" /> : <IconChevronLeft class="w-3 h-3" />}
              </button>
            </Show>
            
            <button
              onClick={saveFile}
              disabled={!hasChanges() || saving()}
              class={`px-1.5 py-0.5 text-xs rounded transition-colors ${
                hasChanges() && !saving()
                  ? 'bg-primary text-primary-content hover:bg-primary/80 cursor-pointer'
                  : 'text-base-content/50 cursor-not-allowed hover:bg-base-300/60'
              }`}
              title="Save (Ctrl+S)"
            >
              <Show when={saving()} fallback={<IconDeviceFloppy class="w-3 h-3" />}>
                <div class="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
              </Show>
            </button>
            
            <button
              onClick={handleClose}
              class="px-1.5 py-0.5 text-xs rounded text-base-content/60 hover:text-base-content hover:bg-base-300/60 transition-colors cursor-pointer"
              title="Close"
            >
              <IconX class="w-3 h-3" />
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
            <div class="flex-1 flex flex-col">
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

export default CodeEditorPanel;