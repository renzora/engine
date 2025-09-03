import { Show, For } from 'solid-js';
import { IconChevronRight } from '@tabler/icons-solidjs';

function AssetBreadcrumbs({ 
  breadcrumbs, 
  viewMode, 
  selectedCategory, 
  assetCategories,
  onBreadcrumbClick,
  dragOverBreadcrumb,
  setDragOverBreadcrumb,
  isInternalDrag,
  onBreadcrumbDrop
}) {
  const handleDragOver = (e, path) => {
    if (isInternalDrag()) {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      setDragOverBreadcrumb(path);
    }
  };

  const handleDragEnter = (e, path) => {
    if (isInternalDrag()) {
      e.preventDefault();
      setDragOverBreadcrumb(path);
    }
  };

  const handleDragLeave = (e) => {
    if (!e.currentTarget.contains(e.relatedTarget)) {
      setDragOverBreadcrumb(null);
    }
  };

  const handleDrop = (e, path) => {
    e.preventDefault();
    if (isInternalDrag()) {
      setDragOverBreadcrumb(null);
      onBreadcrumbDrop?.(e, path);
    }
  };

  console.log('🟡 AssetBreadcrumbs render - breadcrumbs:', breadcrumbs?.(), 'viewMode:', viewMode?.());
  
  return (
    <div class="flex items-center text-xs">
      <Show when={viewMode() === 'folder' && breadcrumbs().length > 0} fallback={
        <span class="text-base-content/60 px-2 py-1">
          {viewMode() === 'type' && assetCategories() && assetCategories()[selectedCategory()] 
            ? assetCategories()[selectedCategory()].name 
            : 'Assets'
          }
        </span>
      }>
        <For each={breadcrumbs()}>
          {(crumb, index) => (
            <>
              <button 
                onClick={() => onBreadcrumbClick(crumb.path)}
                class={`px-2 py-1 rounded transition-colors ${
                  dragOverBreadcrumb() === crumb.path
                    ? 'bg-primary/30 border border-primary border-dashed text-primary'
                    : index() === breadcrumbs().length - 1 
                      ? 'text-base-content font-medium hover:text-primary' 
                      : 'text-base-content/60 hover:text-primary'
                }`}
                onDragOver={(e) => handleDragOver(e, crumb.path)}
                onDragEnter={(e) => handleDragEnter(e, crumb.path)}
                onDragLeave={handleDragLeave}
                onDrop={(e) => handleDrop(e, crumb.path)}
              >
                {crumb.name}
              </button>
              <Show when={index() < breadcrumbs().length - 1}>
                <IconChevronRight class="w-3 h-3 mx-1 text-base-content/40" />
              </Show>
            </>
          )}
        </For>
      </Show>
    </div>
  );
}

export default AssetBreadcrumbs;