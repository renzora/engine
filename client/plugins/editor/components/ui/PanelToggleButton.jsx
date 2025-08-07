import React from 'react';

const PanelToggleButton = ({ onClick, position, className = '', isLeftPanel = false }) => {
  return (
    <div 
      className={`absolute w-6 pointer-events-auto z-30 ${className}`}
      style={{ top: '80px', ...position }}
    >
      <button
        onClick={onClick}
        className="w-6 h-8 text-gray-400 hover:text-blue-400 transition-colors flex items-center justify-center group relative"
        style={{ 
          backgroundColor: '#1e293b',
          borderLeft: '1px solid #475569',
          borderTop: '1px solid #475569',
          borderBottom: '1px solid #475569',
          borderTopLeftRadius: '6px',
          borderBottomLeftRadius: '6px'
        }}
        title="Open panel"
      >
        <div className="w-3 h-3 flex items-center justify-center">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3 h-3">
            <path d="m15 18-6-6 6-6"/>
          </svg>
        </div>
        
        {/* Tooltip */}
        <div className={`absolute ${isLeftPanel ? 'right-full mr-1' : 'right-full mr-1'} top-1/2 -translate-y-1/2 bg-slate-900/95 backdrop-blur-sm border border-slate-600 text-white text-xs px-3 py-1.5 rounded-md opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap shadow-2xl`} 
             style={{ zIndex: 50 }}>
          Open panel
          {/* Arrow pointing to the button */}
          <div className={`absolute ${isLeftPanel ? 'left-full' : 'left-full'} top-1/2 -translate-y-1/2 w-0 h-0 ${isLeftPanel ? 'border-l-4 border-l-slate-900' : 'border-l-4 border-l-slate-900'} border-t-4 border-t-transparent border-b-4 border-b-transparent`}></div>
        </div>
      </button>
    </div>
  );
};

export default PanelToggleButton;