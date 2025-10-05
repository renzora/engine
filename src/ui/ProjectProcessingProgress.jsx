import { Show, createSignal, createEffect } from 'solid-js';
import { formatEstimatedTime, formatCacheStatus } from '@/api/bridge/projectCache.js';

const ProjectProcessingProgress = (props) => {
  const [animatedProgress, setAnimatedProgress] = createSignal(0);

  // Smooth progress animation
  createEffect(() => {
    const targetProgress = props.progress || 0;
    const current = animatedProgress();
    
    if (Math.abs(targetProgress - current) > 0.01) {
      const step = (targetProgress - current) * 0.1;
      const nextProgress = current + step;
      setAnimatedProgress(nextProgress);
      
      // Continue animation
      setTimeout(() => setAnimatedProgress(nextProgress), 50);
    }
  });

  const getProgressColor = () => {
    const progress = props.progress || 0;
    if (progress < 0.3) return 'bg-blue-500';
    if (progress < 0.7) return 'bg-yellow-500';
    return 'bg-green-500';
  };

  const getStatusIcon = () => {
    if (props.error) return '❌';
    if (props.completed) return '✅';
    if (props.processing) return '🔄';
    return '📦';
  };

  return (
    <div class="w-full max-w-md mx-auto bg-white rounded-lg shadow-lg p-6">
      {/* Header */}
      <div class="flex items-center mb-4">
        <span class="text-2xl mr-3">{getStatusIcon()}</span>
        <div class="flex-1">
          <h3 class="text-lg font-semibold text-gray-800">
            {props.title || 'Processing Project'}
          </h3>
          <p class="text-sm text-gray-600">
            {props.projectName || 'Unknown Project'}
          </p>
        </div>
      </div>

      {/* Progress Bar */}
      <Show when={props.processing && !props.error}>
        <div class="mb-4">
          <div class="flex justify-between text-sm text-gray-600 mb-1">
            <span>Progress</span>
            <span>{Math.round((props.progress || 0) * 100)}%</span>
          </div>
          <div class="w-full bg-gray-200 rounded-full h-2">
            <div 
              class={`h-2 rounded-full transition-all duration-300 ease-out ${getProgressColor()}`}
              style={{ width: `${(animatedProgress() || 0) * 100}%` }}
            ></div>
          </div>
        </div>
      </Show>

      {/* Status Message */}
      <div class="mb-4">
        <p class="text-sm text-gray-700">
          <Show when={props.error} fallback={
            <Show when={props.completed} fallback={
              <span>{props.currentStage || props.message || 'Processing...'}</span>
            }>
              <span class="text-green-600">✓ {props.message || 'Processing completed successfully!'}</span>
            </Show>
          }>
            <span class="text-red-600">⚠ {props.error}</span>
          </Show>
        </p>
      </div>

      {/* Processing Details */}
      <Show when={props.processing && !props.error}>
        <div class="space-y-2">
          <Show when={props.currentFile}>
            <div class="text-xs text-gray-500">
              <span class="font-medium">Current:</span> {props.currentFile}
            </div>
          </Show>
          
          <Show when={props.filesProcessed !== undefined && props.totalFiles !== undefined}>
            <div class="text-xs text-gray-500">
              <span class="font-medium">Files:</span> {props.filesProcessed} of {props.totalFiles}
            </div>
          </Show>

          <Show when={props.estimatedTimeRemaining}>
            <div class="text-xs text-gray-500">
              <span class="font-medium">Time remaining:</span> {formatEstimatedTime(props.estimatedTimeRemaining)}
            </div>
          </Show>
        </div>
      </Show>

      {/* Cache Status Details */}
      <Show when={props.cacheStatus && !props.processing}>
        <div class="border-t pt-4 mt-4">
          <div class="text-sm">
            <div class="flex justify-between items-center mb-2">
              <span class="text-gray-600">Cache Status:</span>
              <span class={`font-medium ${props.cacheStatus === 'valid' ? 'text-green-600' : 'text-yellow-600'}`}>
                {formatCacheStatus(props.cacheStatus)}
              </span>
            </div>
            
            <Show when={props.changesDetected > 0}>
              <div class="text-xs text-gray-500 space-y-1">
                <div>Changes detected: {props.changesDetected}</div>
                <Show when={props.changeSummary}>
                  <div class="grid grid-cols-2 gap-2">
                    <Show when={props.changeSummary.new_files > 0}>
                      <span>📄 {props.changeSummary.new_files} new</span>
                    </Show>
                    <Show when={props.changeSummary.modified_files > 0}>
                      <span>✏️ {props.changeSummary.modified_files} modified</span>
                    </Show>
                    <Show when={props.changeSummary.deleted_files > 0}>
                      <span>🗑️ {props.changeSummary.deleted_files} deleted</span>
                    </Show>
                    <Show when={props.changeSummary.moved_files > 0}>
                      <span>📁 {props.changeSummary.moved_files} moved</span>
                    </Show>
                  </div>
                </Show>
              </div>
            </Show>
          </div>
        </div>
      </Show>

      {/* Action Buttons */}
      <Show when={props.onCancel || props.onRetry || props.onContinue}>
        <div class="flex gap-3 mt-6">
          <Show when={props.onCancel && props.processing}>
            <button 
              onClick={props.onCancel}
              class="flex-1 px-4 py-2 text-sm bg-gray-100 text-gray-700 rounded hover:bg-gray-200 transition-colors"
            >
              Cancel
            </button>
          </Show>
          
          <Show when={props.onRetry && props.error}>
            <button 
              onClick={props.onRetry}
              class="flex-1 px-4 py-2 text-sm bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
            >
              Retry
            </button>
          </Show>
          
          <Show when={props.onContinue && (props.completed || props.error)}>
            <button 
              onClick={props.onContinue}
              class="flex-1 px-4 py-2 text-sm bg-green-500 text-white rounded hover:bg-green-600 transition-colors"
            >
              {props.error ? 'Continue Anyway' : 'Continue'}
            </button>
          </Show>
        </div>
      </Show>
    </div>
  );
};

export default ProjectProcessingProgress;