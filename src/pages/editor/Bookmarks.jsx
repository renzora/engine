import { createSignal, For } from 'solid-js';
import { 
  Plus, Settings, Trash, Edit, Copy, Search, 
  ArrowUp, ArrowDown, Folder, FolderOpen, Bookmark 
} from '@/ui/icons';

function Bookmarks() {
  const [bookmarks, setBookmarks] = createSignal([
    {
      id: 1,
      name: 'Main Camera View',
      type: 'camera',
      position: { x: 0, y: 5, z: 10 },
      rotation: { x: -15, y: 0, z: 0 },
      timestamp: Date.now() - 3600000,
      selected: false,
      folder: 'cameras'
    },
    {
      id: 2,
      name: 'Character Close-up',
      type: 'camera',
      position: { x: 2, y: 1.8, z: 3 },
      rotation: { x: 0, y: 45, z: 0 },
      timestamp: Date.now() - 1800000,
      selected: false,
      folder: 'cameras'
    },
    {
      id: 3,
      name: 'Lighting Setup 1',
      type: 'scene',
      description: 'Golden hour lighting configuration',
      timestamp: Date.now() - 7200000,
      selected: true,
      folder: 'lighting'
    },
    {
      id: 4,
      name: 'Material Test Scene',
      type: 'scene',
      description: 'Scene with various PBR materials',
      timestamp: Date.now() - 900000,
      selected: false,
      folder: 'materials'
    }
  ]);

  const [folders, setFolders] = createSignal([
    { id: 'cameras', name: 'Camera Views', expanded: true, color: 'text-blue-500' },
    { id: 'lighting', name: 'Lighting Setups', expanded: true, color: 'text-yellow-500' },
    { id: 'materials', name: 'Material Tests', expanded: false, color: 'text-purple-500' },
    { id: 'scenes', name: 'Scene States', expanded: false, color: 'text-green-500' }
  ]);

  const [searchTerm, setSearchTerm] = createSignal('');
  const [selectedBookmark, setSelectedBookmark] = createSignal(bookmarks().find(b => b.selected));

  const filteredBookmarks = () => {
    return bookmarks().filter(bookmark => {
      const matchesSearch = !searchTerm() || 
        bookmark.name.toLowerCase().includes(searchTerm().toLowerCase());
      return matchesSearch;
    });
  };

  const getBookmarksByFolder = (folderId) => {
    return filteredBookmarks().filter(bookmark => bookmark.folder === folderId);
  };

  const selectBookmark = (bookmark) => {
    setBookmarks(prev => prev.map(b => ({
      ...b,
      selected: b.id === bookmark.id
    })));
    setSelectedBookmark(bookmark);
  };

  const toggleFolder = (folderId) => {
    setFolders(prev => prev.map(f => 
      f.id === folderId ? { ...f, expanded: !f.expanded } : f
    ));
  };

  const deleteBookmark = (bookmarkId) => {
    setBookmarks(prev => prev.filter(b => b.id !== bookmarkId));
    if (selectedBookmark()?.id === bookmarkId) {
      setSelectedBookmark(null);
    }
  };

  const duplicateBookmark = (bookmark) => {
    const newBookmark = {
      ...bookmark,
      id: Date.now(),
      name: `${bookmark.name} Copy`,
      timestamp: Date.now(),
      selected: false
    };
    setBookmarks(prev => [...prev, newBookmark]);
  };

  const applyBookmark = (bookmark) => {
    if (bookmark.type === 'camera') {
      console.log('Applying camera bookmark:', bookmark.name);
      // Apply camera position and rotation
    } else if (bookmark.type === 'scene') {
      console.log('Applying scene bookmark:', bookmark.name);
      // Apply scene state
    }
  };

  const getBookmarkIcon = (type) => {
    switch (type) {
      case 'camera': return '📷';
      case 'scene': return '🎬';
      case 'lighting': return '💡';
      case 'material': return '🎨';
      default: return '📌';
    }
  };

  const formatTimestamp = (timestamp) => {
    const now = Date.now();
    const diff = now - timestamp;
    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (minutes < 1) return 'Just now';
    if (minutes < 60) return `${minutes}m ago`;
    if (hours < 24) return `${hours}h ago`;
    return `${days}d ago`;
  };

  return (
    <div class="h-full flex flex-col bg-base-100">
      {/* Bookmarks Header */}
      <div class="flex items-center justify-between p-3 border-b border-base-300">
        <div class="flex items-center space-x-2">
          <Bookmark class="w-4 h-4 text-yellow-500" />
          <span class="text-sm font-medium text-base-content">Bookmarks</span>
        </div>
        
        <div class="flex items-center space-x-1">
          <button class="btn btn-xs btn-primary" title="Add Bookmark">
            <Plus class="w-3 h-3" />
          </button>
          <button class="btn btn-xs btn-ghost" title="Settings">
            <Settings class="w-3 h-3" />
          </button>
        </div>
      </div>

      {/* Search */}
      <div class="p-3 border-b border-base-300">
        <div class="flex items-center space-x-2">
          <Search class="w-3 h-3 text-base-content/40" />
          <input
            type="text"
            placeholder="Search bookmarks..."
            class="input input-xs input-ghost flex-1 text-xs"
            value={searchTerm()}
            onInput={(e) => setSearchTerm(e.target.value)}
          />
        </div>
      </div>

      <div class="flex-1 flex">
        {/* Bookmarks List */}
        <div class="flex-1 overflow-y-auto">
          <For each={folders()}>
            {(folder) => (
              <div>
                {/* Folder Header */}
                <div
                  class="flex items-center justify-between p-2 hover:bg-base-200 cursor-pointer border-b border-base-300/50"
                  onClick={() => toggleFolder(folder.id)}
                >
                  <div class="flex items-center space-x-2">
                    {folder.expanded ? <FolderOpen class="w-3 h-3" /> : <Folder class="w-3 h-3" />}
                    <span class={`text-xs font-medium ${folder.color}`}>{folder.name}</span>
                    <span class="text-xs text-base-content/40">
                      ({getBookmarksByFolder(folder.id).length})
                    </span>
                  </div>
                  
                  <div class="flex items-center space-x-1">
                    <button
                      class="btn btn-xs btn-ghost p-0 w-4 h-4"
                      onClick={(e) => {
                        e.stopPropagation();
                        // Add bookmark to this folder
                      }}
                      title="Add to folder"
                    >
                      <Plus class="w-2 h-2" />
                    </button>
                  </div>
                </div>

                {/* Folder Content */}
                {folder.expanded && (
                  <div class="bg-base-50">
                    <For each={getBookmarksByFolder(folder.id)}>
                      {(bookmark) => (
                        <div
                          class={`flex items-center justify-between p-3 pl-6 hover:bg-base-200 cursor-pointer border-l-2 ${
                            bookmark.selected ? 'bg-base-200 border-primary' : 'border-transparent'
                          }`}
                          onClick={() => selectBookmark(bookmark)}
                          onDblClick={() => applyBookmark(bookmark)}
                        >
                          <div class="flex-1 min-w-0">
                            <div class="flex items-center space-x-2">
                              <span class="text-sm">{getBookmarkIcon(bookmark.type)}</span>
                              <span class="text-xs font-medium truncate">{bookmark.name}</span>
                            </div>
                            
                            {bookmark.description && (
                              <p class="text-[10px] text-base-content/60 mt-1 truncate">
                                {bookmark.description}
                              </p>
                            )}
                            
                            <div class="text-[10px] text-base-content/40 mt-1">
                              {formatTimestamp(bookmark.timestamp)}
                            </div>
                          </div>
                          
                          <div class="flex items-center space-x-1 opacity-0 group-hover:opacity-100">
                            <button
                              class="btn btn-xs btn-ghost p-0 w-4 h-4"
                              onClick={(e) => {
                                e.stopPropagation();
                                applyBookmark(bookmark);
                              }}
                              title="Apply bookmark"
                            >
                              <ArrowUp class="w-2 h-2" />
                            </button>
                            <button
                              class="btn btn-xs btn-ghost p-0 w-4 h-4"
                              onClick={(e) => {
                                e.stopPropagation();
                                duplicateBookmark(bookmark);
                              }}
                              title="Duplicate"
                            >
                              <Copy class="w-2 h-2" />
                            </button>
                            <button
                              class="btn btn-xs btn-ghost p-0 w-4 h-4"
                              onClick={(e) => {
                                e.stopPropagation();
                                // Edit bookmark
                              }}
                              title="Edit"
                            >
                              <Edit class="w-2 h-2" />
                            </button>
                            <button
                              class="btn btn-xs btn-ghost p-0 w-4 h-4 text-error"
                              onClick={(e) => {
                                e.stopPropagation();
                                deleteBookmark(bookmark.id);
                              }}
                              title="Delete"
                            >
                              <Trash class="w-2 h-2" />
                            </button>
                          </div>
                        </div>
                      )}
                    </For>

                    {getBookmarksByFolder(folder.id).length === 0 && (
                      <div class="p-4 pl-6 text-center text-base-content/40">
                        <p class="text-xs">No bookmarks in this folder</p>
                      </div>
                    )}
                  </div>
                )}
              </div>
            )}
          </For>
        </div>

        {/* Properties Panel */}
        {selectedBookmark() && (
          <div class="w-48 border-l border-base-300 flex flex-col">
            <div class="p-3 border-b border-base-300">
              <h3 class="text-sm font-medium">Properties</h3>
            </div>
            
            <div class="flex-1 overflow-y-auto p-3 space-y-4">
              <div class="space-y-2">
                <h4 class="text-xs font-medium text-base-content/80">Bookmark</h4>
                <div class="space-y-2">
                  <div>
                    <label class="text-xs text-base-content/60">Name</label>
                    <input
                      type="text"
                      class="input input-xs input-bordered w-full text-xs mt-1"
                      value={selectedBookmark().name}
                    />
                  </div>
                  <div>
                    <label class="text-xs text-base-content/60">Description</label>
                    <textarea
                      class="textarea textarea-xs textarea-bordered w-full text-xs mt-1 h-16"
                      placeholder="Add description..."
                      value={selectedBookmark().description || ''}
                    />
                  </div>
                  <div class="flex items-center justify-between">
                    <label class="text-xs text-base-content/60">Type</label>
                    <span class="text-xs text-base-content capitalize">{selectedBookmark().type}</span>
                  </div>
                  <div class="flex items-center justify-between">
                    <label class="text-xs text-base-content/60">Created</label>
                    <span class="text-xs text-base-content/60">
                      {formatTimestamp(selectedBookmark().timestamp)}
                    </span>
                  </div>
                </div>
              </div>

              {selectedBookmark().type === 'camera' && selectedBookmark().position && (
                <div class="space-y-2">
                  <h4 class="text-xs font-medium text-base-content/80">Camera</h4>
                  <div class="space-y-2">
                    <div>
                      <label class="text-xs text-base-content/60">Position</label>
                      <div class="grid grid-cols-3 gap-1 mt-1">
                        <input
                          type="number"
                          step="0.1"
                          class="input input-xs input-bordered text-xs"
                          value={selectedBookmark().position.x}
                          placeholder="X"
                        />
                        <input
                          type="number"
                          step="0.1"
                          class="input input-xs input-bordered text-xs"
                          value={selectedBookmark().position.y}
                          placeholder="Y"
                        />
                        <input
                          type="number"
                          step="0.1"
                          class="input input-xs input-bordered text-xs"
                          value={selectedBookmark().position.z}
                          placeholder="Z"
                        />
                      </div>
                    </div>
                    <div>
                      <label class="text-xs text-base-content/60">Rotation</label>
                      <div class="grid grid-cols-3 gap-1 mt-1">
                        <input
                          type="number"
                          step="1"
                          class="input input-xs input-bordered text-xs"
                          value={selectedBookmark().rotation.x}
                          placeholder="X"
                        />
                        <input
                          type="number"
                          step="1"
                          class="input input-xs input-bordered text-xs"
                          value={selectedBookmark().rotation.y}
                          placeholder="Y"
                        />
                        <input
                          type="number"
                          step="1"
                          class="input input-xs input-bordered text-xs"
                          value={selectedBookmark().rotation.z}
                          placeholder="Z"
                        />
                      </div>
                    </div>
                  </div>
                </div>
              )}

              <div class="space-y-2">
                <h4 class="text-xs font-medium text-base-content/80">Actions</h4>
                <div class="space-y-1">
                  <button
                    class="btn btn-xs btn-primary w-full"
                    onClick={() => applyBookmark(selectedBookmark())}
                  >
                    Apply Bookmark
                  </button>
                  <button
                    class="btn btn-xs btn-ghost w-full"
                    onClick={() => duplicateBookmark(selectedBookmark())}
                  >
                    Duplicate
                  </button>
                  <button
                    class="btn btn-xs btn-error w-full"
                    onClick={() => deleteBookmark(selectedBookmark().id)}
                  >
                    Delete
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Empty State */}
      {filteredBookmarks().length === 0 && (
        <div class="flex-1 flex items-center justify-center">
          <div class="text-center text-base-content/40">
            <Bookmark class="w-8 h-8 mx-auto mb-2 text-yellow-500" />
            <p class="text-xs mb-2">No bookmarks found</p>
            <p class="text-xs">Save camera views and scene states</p>
          </div>
        </div>
      )}
    </div>
  );
}

export default Bookmarks;