import { createStore } from 'solid-js/store';
import { createSignal } from 'solid-js';
import { viewportActions } from './ViewportStore.jsx';

// Store for script editor state
const [scriptEditorStore, setScriptEditorStore] = createStore({
  openScripts: new Map(), // Map of filePath -> { content, isDirty, lastSaved }
  activeScript: null,     // Currently active script path
  isVisible: false        // Whether script editor is visible
});

// Actions for managing script editor state
export const scriptEditorActions = {
  openScript(filePath, fileName, content = '') {
    setScriptEditorStore('openScripts', (scripts) => {
      const newScripts = new Map(scripts);
      if (!newScripts.has(filePath)) {
        newScripts.set(filePath, {
          fileName,
          content,
          isDirty: false,
          lastSaved: null
        });
      }
      return newScripts;
    });
    
    setScriptEditorStore('activeScript', filePath);
    setScriptEditorStore('isVisible', true);
    
    // Switch to script editor tab
    viewportActions.setActiveViewportTab('script-editor');
  },

  closeScript(filePath) {
    setScriptEditorStore('openScripts', (scripts) => {
      const newScripts = new Map(scripts);
      newScripts.delete(filePath);
      return newScripts;
    });
    
    // If this was the active script, switch to another or close editor
    if (scriptEditorStore.activeScript === filePath) {
      const remainingScripts = Array.from(scriptEditorStore.openScripts.keys());
      if (remainingScripts.length > 0) {
        setScriptEditorStore('activeScript', remainingScripts[0]);
      } else {
        setScriptEditorStore('activeScript', null);
        setScriptEditorStore('isVisible', false);
      }
    }
  },

  setActiveScript(filePath) {
    if (scriptEditorStore.openScripts.has(filePath)) {
      setScriptEditorStore('activeScript', filePath);
    }
  },

  updateScriptContent(filePath, content, isDirty = true) {
    setScriptEditorStore('openScripts', filePath, {
      content,
      isDirty,
      lastSaved: isDirty ? scriptEditorStore.openScripts.get(filePath)?.lastSaved : new Date().toLocaleTimeString()
    });
  },

  markScriptSaved(filePath) {
    setScriptEditorStore('openScripts', filePath, 'isDirty', false);
    setScriptEditorStore('openScripts', filePath, 'lastSaved', new Date().toLocaleTimeString());
  },

  showEditor() {
    setScriptEditorStore('isVisible', true);
    // Switch to script editor tab
    viewportActions.setActiveViewportTab('script-editor');
  },

  hideEditor() {
    setScriptEditorStore('isVisible', false);
  },

  closeAllScripts() {
    setScriptEditorStore('openScripts', new Map());
    setScriptEditorStore('activeScript', null);
    setScriptEditorStore('isVisible', false);
  }
};

export { scriptEditorStore };