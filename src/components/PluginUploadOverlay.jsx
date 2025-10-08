import { createSignal, Show } from 'solid-js';
import { usePluginAPI } from '@/api/plugin/index.jsx';

export default function PluginUploadOverlay(props) {
  // Use external isOpen prop if provided, otherwise use internal state
  const [internalIsOpen, setInternalIsOpen] = createSignal(false);
  const isOpen = () => props.isOpen !== undefined ? props.isOpen : internalIsOpen();
  const setIsOpen = (value) => {
    if (props.onClose && !value) {
      props.onClose();
    } else if (props.isOpen === undefined) {
      setInternalIsOpen(value);
    }
  };
  const [isUploading, setIsUploading] = createSignal(false);
  const [uploadProgress, setUploadProgress] = createSignal(0);
  const [uploadStatus, setUploadStatus] = createSignal('');
  const [dragActive, setDragActive] = createSignal(false);
  
  const pluginAPI = usePluginAPI();

  const fileToBase64 = (file) => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.readAsDataURL(file);
      reader.onload = () => {
        // Remove the data:application/zip;base64, prefix
        const base64 = reader.result.split(',')[1];
        resolve(base64);
      };
      reader.onerror = error => reject(error);
    });
  };

  const handleFileSelect = async (file) => {
    if (!file || !file.name.endsWith('.zip')) {
      setUploadStatus('Please select a valid ZIP file');
      return;
    }

    await uploadPlugin(file);
  };

  const uploadPlugin = async (file) => {
    setIsUploading(true);
    setUploadProgress(0);
    setUploadStatus('Uploading plugin...');

    try {
      // Convert file to base64
      const base64Data = await fileToBase64(file);
      
      setUploadProgress(25);
      setUploadStatus('Converting plugin data...');

      // Create request body with base64 encoded ZIP
      const requestBody = {
        plugin: base64Data
      };

      // Upload to bridge backend
      const response = await fetch('http://localhost:3001/api/plugins/install', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(requestBody)
      });

      setUploadProgress(75);
      setUploadStatus('Installing plugin...');

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`Upload failed: ${response.statusText} - ${errorText}`);
      }

      const result = await response.json();
      setUploadProgress(90);
      setUploadStatus('Plugin installed successfully! Loading...');
      
      // Try to load the plugin immediately using the PluginAPI
      await pluginAPI.loadPluginDynamically(result.plugin_id, result.plugin_path, result.main_file);
      
      setUploadProgress(100);
      setUploadStatus(`Plugin "${result.plugin_id}" loaded successfully!`);
      
      // Close overlay after success
      setTimeout(() => {
        setIsOpen(false);
        setUploadStatus('');
        setUploadProgress(0);
      }, 2000);

    } catch (error) {
      console.error('Plugin upload failed:', error);
      setUploadStatus(`Upload failed: ${error.message}`);
    } finally {
      setIsUploading(false);
    }
  };


  const handleDragOver = (e) => {
    e.preventDefault();
    setDragActive(true);
  };

  const handleDragLeave = (e) => {
    e.preventDefault();
    setDragActive(false);
  };

  const handleDrop = (e) => {
    e.preventDefault();
    setDragActive(false);
    
    const files = Array.from(e.dataTransfer.files);
    if (files.length > 0) {
      handleFileSelect(files[0]);
    }
  };

  return (
    <>
      {/* Trigger Button - only show when operating in standalone mode */}
      <Show when={props.isOpen === undefined}>
        <button
          onClick={() => setIsOpen(true)}
          class="btn btn-primary btn-sm"
          title="Install Plugin"
        >
          📦 Install Plugin
        </button>
      </Show>

      {/* Overlay */}
      <Show when={isOpen()}>
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div class="bg-base-100 rounded-lg p-6 w-96 max-w-full mx-4">
            <div class="flex justify-between items-center mb-4">
              <h3 class="text-lg font-semibold">Install Plugin</h3>
              <button
                onClick={() => setIsOpen(false)}
                class="btn btn-sm btn-ghost"
                disabled={isUploading()}
              >
                ✕
              </button>
            </div>

            <Show when={!isUploading()}>
              {/* File Upload Area */}
              <div
                class={`border-2 border-dashed rounded-lg p-8 text-center transition-colors ${
                  dragActive() 
                    ? 'border-primary bg-primary bg-opacity-10' 
                    : 'border-base-300'
                }`}
                onDragOver={handleDragOver}
                onDragLeave={handleDragLeave}
                onDrop={handleDrop}
              >
                <div class="mb-4">
                  <div class="text-4xl mb-2">📦</div>
                  <p class="text-base-content mb-2">
                    Drop plugin ZIP file here or click to browse
                  </p>
                  <p class="text-sm text-base-content opacity-60">
                    Supports .zip files only
                  </p>
                </div>
                
                <input
                  type="file"
                  accept=".zip"
                  onChange={(e) => handleFileSelect(e.target.files[0])}
                  class="file-input file-input-bordered w-full"
                />
              </div>
            </Show>

            <Show when={isUploading()}>
              {/* Upload Progress */}
              <div class="text-center">
                <div class="mb-4">
                  <div class="loading loading-spinner loading-lg"></div>
                </div>
                <div class="mb-4">
                  <progress 
                    class="progress progress-primary w-full" 
                    value={uploadProgress()} 
                    max="100"
                  ></progress>
                  <p class="text-sm mt-1">{uploadProgress()}%</p>
                </div>
                <p class="text-sm text-base-content">{uploadStatus()}</p>
              </div>
            </Show>

            <Show when={uploadStatus() && !isUploading()}>
              <div class="mt-4 p-3 rounded bg-base-200">
                <p class="text-sm text-center">{uploadStatus()}</p>
              </div>
            </Show>

            {/* Instructions */}
            <div class="mt-6 text-xs text-base-content opacity-60">
              <p class="mb-1">• Plugin ZIP should contain an index.jsx file</p>
              <p class="mb-1">• Plugin will be installed and loaded immediately</p>
              <p>• Restart app to include in build permanently</p>
            </div>
          </div>
        </div>
      </Show>
    </>
  );
}