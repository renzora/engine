import { createSignal, onMount } from 'solid-js';
import { Button } from '../../../components/ui/Button';
import { Section } from '../../../components/ui/Section';
import { Select } from '../../../components/ui/Select';
import { LoadingSpinner } from '../../../components/ui/LoadingSpinner';

export function UpdatePlugin() {
  const [updateConfig, setUpdateConfig] = createSignal(null);
  const [updateCheck, setUpdateCheck] = createSignal(null);
  const [isChecking, setIsChecking] = createSignal(false);
  const [isDownloading, setIsDownloading] = createSignal(false);
  const [isApplying, setIsApplying] = createSignal(false);
  const [error, setError] = createSignal(null);

  const loadConfig = async () => {
    try {
      const response = await fetch('http://localhost:3001/update/config');
      const data = await response.json();
      setUpdateConfig(data);
    } catch {
      setError('Failed to load update configuration');
    }
  };

  const checkForUpdates = async () => {
    setIsChecking(true);
    setError(null);
    try {
      const response = await fetch('http://localhost:3001/update/check');
      const data = await response.json();
      setUpdateCheck(data);
    } catch {
      setError('Failed to check for updates');
    } finally {
      setIsChecking(false);
    }
  };

  const setChannel = async (channel) => {
    try {
      const response = await fetch('http://localhost:3001/update/channel', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ channel }),
      });
      
      if (response.ok) {
        await loadConfig();
        setUpdateCheck(null);
      } else {
        setError('Failed to update channel');
      }
    } catch {
      setError('Failed to update channel');
    }
  };

  const downloadUpdate = async () => {
    if (!updateCheck()?.release) return;
    
    setIsDownloading(true);
    setError(null);
    try {
      const response = await fetch('http://localhost:3001/update/download', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ 
          version: updateCheck().release.version.to_string || updateCheck().release.tag_name 
        }),
      });
      
      if (!response.ok) {
        setError('Failed to download update');
      }
    } catch {
      setError('Failed to download update');
    } finally {
      setIsDownloading(false);
    }
  };

  const applyUpdate = async () => {
    setIsApplying(true);
    setError(null);
    try {
      const response = await fetch('http://localhost:3001/update/apply', {
        method: 'POST',
      });
      
      if (response.ok) {
        alert('Update applied successfully! Please restart the application.');
      } else {
        setError('Failed to apply update');
      }
    } catch {
      setError('Failed to apply update');
    } finally {
      setIsApplying(false);
    }
  };

  onMount(() => {
    loadConfig();
  });

  const formatVersion = (version) => {
    if (typeof version === 'string') return version;
    if (version?.to_string) return version.to_string;
    if (version?.major !== undefined) {
      return version.minor !== undefined ? `r${version.major}.${version.minor}` : `r${version.major}`;
    }
    return 'Unknown';
  };

  const getChannelBadgeColor = (channel) => {
    return channel === 'stable' ? 'bg-green-500' : 'bg-blue-500';
  };

  return (
    <div class="p-6 space-y-6">
      <div class="flex items-center justify-between">
        <h2 class="text-2xl font-bold text-white">Update Manager</h2>
        <div class="flex gap-2">
          <Button 
            onClick={checkForUpdates} 
            disabled={isChecking()}
            variant="outline"
          >
            {isChecking() ? <LoadingSpinner size="sm" /> : 'Check for Updates'}
          </Button>
        </div>
      </div>

      {error() && (
        <div class="bg-red-500/20 border border-red-500 rounded-lg p-4">
          <p class="text-red-200">{error()}</p>
        </div>
      )}

      <Section title="Current Configuration">
        <div class="space-y-4">
          {updateConfig() ? (
            <>
              <div class="flex items-center justify-between">
                <span class="text-gray-300">Current Version:</span>
                <span class="font-mono text-white">
                  {formatVersion(updateConfig().current_version)}
                </span>
              </div>
              
              <div class="flex items-center justify-between">
                <span class="text-gray-300">Update Channel:</span>
                <div class="flex items-center gap-2">
                  <span class={`px-2 py-1 rounded text-xs font-semibold text-white ${getChannelBadgeColor(updateConfig().update_channel)}`}>
                    {updateConfig().update_channel}
                  </span>
                  <Select
                    value={updateConfig().update_channel}
                    onChange={setChannel}
                    options={[
                      { value: 'stable', label: 'Stable (r1, r2, r3...)' },
                      { value: 'dev', label: 'Dev (r1.x, r2.x...)' }
                    ]}
                  />
                </div>
              </div>

              <div class="flex items-center justify-between">
                <span class="text-gray-300">GitHub Repository:</span>
                <span class="font-mono text-white">
                  {updateConfig().github_owner}/{updateConfig().github_repo}
                </span>
              </div>

              <div class="flex items-center justify-between">
                <span class="text-gray-300">Auto Update:</span>
                <span class="text-white">
                  {updateConfig().auto_update ? 'Enabled' : 'Disabled'}
                </span>
              </div>
            </>
          ) : (
            <LoadingSpinner />
          )}
        </div>
      </Section>

      {updateCheck() && (
        <Section title="Update Status">
          <div class="space-y-4">
            {updateCheck().update_available ? (
              <div class="bg-blue-500/20 border border-blue-500 rounded-lg p-4">
                <div class="flex items-center justify-between mb-4">
                  <h3 class="text-lg font-semibold text-blue-200">Update Available!</h3>
                  <span class="px-3 py-1 bg-blue-500 text-white rounded-full text-sm font-semibold">
                    {formatVersion(updateCheck().latest_version)}
                  </span>
                </div>
                
                {updateCheck().release && (
                  <div class="space-y-3">
                    <div>
                      <p class="text-blue-200 font-medium">{updateCheck().release.name}</p>
                      <p class="text-gray-300 text-sm mt-1">
                        Published: {new Date(updateCheck().release.published_at).toLocaleDateString()}
                      </p>
                    </div>
                    
                    {updateCheck().release.body && (
                      <div class="bg-gray-800 rounded p-3">
                        <p class="text-gray-300 text-sm whitespace-pre-wrap">
                          {updateCheck().release.body}
                        </p>
                      </div>
                    )}

                    <div class="flex gap-2 pt-2">
                      <Button 
                        onClick={downloadUpdate} 
                        disabled={isDownloading()}
                        variant="primary"
                      >
                        {isDownloading() ? <LoadingSpinner size="sm" /> : 'Download Update'}
                      </Button>
                      
                      <Button 
                        onClick={applyUpdate} 
                        disabled={isApplying()}
                        variant="secondary"
                      >
                        {isApplying() ? <LoadingSpinner size="sm" /> : 'Apply Update'}
                      </Button>
                    </div>
                  </div>
                )}
              </div>
            ) : (
              <div class="bg-green-500/20 border border-green-500 rounded-lg p-4">
                <p class="text-green-200">You're running the latest version!</p>
                <p class="text-gray-300 text-sm mt-1">
                  Current: {formatVersion(updateCheck().current_version)}
                </p>
              </div>
            )}
          </div>
        </Section>
      )}

      <Section title="Release Information">
        <div class="space-y-3">
          <div class="text-gray-300">
            <h4 class="font-semibold mb-2">Version Scheme:</h4>
            <ul class="space-y-1 text-sm">
              <li><strong>Stable Channel:</strong> r1, r2, r3... (major releases only)</li>
              <li><strong>Dev Channel:</strong> r1.1, r1.2, r1.49... (includes commits between releases)</li>
            </ul>
          </div>
          
          <div class="text-gray-300">
            <h4 class="font-semibold mb-2">Channel Selection:</h4>
            <ul class="space-y-1 text-sm">
              <li><strong>Stable:</strong> Recommended for production use - only get updates for official releases</li>
              <li><strong>Dev:</strong> Get the latest features and fixes as they're committed</li>
            </ul>
          </div>
        </div>
      </Section>
    </div>
  );
}