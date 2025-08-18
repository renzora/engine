import { createSignal, createContext, useContext } from 'solid-js';
import { createPlugin } from '@/api/plugin';
import ViewportCanvas from './components/ViewportCanvas';

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

export const useEngineReady = () => {
  const { isEngineReady } = useEngine();
  return { isEngineReady };
};

function Viewport(props) {
  return <ViewportCanvas {...props} />
}

export default createPlugin({
  id: 'core-render-plugin',
  name: 'Core Render Plugin',
  version: '1.0.0',
  description: 'Core rendering functionality for Renzora Engine',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[RenderPlugin] Initializing core render plugin...');
  },

  async onStart() {
    console.log('[RenderPlugin] Starting core render plugin...');
  },

  onUpdate() {
    // Render loop updates if needed
  },

  async onStop() {
    console.log('[RenderPlugin] Stopping core render plugin...');
  },

  async onDispose() {
    console.log('[RenderPlugin] Disposing core render plugin...');
  }
});

export function RenderProvider(props) {
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