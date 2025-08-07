import React from 'react';

class ErrorBoundary extends React.Component {
  constructor(props) {
    super(props);
    this.state = { hasError: false, error: null, errorInfo: null };
  }

  static getDerivedStateFromError(error) {
    return { hasError: true };
  }

  componentDidCatch(error, errorInfo) {
    console.error('ErrorBoundary caught an error:', error, errorInfo);
    this.setState({
      error: error,
      errorInfo: errorInfo
    });
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="w-full h-full bg-red-900/20 border border-red-500/50 flex items-center justify-center">
          <div className="text-center p-6 max-w-2xl">
            <div className="text-red-400 text-lg font-semibold mb-4">
              Component Error
            </div>
            <div className="text-red-300 text-sm mb-4">
              {this.state.error && this.state.error.toString()}
            </div>
            <details className="text-left">
              <summary className="text-red-300 cursor-pointer mb-2">
                Error Details
              </summary>
              <pre className="text-xs text-red-200 bg-red-900/30 p-3 rounded overflow-auto">
                {this.state.error && this.state.error.stack}
                {this.state.errorInfo.componentStack}
              </pre>
            </details>
            <button
              onClick={() => this.setState({ hasError: false, error: null, errorInfo: null })}
              className="mt-4 px-4 py-2 bg-red-600 hover:bg-red-700 text-white rounded transition-colors"
            >
              Try Again
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;