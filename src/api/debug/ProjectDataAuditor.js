/**
 * Development tool for auditing project data leaks between project switches
 * Only used in development mode to help detect data that persists when it shouldn't
 */

class ProjectDataAuditor {
  constructor() {
    this.snapshots = new Map();
    this.enabled = false; // Only enable in development
  }

  enable() {
    this.enabled = true;
    console.log('🔍 ProjectDataAuditor enabled - will track data leaks between projects');
  }

  disable() {
    this.enabled = false;
    this.snapshots.clear();
    console.log('🔍 ProjectDataAuditor disabled');
  }

  /**
   * Take a snapshot of all data stores before project switch
   */
  takePreSwitchSnapshot(projectName) {
    if (!this.enabled) return;

    const snapshot = {
      timestamp: Date.now(),
      projectName,
      data: this._captureAllStoreData()
    };

    this.snapshots.set(`pre-${projectName}`, snapshot);
    console.log(`📸 Pre-switch snapshot taken for project: ${projectName}`);
  }

  /**
   * Take a snapshot of all data stores after project switch and compare
   */
  takePostSwitchSnapshot(newProjectName, oldProjectName) {
    if (!this.enabled) return;

    const postSnapshot = {
      timestamp: Date.now(),
      projectName: newProjectName,
      data: this._captureAllStoreData()
    };

    this.snapshots.set(`post-${newProjectName}`, postSnapshot);

    // Compare with pre-switch snapshot
    const preSnapshot = this.snapshots.get(`pre-${oldProjectName}`);
    if (preSnapshot) {
      this._compareSnapshots(preSnapshot, postSnapshot);
    }

    console.log(`📸 Post-switch snapshot taken for project: ${newProjectName}`);
  }

  /**
   * Capture data from all stores
   */
  _captureAllStoreData() {
    const data = {};

    try {
      // Editor Store
      if (window.editorStore) {
        data.editorStore = {
          selection: { ...window.editorStore.selection },
          consoleMessages: window.editorStore.console.messages.length,
          scriptExecution: window.editorStore.scripts.isPlaying,
          panels: { ...window.editorStore.panels },
          ui: { 
            selectedTool: window.editorStore.ui.selectedTool,
            selectedBottomTab: window.editorStore.ui.selectedBottomTab
          }
        };
      }

      // Render Store
      if (window.renderStore) {
        data.renderStore = {
          selectedObjectId: window.renderStore.selectedObject?.uniqueId || window.renderStore.selectedObject?.name,
          hierarchyCount: window.renderStore.hierarchy.length,
          transformMode: window.renderStore.transformMode,
          isGizmoDragging: window.renderStore.isGizmoDragging,
          hasEngine: !!window.renderStore.engine,
          hasScene: !!window.renderStore.scene,
          hasCamera: !!window.renderStore.camera
        };
      }

      // Viewport Store
      if (window.viewportStore) {
        data.viewportStore = {
          tabsCount: window.viewportStore.tabs.length,
          activeTabId: window.viewportStore.activeTabId,
          cameraPosition: [...window.viewportStore.camera.position],
          cameraTarget: [...window.viewportStore.camera.target]
        };
      }

      // Object Properties Store
      if (window.objectPropertiesStore) {
        data.objectPropertiesStore = {
          objectCount: Object.keys(window.objectPropertiesStore.objects).length,
          objectIds: Object.keys(window.objectPropertiesStore.objects)
        };
      }

      // Asset Store (if available)
      if (window.assetsStore) {
        data.assetsStore = {
          cacheSize: window.assetsStore.cache ? Object.keys(window.assetsStore.cache).length : 0,
          currentProject: window.assetsStore.currentProject
        };
      }

    } catch (error) {
      console.warn('⚠️ Error capturing store data:', error);
    }

    return data;
  }

  /**
   * Compare two snapshots and report potential leaks
   */
  _compareSnapshots(preSnapshot, postSnapshot) {
    console.log('🔍 Comparing snapshots for data leaks...');
    
    const issues = [];

    // Check each store for potential leaks
    this._checkEditorStoreLeaks(preSnapshot.data.editorStore, postSnapshot.data.editorStore, issues);
    this._checkRenderStoreLeaks(preSnapshot.data.renderStore, postSnapshot.data.renderStore, issues);
    this._checkViewportStoreLeaks(preSnapshot.data.viewportStore, postSnapshot.data.viewportStore, issues);
    this._checkObjectPropertiesLeaks(preSnapshot.data.objectPropertiesStore, postSnapshot.data.objectPropertiesStore, issues);

    // Report findings
    if (issues.length === 0) {
      console.log('✅ No data leaks detected between projects!');
    } else {
      console.warn('⚠️ Potential data leaks detected:');
      issues.forEach(issue => console.warn(`  - ${issue}`));
      
      // Also show detailed comparison for debugging
      console.group('📊 Detailed comparison:');
      console.log('Pre-switch:', preSnapshot.data);
      console.log('Post-switch:', postSnapshot.data);
      console.groupEnd();
    }
  }

  _checkEditorStoreLeaks(pre, post, issues) {
    if (!pre || !post) return;

    if (pre.selection.entity && post.selection.entity === pre.selection.entity) {
      issues.push('EditorStore: Selection entity not cleared');
    }

    if (pre.consoleMessages > 0 && post.consoleMessages >= pre.consoleMessages) {
      issues.push('EditorStore: Console messages not cleared');
    }

    if (pre.scriptExecution && post.scriptExecution) {
      issues.push('EditorStore: Script execution state not reset');
    }
  }

  _checkRenderStoreLeaks(pre, post, issues) {
    if (!pre || !post) return;

    if (pre.selectedObjectId && post.selectedObjectId === pre.selectedObjectId) {
      issues.push('RenderStore: Selected object not cleared');
    }

    if (pre.hierarchyCount > 0 && post.hierarchyCount >= pre.hierarchyCount) {
      issues.push('RenderStore: Hierarchy not properly cleared');
    }

    if (pre.transformMode !== 'select' && post.transformMode === pre.transformMode) {
      issues.push('RenderStore: Transform mode not reset');
    }
  }

  _checkViewportStoreLeaks(pre, post, issues) {
    if (!pre || !post) return;

    if (pre.tabsCount > 0 && post.tabsCount >= pre.tabsCount) {
      issues.push('ViewportStore: Tabs not cleared');
    }

    if (pre.activeTabId && post.activeTabId === pre.activeTabId) {
      issues.push('ViewportStore: Active tab ID not cleared');
    }
  }

  _checkObjectPropertiesLeaks(pre, post, issues) {
    if (!pre || !post) return;

    if (pre.objectCount > 0 && post.objectCount >= pre.objectCount) {
      issues.push('ObjectPropertiesStore: Object properties not cleared');
    }

    // Check for specific object ID overlaps
    if (pre.objectIds && post.objectIds) {
      const overlap = pre.objectIds.filter(id => post.objectIds.includes(id));
      if (overlap.length > 0) {
        issues.push(`ObjectPropertiesStore: ${overlap.length} object IDs persisted: ${overlap.join(', ')}`);
      }
    }
  }

  /**
   * Generate a report of current data store states
   */
  generateReport() {
    if (!this.enabled) {
      console.log('🔍 ProjectDataAuditor is disabled');
      return;
    }

    console.group('📊 Current Data Store Report');
    
    const currentData = this._captureAllStoreData();
    
    Object.entries(currentData).forEach(([storeName, storeData]) => {
      console.group(`📦 ${storeName}`);
      Object.entries(storeData).forEach(([key, value]) => {
        console.log(`${key}:`, value);
      });
      console.groupEnd();
    });
    
    console.groupEnd();
  }

  /**
   * Clean up old snapshots to prevent memory leaks
   */
  cleanup() {
    const now = Date.now();
    const maxAge = 5 * 60 * 1000; // 5 minutes

    for (const [key, snapshot] of this.snapshots.entries()) {
      if (now - snapshot.timestamp > maxAge) {
        this.snapshots.delete(key);
      }
    }
  }
}

// Create singleton instance
const projectDataAuditor = new ProjectDataAuditor();

// Auto-enable in development
if (typeof window !== 'undefined' && window.location.hostname === 'localhost') {
  projectDataAuditor.enable();
}

export { projectDataAuditor };