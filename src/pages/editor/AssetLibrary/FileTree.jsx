import { Show, For } from 'solid-js';
import { Folder, ChevronRight } from '@/ui/icons';

function FileTree({ 
  node, 
  depth = 0,
  expandedFolders,
  currentPath,
  dragOverTreeFolder,
  isInternalDrag,
  viewMode,
  onFolderClick,
  onFolderToggle,
  onDragOver,
  onDragEnter,
  onDragLeave,
  onDrop
}) {
  if (!node) return null;

  const isExpanded = () => expandedFolders().has(node.path);
  const isSelected = () => currentPath() === node.path;
  const hasChildren = node.children && node.children.length > 0;
  
  const handleDragOver = (e) => {
    if (isInternalDrag() && viewMode() === 'folder') {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      onDragOver?.(node.path);
    }
  };

  const handleDragEnter = (e) => {
    if (isInternalDrag() && viewMode() === 'folder') {
      e.preventDefault();
      onDragEnter?.(node.path);
    }
  };

  const handleDragLeave = (e) => {
    if (!e.currentTarget.contains(e.relatedTarget)) {
      onDragLeave?.();
    }
  };

  const handleDrop = (e) => {
    e.preventDefault();
    if (isInternalDrag() && viewMode() === 'folder') {
      onDrop?.(e, node.path);
    }
  };
  
  return (
    <div class="select-none relative">
      <div
        class={`flex items-center py-0.5 pr-6 text-xs cursor-pointer transition-colors relative overflow-hidden ${ 
          dragOverTreeFolder() === node.path 
            ? 'bg-primary/50'
            : isSelected() 
              ? 'bg-primary text-primary-content' 
              : 'text-base-content/70 hover:bg-base-200 hover:text-base-content'
        }`}
        style={{ 'padding-left': `${6 + depth * 16}px` }}
        onClick={() => {
          onFolderClick(node.path);
          if (hasChildren) {
            onFolderToggle(node.path);
          }
        }}
        onDragOver={handleDragOver}
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      >
        <Show when={isSelected()}>
          <div class="absolute left-0 top-0 bottom-0 w-0.5 bg-primary pointer-events-none" />
        </Show>
        
        <Show when={depth > 0}>
          <div class="absolute left-0 top-0 bottom-0 pointer-events-none">
            <div
              class="absolute top-0 bottom-0 w-px bg-base-content/30"
              style={{ left: `${6 + (depth - 1) * 16 + 8}px` }}
            />
            <div
              class="absolute top-1/2 w-2 h-px bg-base-content/30"
              style={{ left: `${6 + (depth - 1) * 16 + 8}px` }}
            />
          </div>
        </Show>
        
        <div class="relative flex items-center">
          <Folder class={`w-3 h-3 mr-1.5 ${
            isSelected() ? 'text-primary-content' : 'text-warning'
          }`} />
          <button
            onClick={(e) => {
              e.stopPropagation();
              if (hasChildren) {
                onFolderToggle(node.path);
              }
            }}
            class="absolute -left-0.5 p-0.5 rounded transition-all duration-200 hover:bg-base-200/50"
          >
            <ChevronRight 
              class={`w-2.5 h-2.5 transition-all duration-200 ${
                hasChildren && isExpanded() 
                  ? 'rotate-90 text-primary' 
                  : hasChildren
                    ? 'text-base-content/50 hover:text-base-content/70'
                    : 'text-base-content/20'
              }`} 
            />
          </button>
        </div>
        <span class="flex-1 text-base-content/80 truncate">{node.name}</span>
      </div>
      
      <Show when={hasChildren && isExpanded()}>
        <div class="transition-all duration-300 ease-out">
          <For each={node.children}>
            {(child) => (
              <FileTree 
                node={child}
                depth={depth + 1}
                expandedFolders={expandedFolders}
                currentPath={currentPath}
                dragOverTreeFolder={dragOverTreeFolder}
                isInternalDrag={isInternalDrag}
                viewMode={viewMode}
                onFolderClick={onFolderClick}
                onFolderToggle={onFolderToggle}
                onDragOver={onDragOver}
                onDragEnter={onDragEnter}
                onDragLeave={onDragLeave}
                onDrop={onDrop}
              />
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}

export default FileTree;