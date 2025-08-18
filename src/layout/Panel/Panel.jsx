import { createSignal, Show } from 'solid-js';
import { cn } from '../../utils/cn';

export const Panel = (props) => {
  const [isCollapsed, setIsCollapsed] = createSignal(false);
  
  return (
    <div
      className={cn(
        'bg-[rgb(var(--panel))] border border-[rgb(var(--panel-border))] rounded-md overflow-hidden',
        props.className
      )}
      style={props.style}
    >
      {props.title && (
        <div className="flex items-center justify-between px-3 py-2 border-b border-[rgb(var(--panel-border))] bg-[rgb(var(--panel-secondary))]">
          <h3 className="text-sm font-medium text-[rgb(var(--text-primary))]">
            {props.title}
          </h3>
          
          <div className="flex items-center gap-1">
            {props.actions && props.actions}
            
            {props.collapsible && (
              <button
                onClick={() => setIsCollapsed(!isCollapsed())}
                className="p-1 hover:bg-[rgb(var(--surface-hover))] rounded transition-colors"
                aria-label={isCollapsed() ? 'Expand' : 'Collapse'}
              >
                <svg
                  className={cn(
                    'w-3 h-3 text-[rgb(var(--text-secondary))] transition-transform',
                    isCollapsed() && 'rotate-180'
                  )}
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M5 15l7-7 7 7"
                  />
                </svg>
              </button>
            )}
            
            {props.closable && (
              <button
                onClick={props.onClose}
                className="p-1 hover:bg-[rgb(var(--danger))] hover:text-white rounded transition-colors"
                aria-label="Close"
              >
                <svg
                  className="w-3 h-3"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            )}
          </div>
        </div>
      )}
      
      <Show when={!isCollapsed()}>
        <div className={cn('p-3', props.contentClassName)}>
          {props.children}
        </div>
      </Show>
    </div>
  );
};

export const PanelGroup = (props) => {
  return (
    <div
      className={cn(
        'space-y-3',
        props.horizontal && 'space-y-0 space-x-3 flex',
        props.className
      )}
    >
      {props.children}
    </div>
  );
};
