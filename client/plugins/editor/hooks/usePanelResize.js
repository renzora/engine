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

  const handleBottomResizeStart = useCallback(() => {
    console.log('🔵 Bottom panel resize START');
    setIsResizingBottom(true);
    setResizingPanels(true);
    document.body.classList.add('dragging-vertical');
    
    import('@/plugins/core/AutoSaveManager.js').then(({ autoSaveManager }) => {
      autoSaveManager.cancelPendingAutoSave()
    })
  }, [setResizingPanels]);

  const handleBottomResizeMove = useCallback((e, { isAssetPanelOpen }) => {
    if (!isResizingBottom) return;
    e.preventDefault();
    
    const newHeight = window.innerHeight - e.clientY;
    const maxHeight = window.innerHeight * 0.85;
    const snapThreshold = 80;
    const openThreshold = 120;
    
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

  const handleRightResizeStart = useCallback(() => {
    console.log('🟡 Right panel resize START');
    setIsResizingRight(true);
    setResizingPanels(true);
    document.body.classList.add('dragging-horizontal');
    
    import('@/plugins/core/AutoSaveManager.js').then(({ autoSaveManager }) => {
      autoSaveManager.cancelPendingAutoSave()
    })
  }, [setResizingPanels]);

  const handleRightResizeMove = useCallback((e, { isScenePanelOpen, isLeftPanel = false }) => {
    if (!isResizingRight) return;
    
    const newWidth = isLeftPanel 
      ? e.clientX
      : window.innerWidth - e.clientX;
    
    const snapThreshold = 100;
    const openThreshold = 150;
    
    if (!isScenePanelOpen && newWidth > openThreshold) {
      setScenePanelOpen(true);
      setRightPanelWidth(Math.max(200, newWidth));
      setSelectedRightTool('scene');
    } else if (isScenePanelOpen && newWidth < snapThreshold) {
      setScenePanelOpen(false);
      setIsResizingRight(false);
      setResizingPanels(false);
      document.body.classList.remove('dragging-horizontal');
      setSelectedRightTool('select');
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