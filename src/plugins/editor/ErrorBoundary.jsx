import { createSignal, ErrorBoundary as SolidErrorBoundary } from 'solid-js'

const ErrorFallback = (props) => {
  const [showDetails, setShowDetails] = createSignal(false)

  const handleRetry = () => {
    props.reset()
  }

  return (
    <div class="w-full h-full bg-red-900/20 border border-red-500/50 flex items-center justify-center">
      <div class="text-center p-6 max-w-2xl">
        <div class="text-red-400 text-lg font-semibold mb-4">
          Component Error
        </div>
        <div class="text-red-300 text-sm mb-4">
          {props.error && props.error.toString()}
        </div>
        <details class="text-left">
          <summary 
            class="text-red-300 cursor-pointer mb-2"
            onClick={() => setShowDetails(!showDetails())}
          >
            Error Details
          </summary>
          <pre class="text-xs text-red-200 bg-red-900/30 p-3 rounded overflow-auto">
            {props.error && props.error.stack}
            {props.info}
          </pre>
        </details>
        <button
          onClick={handleRetry}
          class="mt-4 px-4 py-2 bg-red-600 hover:bg-red-700 text-white rounded transition-colors"
        >
          Try Again
        </button>
      </div>
    </div>
  )
}

export default function ErrorBoundary(props) {
  const fallback = (err, reset) => {
    console.error('ErrorBoundary caught an error:', err)
    return <ErrorFallback error={err} reset={reset} />
  }

  return (
    <SolidErrorBoundary fallback={fallback}>
      {props.children}
    </SolidErrorBoundary>
  )
}