import { createSignal, createEffect, onMount, onCleanup, Show, For } from 'solid-js';
import { Button, Select, LoadingSpinner } from '@/ui';

export function UpdateModal(props) {
  const [updateConfig, setUpdateConfig] = createSignal(null);
  const [latestVersion, setLatestVersion] = createSignal(null);
  const [isChecking, setIsChecking] = createSignal(false);
  const [isDownloading, setIsDownloading] = createSignal(false);
  const [error, setError] = createSignal(null);
  const [releases, setReleases] = createSignal([]);
  const [currentVersion] = createSignal('Current Build'); // Current version is determined by bridge startup

  // Load initial config
  onMount(async () => {
    await loadConfig();
    await checkForUpdates();
  });
  
  // Clean up event listener on unmount
  onCleanup(() => {
    // No cleanup needed for custom events
  });

  const loadConfig = async () => {
    try {
      const response = await fetch('http://localhost:3001/update/config');
      if (response.ok) {
        const data = await response.json();
        setUpdateConfig(data);
      }
    } catch (err) {
      console.warn('Failed to load update config:', err);
      // Fallback config
      setUpdateConfig({
        github_owner: 'renzora',
        github_repo: 'engine',
        current_version: { major: 1, minor: null, channel: 'stable' },
        update_channel: 'stable',
        auto_update: false,
        check_interval_hours: 24
      });
    }
  };

  const checkForUpdates = async () => {
    setIsChecking(true);
    setError(null);
    
    try {
      const config = updateConfig();
      if (!config) return;

      // Get bridge startup time
      const bridgeResponse = await fetch('http://localhost:3001/startup-time');
      if (!bridgeResponse.ok) {
        throw new Error('Failed to get bridge startup time');
      }
      const { startup_time_ms } = await bridgeResponse.json();

      // Get latest commit from appropriate branch
      const branch = config.update_channel === 'dev' ? 'dev' : 'main';
      const commitsUrl = `https://api.github.com/repos/${config.github_owner}/${config.github_repo}/commits?sha=${branch}&per_page=1`;
      
      const commitsResponse = await fetch(commitsUrl, {
        headers: {
          'Accept': 'application/vnd.github.v3+json',
          'User-Agent': 'Renzora-Engine-Update-Client'
        }
      });

      if (!commitsResponse.ok) {
        throw new Error(`GitHub API error: ${commitsResponse.status}`);
      }

      const commits = await commitsResponse.json();
      if (!commits || commits.length === 0) {
        throw new Error('No commits found');
      }

      const latestCommit = commits[0];
      const latestCommitTime = new Date(latestCommit.commit.author.date).getTime();
      
      // Check if update is available (commit is newer than bridge startup)
      const isUpdateAvailable = latestCommitTime > startup_time_ms;
      
      console.log('🔄 Update Check Results (Timestamp-based):');
      console.log(`Bridge started at: ${new Date(startup_time_ms).toISOString()}`);
      console.log(`Latest commit at: ${new Date(latestCommitTime).toISOString()}`);
      console.log(`Active channel: ${config.update_channel} (${branch} branch)`);
      console.log(`Update available: ${isUpdateAvailable}`);
      console.log(`Latest commit: ${latestCommit.sha.substring(0, 8)} by ${latestCommit.commit.author.name}`);

      // Create a mock "release" structure for the UI using commit data
      const mockRelease = {
        tag_name: `${branch}-${latestCommit.sha.substring(0, 8)}`,
        name: `${config.update_channel === 'dev' ? 'Dev' : 'Stable'} Update - ${latestCommit.commit.message.split('\n')[0]}`,
        body: latestCommit.commit.message,
        published_at: latestCommit.commit.author.date,
        html_url: `https://github.com/${config.github_owner}/${config.github_repo}/commit/${latestCommit.sha}`,
        assets: [] // No assets for commits
      };

      setReleases([mockRelease]); // Set as array for UI compatibility
      
      if (isUpdateAvailable) {
        setLatestVersion(mockRelease);
      } else {
        setLatestVersion(null); // No update available
      }
    } catch (err) {
      setError(`Failed to check for updates: ${err.message}`);
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
        const newConfig = { ...updateConfig(), update_channel: channel };
        setUpdateConfig(newConfig);
        await checkForUpdates(); // Refresh releases for new channel
      } else {
        setError('Failed to update channel');
      }
    } catch (err) {
      setError('Failed to update channel');
    }
  };

  const downloadUpdate = async () => {
    const latest = latestVersion();
    if (!latest) return;

    setIsDownloading(true);
    setError(null);

    try {
      // For timestamp-based updates, open the commit page
      // In a real implementation, this could trigger a git pull or download
      window.open(latest.html_url, '_blank');
      
      // Show instruction message
      const config = updateConfig();
      const instructions = config.update_channel === 'dev' 
        ? 'To update: Run `git pull origin dev` in your local repository, then restart the bridge.'
        : 'To update: Run `git pull origin main` in your local repository, then restart the bridge.';
        
      alert(`Update information opened in browser!\n\n${instructions}`);
    } catch (err) {
      setError(`Failed to open update: ${err.message}`);
    } finally {
      setIsDownloading(false);
    }
  };

  const formatVersion = (version) => {
    if (typeof version === 'string') return version;
    if (version?.major !== undefined) {
      return version.minor !== undefined ? `r${version.major}.${version.minor}` : `r${version.major}`;
    }
    return 'Unknown';
  };

  const isUpdateAvailable = () => {
    // Update availability is now determined in checkForUpdates
    // If latestVersion is set, then an update is available
    return latestVersion() !== null;
  };

  const getChannelBadgeColor = (channel) => {
    return channel === 'stable' ? 'bg-green-500' : 'bg-blue-500';
  };

  const formatDate = (dateString) => {
    if (!dateString) return 'Unknown date';
    try {
      return new Date(dateString).toLocaleDateString('en-US', {
        year: 'numeric',
        month: 'short',
        day: 'numeric'
      });
    } catch (e) {
      return 'Unknown date';
    }
  };

  return (
    <Show when={props.show}>
      <div class="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-[200] p-4">
        <div class="bg-gray-900 rounded-2xl shadow-2xl max-w-2xl w-full max-h-[90vh] overflow-hidden border border-gray-700">
          {/* Header */}
          <div class="bg-gradient-to-r from-gray-800 to-gray-900 px-6 py-4 border-b border-gray-700">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-3">
                <div class="w-8 h-8 bg-orange-500 rounded-lg flex items-center justify-center">
                  <svg class="w-5 h-5 text-white" fill="currentColor" viewBox="0 0 20 20">
                    <path fill-rule="evenodd" d="M11.3 1.046A1 1 0 0112 2v5h4a1 1 0 01.82 1.573l-7 10A1 1 0 018 18v-5H4a1 1 0 01-.82-1.573l7-10a1 1 0 011.12-.38z" clip-rule="evenodd" />
                  </svg>
                </div>
                <div>
                  <h2 class="text-xl font-bold text-white">Update Manager</h2>
                  <p class="text-gray-400 text-sm">Manage Renzora Engine updates</p>
                </div>
              </div>
              <button
                onClick={props.onClose}
                class="text-gray-400 hover:text-white transition-colors p-2 hover:bg-gray-800 rounded-lg"
              >
                <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
                  <path fill-rule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clip-rule="evenodd" />
                </svg>
              </button>
            </div>
          </div>

          {/* Content */}
          <div class="p-6 space-y-6 overflow-y-auto max-h-[calc(90vh-140px)]">
            {/* Error Message */}
            <Show when={error()}>
              <div class="bg-red-500/20 border border-red-500 rounded-lg p-4">
                <p class="text-red-200">{error()}</p>
              </div>
            </Show>

            {/* Current Version & Channel */}
            <div class="bg-gray-800 rounded-lg p-4 space-y-4">
              <h3 class="text-lg font-semibold text-white">Current Configuration</h3>
              
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label class="block text-sm font-medium text-gray-300 mb-2">Current Version</label>
                  <div class="bg-gray-700 rounded-lg px-4 py-2">
                    <span class="text-white font-mono">{currentVersion()}</span>
                  </div>
                </div>
                
                <div>
                  <label class="block text-sm font-medium text-gray-300 mb-2">Update Channel</label>
                  <div class="relative">
                    <select
                      value={updateConfig()?.update_channel || 'stable'}
                      onChange={(e) => setChannel(e.target.value)}
                      class="w-full bg-gray-700 border border-gray-600 text-white rounded-lg px-4 py-2.5 pr-10 
                             focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500 
                             transition-all cursor-pointer appearance-none hover:bg-gray-600"
                    >
                      <option value="stable" class="bg-gray-800 text-white">Stable</option>
                      <option value="dev" class="bg-gray-800 text-white">Dev</option>
                    </select>
                    <div class="absolute inset-y-0 right-0 flex items-center pr-3 pointer-events-none">
                      <svg class="w-5 h-5 text-gray-400" fill="currentColor" viewBox="0 0 20 20">
                        <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
                      </svg>
                    </div>
                  </div>
                  <div class="mt-2">
                    <Show when={updateConfig()?.update_channel === 'stable'}>
                      <p class="text-xs text-gray-400 flex items-center gap-1">
                        <span class="w-2 h-2 bg-green-500 rounded-full"></span>
                        Production-ready releases
                      </p>
                    </Show>
                    <Show when={updateConfig()?.update_channel === 'dev'}>
                      <p class="text-xs text-gray-400 flex items-center gap-1">
                        <span class="w-2 h-2 bg-blue-500 rounded-full"></span>
                        Latest commits and features
                      </p>
                    </Show>
                  </div>
                </div>
              </div>

              <div class="flex items-center gap-2 text-sm">
                <span class="text-gray-400">Repository:</span>
                <a 
                  href={`https://github.com/${updateConfig()?.github_owner}/${updateConfig()?.github_repo}`}
                  target="_blank"
                  class="text-blue-400 hover:text-blue-300 transition-colors"
                >
                  {updateConfig()?.github_owner}/{updateConfig()?.github_repo}
                </a>
                <Show when={updateConfig()?.update_channel === 'dev'}>
                  <span class="text-gray-500">•</span>
                  <a 
                    href={`https://github.com/${updateConfig()?.github_owner}/${updateConfig()?.github_repo}/tree/dev`}
                    target="_blank"
                    class="text-blue-400 hover:text-blue-300 transition-colors"
                  >
                    dev branch
                  </a>
                </Show>
              </div>
            </div>

            {/* Update Status */}
            <div class="bg-gray-800 rounded-lg p-4">
              <div class="flex items-center justify-between mb-4">
                <h3 class="text-lg font-semibold text-white">Update Status</h3>
                <Button 
                  onClick={checkForUpdates} 
                  disabled={isChecking()}
                  variant="outline"
                  size="sm"
                >
                  {isChecking() ? <LoadingSpinner size="sm" /> : 'Refresh'}
                </Button>
              </div>

              <Show when={isChecking()}>
                <div class="flex items-center justify-center py-8">
                  <LoadingSpinner />
                  <span class="ml-3 text-gray-300">Checking for updates...</span>
                </div>
              </Show>

              <Show when={!isChecking()}>
                <Show when={latestVersion() && isUpdateAvailable()} fallback={
                  <Show when={latestVersion()} fallback={
                    <div class="bg-gray-500/20 border border-gray-500 rounded-lg p-4">
                      <div class="flex items-center gap-3">
                        <svg class="w-6 h-6 text-gray-400" fill="currentColor" viewBox="0 0 20 20">
                          <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd" />
                        </svg>
                        <div>
                          <p class="text-gray-300 font-medium">No releases available</p>
                          <p class="text-gray-400 text-sm">No releases found for the selected channel</p>
                        </div>
                      </div>
                    </div>
                  }>
                    <div class="bg-green-500/20 border border-green-500 rounded-lg p-4">
                      <div class="flex items-center gap-3">
                        <svg class="w-6 h-6 text-green-400" fill="currentColor" viewBox="0 0 20 20">
                          <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
                        </svg>
                        <div>
                          <p class="text-green-200 font-medium">You're up to date!</p>
                          <p class="text-green-300 text-sm">Current: {currentVersion()} • Latest: {latestVersion()?.tag_name}</p>
                        </div>
                      </div>
                    </div>
                  </Show>
                }>
                  <div class="bg-orange-500/20 border border-orange-500 rounded-lg p-4">
                    <div class="flex items-start justify-between">
                      <div class="flex items-start gap-3">
                        <svg class="w-6 h-6 text-orange-400 mt-0.5" fill="currentColor" viewBox="0 0 20 20">
                          <path fill-rule="evenodd" d="M11.3 1.046A1 1 0 0112 2v5h4a1 1 0 01.82 1.573l-7 10A1 1 0 018 18v-5H4a1 1 0 01-.82-1.573l7-10a1 1 0 011.12-.38z" clip-rule="evenodd" />
                        </svg>
                        <div class="flex-1">
                          <p class="text-orange-200 font-medium mb-1">Update Available!</p>
                          <p class="text-orange-300 text-sm mb-3">
                            {latestVersion()?.name || latestVersion()?.tag_name || 'New version'} is now available
                          </p>
                          <div class="text-xs text-orange-400 mb-3">
                            Published: {formatDate(latestVersion()?.published_at)}
                          </div>
                          
                          <Show when={latestVersion()?.body}>
                            <div class="bg-gray-900/50 rounded p-3 mb-4">
                              <p class="text-gray-300 text-sm whitespace-pre-wrap max-h-24 overflow-hidden">
                                {latestVersion()?.body}
                              </p>
                            </div>
                          </Show>

                          <div class="flex gap-2">
                            <Button 
                              onClick={downloadUpdate} 
                              disabled={isDownloading()}
                              variant="primary"
                              size="sm"
                            >
                              {isDownloading() ? (
                                <>
                                  <LoadingSpinner size="sm" />
                                  <span class="ml-2">Starting Download...</span>
                                </>
                              ) : (
                                <>
                                  <svg class="w-4 h-4 mr-2" fill="currentColor" viewBox="0 0 20 20">
                                    <path fill-rule="evenodd" d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z" clip-rule="evenodd" />
                                  </svg>
                                  Download Update
                                </>
                              )}
                            </Button>
                            
                            <Button 
                              onClick={() => window.open(latestVersion()?.html_url, '_blank')}
                              variant="outline"
                              size="sm"
                            >
                              View Release
                            </Button>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </Show>
              </Show>
            </div>

            {/* Recent Releases */}
            <Show when={releases().length > 0}>
              <div class="bg-gray-800 rounded-lg p-4">
                <h3 class="text-lg font-semibold text-white mb-4">Recent Releases</h3>
                <div class="space-y-2 max-h-48 overflow-y-auto">
                  <For each={releases().slice(0, 5)}>
                    {(release) => (
                      <div class="flex items-center justify-between py-2 px-3 hover:bg-gray-700/50 rounded-lg transition-colors">
                        <div class="flex items-center gap-3">
                          <span class={`px-2 py-1 rounded text-xs font-semibold text-white ${getChannelBadgeColor(release?.prerelease || (release?.tag_name && release.tag_name.includes('.')) ? 'dev' : 'stable')}`}>
                            {release?.tag_name || 'Unknown'}
                          </span>
                          <span class="text-white text-sm">{release?.name || release?.tag_name || 'Unnamed Release'}</span>
                        </div>
                        <div class="flex items-center gap-2 text-xs text-gray-400">
                          <span>{formatDate(release?.published_at)}</span>
                          <Show when={release?.html_url}>
                            <a 
                              href={release.html_url}
                              target="_blank"
                              class="text-blue-400 hover:text-blue-300 transition-colors"
                            >
                              View
                            </a>
                          </Show>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </div>
            </Show>

            {/* Channel Information */}
            <div class="bg-gray-800 rounded-lg p-4">
              <h3 class="text-lg font-semibold text-white mb-3">Channel Information</h3>
              <div class="grid grid-cols-2 gap-4">
                <div class="bg-gray-700/50 rounded-lg p-3 border border-gray-600 hover:border-green-500/50 transition-colors">
                  <div class="flex items-center gap-2 mb-2">
                    <div class="w-3 h-3 bg-green-500 rounded-full animate-pulse"></div>
                    <span class="text-white font-medium">Stable</span>
                  </div>
                  <p class="text-gray-400 text-xs">
                    Official releases only. Thoroughly tested and production-ready.
                  </p>
                </div>
                <div class="bg-gray-700/50 rounded-lg p-3 border border-gray-600 hover:border-blue-500/50 transition-colors">
                  <div class="flex items-center gap-2 mb-2">
                    <div class="w-3 h-3 bg-blue-500 rounded-full animate-pulse"></div>
                    <span class="text-white font-medium">Dev</span>
                  </div>
                  <p class="text-gray-400 text-xs">
                    Latest commits from GitHub. Get new features as they're developed.
                  </p>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Show>
  );
}