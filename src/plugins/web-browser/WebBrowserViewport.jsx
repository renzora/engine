import { createSignal, Show } from 'solid-js';

export default function WebBrowserViewport() {
  const [url, setUrl] = createSignal('https://example.com');
  const [isLoading, setIsLoading] = createSignal(false);
  const [hasError, setHasError] = createSignal(false);

  const handleUrlSubmit = (e) => {
    e.preventDefault();
    const formData = new FormData(e.target);
    const newUrl = formData.get('url');
    if (newUrl) {
      setUrl(newUrl.startsWith('http') ? newUrl : `https://${newUrl}`);
      setIsLoading(true);
      setHasError(false);
    }
  };

  const handleIframeLoad = () => {
    setIsLoading(false);
  };

  const handleIframeError = () => {
    setIsLoading(false);
    setHasError(true);
  };

  const openInNewTab = () => {
    window.open(url(), '_blank');
  };

  return (
    <div class="h-full flex flex-col bg-base-100">
      {/* URL Bar */}
      <div class="h-10 bg-base-200 border-b border-base-300 flex items-center px-2">
        <form onSubmit={handleUrlSubmit} class="flex-1 flex gap-2">
          <input
            type="text"
            name="url"
            value={url()}
            onInput={(e) => setUrl(e.target.value)}
            placeholder="Enter URL (try example.com, github.com, etc.)"
            class="flex-1 input input-sm bg-base-100 border-base-300 text-xs"
          />
          <button
            type="submit"
            class="btn btn-sm btn-primary"
            disabled={isLoading()}
          >
            {isLoading() ? 'Loading...' : 'Go'}
          </button>
          <button
            type="button"
            onClick={openInNewTab}
            class="btn btn-sm btn-ghost"
            title="Open in new tab"
          >
            ↗
          </button>
        </form>
      </div>

      {/* Browser Content */}
      <div class="flex-1 relative">
        <Show when={hasError()}>
          <div class="absolute inset-0 bg-base-100 flex flex-col items-center justify-center p-4">
            <div class="text-error text-lg mb-2">⚠️</div>
            <div class="text-base-content mb-2">Cannot load this website</div>
            <div class="text-base-content/60 text-sm text-center mb-4">
              Many sites (YouTube, Twitter, etc.) block embedding for security reasons
            </div>
            <button
              onClick={openInNewTab}
              class="btn btn-primary btn-sm"
            >
              Open in Browser Instead
            </button>
          </div>
        </Show>
        
        <Show when={isLoading()}>
          <div class="absolute inset-0 bg-base-100 flex items-center justify-center z-10">
            <div class="loading loading-spinner loading-md"></div>
          </div>
        </Show>
        
        <iframe
          src={url()}
          class="w-full h-full border-0"
          onLoad={handleIframeLoad}
          onError={handleIframeError}
          sandbox="allow-same-origin allow-scripts allow-popups allow-forms allow-downloads"
        />
      </div>
    </div>
  );
}