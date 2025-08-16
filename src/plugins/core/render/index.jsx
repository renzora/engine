import { createSignal, createContext, useContext } from 'solid-js';
import ViewportCanvas from './components/ViewportCanvas';

// Engine context - moved from engine folder to render plugin where it belongs
const EngineContext = createContext();

export const useEngine = () => {
  const context = useContext(EngineContext);
  if (!context) {
    console.warn('EngineContext not found, returning default values');
    const [defaultReady] = createSignal(true);
    return { isEngineReady: defaultReady, isLoading: () => false, error: () => null };
  }
  return context;
};

// Backward compatibility
export const useEngineReady = () => {
  const { isEngineReady } = useEngine();
  return { isEngineReady };
};

function Viewport(props) {
  return <ViewportCanvas {...props} />
}

export default function RenderPlugin(props) {
  // Provide engine context at the render plugin level
  const [isReady, setIsReady] = createSignal(true);
  const [isLoading, setIsLoading] = createSignal(false);
  const [error, setError] = createSignal(null);

  const contextValue = {
    isEngineReady: isReady,
    isLoading,
    error
  };

  if (props.embedded) {
    return (
      <EngineContext.Provider value={contextValue}>
        <Viewport style={props.style} onContextMenu={props.onContextMenu}>{props.children}</Viewport>
      </EngineContext.Provider>
    );
  }

  const defaultStyle = props.viewportBounds ? {
    position: 'fixed',
    top: props.viewportBounds.top || 0,
    left: props.viewportBounds.left || 0,
    right: props.viewportBounds.right || 0,
    bottom: props.viewportBounds.bottom || 0,
    width: 'auto',
    height: 'auto'
  } : { width: '100vw', height: '100vh' };

  return (
    <EngineContext.Provider value={contextValue}>
      <Viewport style={{ ...defaultStyle, ...props.style }} onContextMenu={props.onContextMenu}>
        {props.children}
      </Viewport>
    </EngineContext.Provider>
  );
}

export { Viewport as ViewportCanvas };