import { useState, useCallback } from 'react';

export const usePanelResize = (editorActions) => {
  const [isResizingBottom, setIsResizingBottom] = useState(false);
  const [isResizingRight, setIsResizingRight] = useState(false);

  const {
    setBottomPanelHeight,
    setRightPanelWidth,
    setAssetPanelOpen,
    setScenePanelOpen,
    setResizingPanels,
    setSelectedTool: setSelectedRightTool
  } = editorActions;

  // Bottom panel resize handlers
  const handleBottomResizeStart = useCallback(() => {
    console.log('🔵 Bottom panel resize START');
    setIsResizingBottom(true);
    setResizingPanels(true);
    document.body.classList.add('dragging-vertical');
    
    // Cancel any pending auto-saves
    import('@/plugins/core/AutoSaveManager.js').then(({ autoSaveManager }) => {
      autoSaveManager.cancelPendingAutoSave()
    })
  }, [setResizingPanels]);

  const handleBottomResizeMove = useCallback((e, { isAssetPanelOpen }) => {
    if (!isResizingBottom) return;
    e.preventDefault();
    
    const newHeight = window.innerHeight - e.clientY;
    const maxHeight = window.innerHeight * 0.85; // Allow up to 85% of viewport height
    const snapThreshold = 80; // Snap to hidden when within 80px of bottom edge
    const openThreshold = 120; // Snap to open when dragged up 120px
    
    if (!isAssetPanelOpen && newHeight > openThreshold) {
      setAssetPanelOpen(true);
      setBottomPanelHeight(Math.max(200, newHeight));
    } else if (isAssetPanelOpen && newHeight < snapThreshold) {
      setAssetPanelOpen(false);
      setIsResizingBottom(false);
      setResizingPanels(false);
      document.body.classList.remove('dragging-vertical');
    } else if (isAssetPanelOpen) {
      const constrainedHeight = Math.max(40, Math.min(maxHeight, newHeight));
      setBottomPanelHeight(constrainedHeight);
    }
  }, [isResizingBottom, setAssetPanelOpen, setBottomPanelHeight]);

  const handleBottomResizeEnd = useCallback(() => {
    console.log('🔴 Bottom panel resize END');
    setIsResizingBottom(false);
    setResizingPanels(false);
    document.body.classList.remove('dragging-vertical');
  }, [setResizingPanels]);

  // Right panel resize handlers
  const handleRightResizeStart = useCallback(() => {
    console.log('🟡 Right panel resize START');
    setIsResizingRight(true);
    setResizingPanels(true);
    document.body.classList.add('dragging-horizontal');
    
    // Cancel any pending auto-saves
    import('@/plugins/core/AutoSaveManager.js').then(({ autoSaveManager }) => {
      autoSaveManager.cancelPendingAutoSave()
    })
  }, [setResizingPanels]);

  const handleRightResizeMove = useCallback((e, { isScenePanelOpen, isLeftPanel = false }) => {
    if (!isResizingRight) return;
    
    // Calculate width based on panel position
    const newWidth = isLeftPanel 
      ? e.clientX // For left panel, width increases as mouse moves right
      : window.innerWidth - e.clientX; // For right panel, width increases as mouse moves left
    
    const snapThreshold = 100; // Snap to hidden when within 100px of edge
    const openThreshold = 150; // Snap to open when dragged 150px
    
    if (!isScenePanelOpen && newWidth > openThreshold) {
      setScenePanelOpen(true);
      setRightPanelWidth(Math.max(200, newWidth));
      setSelectedRightTool('scene'); // Activate scene tab when opening via resize
    } else if (isScenePanelOpen && newWidth < snapThreshold) {
      setScenePanelOpen(false);
      setIsResizingRight(false);
      setResizingPanels(false);
      document.body.classList.remove('dragging-horizontal');
      setSelectedRightTool('select'); // Deactivate menu when closing via resize
    } else if (isScenePanelOpen) {
      setRightPanelWidth(Math.max(200, Math.min(600, newWidth)));
    }
  }, [isResizingRight, setScenePanelOpen, setRightPanelWidth, setSelectedRightTool]);

  const handleRightResizeEnd = useCallback(() => {
    console.log('🟠 Right panel resize END');
    setIsResizingRight(false);
    setResizingPanels(false);
    document.body.classList.remove('dragging-horizontal');
  }, [setResizingPanels]);

  return {
    isResizingBottom,
    isResizingRight,
    handleBottomResizeStart,
    handleBottomResizeMove,
    handleBottomResizeEnd,
    handleRightResizeStart,
    handleRightResizeMove,
    handleRightResizeEnd
  };
};