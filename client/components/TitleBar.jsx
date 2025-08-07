import { useState, useEffect } from 'react';

const TitleBar = () => {
  const [isMaximized, setIsMaximized] = useState(false);
  const [isElectron, setIsElectron] = useState(false);

  useEffect(() => {
    const electronCheck = window.electronAPI?.isElectron || false;
    setIsElectron(electronCheck);

    if (electronCheck && window.windowAPI) {
      window.windowAPI.isMaximized().then(setIsMaximized);

      const checkMaximized = () => {
        window.windowAPI.isMaximized().then(setIsMaximized);
      };

      const interval = setInterval(checkMaximized, 500);
      return () => clearInterval(interval);
    }
  }, []);

  const handleMinimize = () => {
    if (window.windowAPI) {
      window.windowAPI.minimize();
    }
  };

  const handleMaximize = () => {
    if (window.windowAPI) {
      window.windowAPI.maximize().then(() => {
        setTimeout(() => {
          window.windowAPI.isMaximized().then(setIsMaximized);
        }, 100);
      });
    }
  };

  const handleClose = () => {
    if (window.windowAPI) {
      window.windowAPI.close();
    }
  };

  if (!isElectron) {
    return null;
  }

  return (
    <div 
      className="flex items-center justify-between h-8 bg-slate-900 border-b border-slate-700 select-none"
      style={{ 
        WebkitAppRegion: 'drag',
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        zIndex: 10000
      }}
    >
      <div className="flex items-center px-4">
        <span className="text-sm text-gray-300 font-medium">
          Renzora Engine
        </span>
      </div>

      <div 
        className="flex"
        style={{ WebkitAppRegion: 'no-drag' }}
      >
        <button
          onClick={handleMinimize}
          className="w-12 h-8 flex items-center justify-center hover:bg-slate-700 transition-colors group"
          title="Minimize"
        >
          <svg 
            width="10" 
            height="1" 
            className="text-gray-400 group-hover:text-white"
          >
            <rect width="10" height="1" fill="currentColor" />
          </svg>
        </button>

        <button
          onClick={handleMaximize}
          className="w-12 h-8 flex items-center justify-center hover:bg-slate-700 transition-colors group"
          title={isMaximized ? "Restore" : "Maximize"}
        >
          {isMaximized ? (
            <svg 
              width="10" 
              height="10" 
              className="text-gray-400 group-hover:text-white"
            >
              <rect x="2" y="2" width="6" height="6" stroke="currentColor" strokeWidth="1" fill="none" />
              <rect x="0" y="0" width="6" height="6" stroke="currentColor" strokeWidth="1" fill="none" />
            </svg>
          ) : (
            <svg 
              width="10" 
              height="10" 
              className="text-gray-400 group-hover:text-white"
            >
              <rect width="10" height="10" stroke="currentColor" strokeWidth="1" fill="none" />
            </svg>
          )}
        </button>

        <button
          onClick={handleClose}
          className="w-12 h-8 flex items-center justify-center hover:bg-red-600 transition-colors group"
          title="Close"
        >
          <svg 
            width="10" 
            height="10" 
            className="text-gray-400 group-hover:text-white"
          >
            <line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" strokeWidth="1" />
            <line x1="10" y1="0" x2="0" y2="10" stroke="currentColor" strokeWidth="1" />
          </svg>
        </button>
      </div>
    </div>
  );
};

export default TitleBar;