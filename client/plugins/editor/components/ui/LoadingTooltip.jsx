import React from 'react'

const LoadingTooltip = ({ isVisible, message, position, progress }) => {
  if (!isVisible) return null

  const hasProgress = progress !== undefined && progress !== null
  const progressPercent = hasProgress ? Math.round(progress * 100) : 0

  return (
    <div
      className="fixed z-50 bg-gray-900/95 border border-gray-700 rounded-lg px-3 py-2 shadow-lg pointer-events-none transition-opacity duration-200"
      style={{
        left: position?.x || 0,
        top: position?.y || 0,
        transform: 'translate(-50%, -100%)',
        marginTop: '-8px'
      }}
    >
      <div className="flex items-center gap-2 mb-1">
        <div className="w-3 h-3 border-2 border-blue-500 border-t-transparent rounded-full animate-spin"></div>
        
        <span className="text-xs text-gray-200 whitespace-nowrap">
          {message || 'Loading...'}
        </span>
        
        {hasProgress && (
          <span className="text-xs text-blue-400 font-mono">
            {progressPercent}%
          </span>
        )}
      </div>
      
      {hasProgress && (
        <div className="w-full bg-gray-800 rounded-full h-1 overflow-hidden">
          <div 
            className="h-full bg-gradient-to-r from-blue-500 to-purple-500 rounded-full transition-all duration-300 ease-out"
            style={{ width: `${progressPercent}%` }}
          />
        </div>
      )}
      
      <div className="absolute left-1/2 transform -translate-x-1/2 border-t-4 border-t-gray-900 border-x-4 border-x-transparent bottom-[-4px]"></div>
    </div>
  )
}

export default LoadingTooltip