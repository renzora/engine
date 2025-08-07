import { useState, useEffect } from 'react'

export default function DevelopmentDisclaimer({ onDismiss }) {
  const [isVisible, setIsVisible] = useState(false)
  const [isClosing, setIsClosing] = useState(false)

  useEffect(() => {
    // Check if user has already dismissed the disclaimer
    const hasBeenDismissed = localStorage.getItem('dev-disclaimer-dismissed')
    
    if (!hasBeenDismissed) {
      // Show disclaimer after a brief delay to allow UI to load
      const timer = setTimeout(() => {
        setIsVisible(true)
      }, 1000)
      
      return () => clearTimeout(timer)
    }
  }, [])

  const handleDismiss = () => {
    setIsClosing(true)
    
    // Store dismissal in localStorage
    localStorage.setItem('dev-disclaimer-dismissed', 'true')
    
    // Fade out animation
    setTimeout(() => {
      setIsVisible(false)
      onDismiss?.()
    }, 300)
  }

  if (!isVisible) return null

  return (
    <div className={`fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-[100] transition-opacity duration-300 ${
      isClosing ? 'opacity-0' : 'opacity-100'
    }`}>
      <div className={`bg-gray-900/95 border border-yellow-500/50 rounded-lg p-6 max-w-lg w-full mx-4 shadow-2xl transform transition-all duration-300 ${
        isClosing ? 'scale-95 opacity-0' : 'scale-100 opacity-100'
      }`}>
        
        {/* Warning Icon */}
        <div className="flex items-center gap-3 mb-4">
          <div className="w-8 h-8 bg-yellow-500/20 rounded-full flex items-center justify-center">
            <svg className="w-5 h-5 text-yellow-400" fill="currentColor" viewBox="0 0 20 20">
              <path fillRule="evenodd" d="M8.485 3.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 3.495zM10 6a.75.75 0 01.75.75v3.5a.75.75 0 01-1.5 0v-3.5A.75.75 0 0110 6zm0 9a1 1 0 100-2 1 1 0 000 2z" clipRule="evenodd" />
            </svg>
          </div>
          <h3 className="text-lg font-semibold text-yellow-400">Development Version</h3>
        </div>

        {/* Disclaimer Text */}
        <div className="space-y-3 mb-6">
          <p className="text-gray-200 text-sm leading-relaxed">
            <strong>This is an early development version</strong> of Renzora Engine and is intended for 
            <strong className="text-yellow-400"> evaluation purposes only</strong>.
          </p>
          
          <p className="text-gray-300 text-sm leading-relaxed">
            You should expect:
          </p>
          
          <ul className="text-gray-300 text-sm space-y-1 ml-4">
            <li className="flex items-start gap-2">
              <span className="text-yellow-400 mt-1">•</span>
              <span>Breaking changes between versions</span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-yellow-400 mt-1">•</span>
              <span>Incomplete or experimental features</span>
            </li>
            <li className="flex items-start gap-2">
              <span className="text-yellow-400 mt-1">•</span>
              <span>Potential bugs and stability issues</span>
            </li>
          </ul>
        </div>

        {/* Dismiss Button */}
        <div className="flex justify-end">
          <button
            onClick={handleDismiss}
            className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm rounded-md transition-colors duration-200 font-medium"
          >
            I Understand
          </button>
        </div>
      </div>
    </div>
  )
}