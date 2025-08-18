import { createSignal, For, Show } from 'solid-js';
import { useComponentTheme } from '../src/ui/hooks/useComponentTheme.js';

export function TreeView(props) {
  const { data = [] } = props;
  const containerTheme = useComponentTheme('TreeView', 'base');
  const itemTheme = useComponentTheme('TreeView', 'item');
  const expandTheme = useComponentTheme('TreeView', 'expand');
  
  return (
    <div 
      className="rounded-md border p-2"
      style={containerTheme.style}
    >
      <For each={data}>
        {(item) => item ? <TreeItem item={item} /> : null}
      </For>
    </div>
  );
}

function TreeItem(props) {
  const { item } = props;
  const [isExpanded, setIsExpanded] = createSignal(false);
  
  const itemTheme = useComponentTheme('TreeView', 'item');
  const expandTheme = useComponentTheme('TreeView', 'expand');
  
  if (!item) return null;
  
  const hasChildren = item.children && item.children.length > 0;
  
  return (
    <div>
      <div 
        className="flex items-center px-2 py-1 rounded cursor-pointer"
        style={itemTheme.style}
        onMouseEnter={itemTheme.onMouseEnter}
        onMouseLeave={itemTheme.onMouseLeave}
        onClick={() => hasChildren && setIsExpanded(!isExpanded())}
      >
        {hasChildren ? (
          <button 
            className="mr-2 p-0.5 rounded"
            style={expandTheme.style}
          >
            <svg 
              className={`w-4 h-4 transition-transform ${isExpanded() ? 'rotate-90' : ''}`}
              fill="none" 
              viewBox="0 0 24 24" 
              stroke="currentColor"
            >
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
            </svg>
          </button>
        ) : (
          <div className="w-6 h-6" />
        )}
        
        <span className="flex-1">{item.name || 'Unnamed Item'}</span>
      </div>
      
      <Show when={hasChildren && isExpanded()}>
        <div className="ml-6 border-l border-current opacity-30">
          <For each={item.children}>
            {(child) => child ? <TreeItem item={child} /> : null}
          </For>
        </div>
      </Show>
    </div>
  );
}
