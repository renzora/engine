import { useState } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';

const Forum = ({ 
  threads = [],
  onThreadClick,
  onNewThread,
  onReply,
  className = '',
  ...props 
}) => {
  const [activeThread, setActiveThread] = useState(null);
  const [newThreadTitle, setNewThreadTitle] = useState('');
  const [newThreadContent, setNewThreadContent] = useState('');
  const [replyContent, setReplyContent] = useState('');
  const [showNewThreadForm, setShowNewThreadForm] = useState(false);

  const handleThreadClick = (thread) => {
    setActiveThread(thread);
    onThreadClick?.(thread);
  };

  const handleNewThread = () => {
    if (newThreadTitle.trim() && newThreadContent.trim()) {
      const newThread = {
        id: Date.now(),
        title: newThreadTitle,
        content: newThreadContent,
        author: 'Current User',
        timestamp: new Date().toISOString(),
        replies: []
      };
      
      onNewThread?.(newThread);
      setNewThreadTitle('');
      setNewThreadContent('');
      setShowNewThreadForm(false);
    }
  };

  const handleReply = () => {
    if (replyContent.trim() && activeThread) {
      const reply = {
        id: Date.now(),
        content: replyContent,
        author: 'Current User',
        timestamp: new Date().toISOString()
      };
      
      onReply?.(activeThread.id, reply);
      setReplyContent('');
    }
  };

  const formatTimestamp = (timestamp) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now - date;
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    return `${diffDays}d ago`;
  };

  if (activeThread) {
    return (
      <div className={`h-full flex flex-col bg-slate-900/50 ${className}`} {...props}>
        {/* Thread Header */}
        <div className="p-4 border-b border-slate-700">
          <div className="flex items-center gap-2 mb-2">
            <button
              onClick={() => setActiveThread(null)}
              className="p-1 hover:bg-slate-700 rounded transition-colors"
            >
              <Icons.ChevronLeft className="w-4 h-4 text-gray-400" />
            </button>
            <h2 className="text-sm font-medium text-white">{activeThread.title}</h2>
          </div>
          <div className="flex items-center gap-2 text-xs text-gray-400">
            <span>by {activeThread.author}</span>
            <span>•</span>
            <span>{formatTimestamp(activeThread.timestamp)}</span>
            <span>•</span>
            <span>{activeThread.replies?.length || 0} replies</span>
          </div>
        </div>

        {/* Thread Content */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          {/* Original Post */}
          <div className="bg-slate-800/40 rounded-lg p-3 border border-slate-700/50">
            <div className="flex items-center gap-2 mb-2">
              <div className="w-6 h-6 bg-blue-600 rounded-full flex items-center justify-center">
                <span className="text-xs text-white font-medium">
                  {activeThread.author.charAt(0).toUpperCase()}
                </span>
              </div>
              <span className="text-xs font-medium text-gray-300">{activeThread.author}</span>
              <span className="text-xs text-gray-500">•</span>
              <span className="text-xs text-gray-500">{formatTimestamp(activeThread.timestamp)}</span>
            </div>
            <p className="text-xs text-gray-200">{activeThread.content}</p>
          </div>

          {/* Replies */}
          {activeThread.replies?.map((reply) => (
            <div key={reply.id} className="bg-slate-800/20 rounded-lg p-3 border border-slate-700/30 ml-4">
              <div className="flex items-center gap-2 mb-2">
                <div className="w-5 h-5 bg-slate-600 rounded-full flex items-center justify-center">
                  <span className="text-xs text-white font-medium">
                    {reply.author.charAt(0).toUpperCase()}
                  </span>
                </div>
                <span className="text-xs font-medium text-gray-300">{reply.author}</span>
                <span className="text-xs text-gray-500">•</span>
                <span className="text-xs text-gray-500">{formatTimestamp(reply.timestamp)}</span>
              </div>
              <p className="text-xs text-gray-200">{reply.content}</p>
            </div>
          ))}
        </div>

        {/* Reply Form */}
        <div className="p-4 border-t border-slate-700">
          <div className="space-y-2">
            <textarea
              value={replyContent}
              onChange={(e) => setReplyContent(e.target.value)}
              placeholder="Write a reply..."
              className="w-full bg-slate-800/80 border border-slate-600 text-white text-xs p-2.5 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500 transition-all resize-none"
              rows={3}
            />
            <div className="flex justify-end gap-2">
              <button
                onClick={() => setReplyContent('')}
                className="px-3 py-1.5 text-xs text-gray-400 hover:text-gray-200 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleReply}
                disabled={!replyContent.trim()}
                className="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed text-white text-xs rounded-lg transition-colors"
              >
                Reply
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={`h-full flex flex-col bg-slate-900/50 ${className}`} {...props}>
      {/* Forum Header */}
      <div className="p-4 border-b border-slate-700">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-medium text-white">Forum</h2>
          <button
            onClick={() => setShowNewThreadForm(true)}
            className="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white text-xs rounded-lg transition-colors"
          >
            New Thread
          </button>
        </div>
        <div className="text-xs text-gray-400 mt-1">
          {threads.length} threads
        </div>
      </div>

      {/* New Thread Form */}
      {showNewThreadForm && (
        <div className="p-4 border-b border-slate-700 bg-slate-800/20">
          <div className="space-y-3">
            <input
              type="text"
              value={newThreadTitle}
              onChange={(e) => setNewThreadTitle(e.target.value)}
              placeholder="Thread title..."
              className="w-full bg-slate-800/80 border border-slate-600 text-white text-xs p-2.5 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500 transition-all"
            />
            <textarea
              value={newThreadContent}
              onChange={(e) => setNewThreadContent(e.target.value)}
              placeholder="What's on your mind?"
              className="w-full bg-slate-800/80 border border-slate-600 text-white text-xs p-2.5 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500 transition-all resize-none"
              rows={4}
            />
            <div className="flex justify-end gap-2">
              <button
                onClick={() => {
                  setShowNewThreadForm(false);
                  setNewThreadTitle('');
                  setNewThreadContent('');
                }}
                className="px-3 py-1.5 text-xs text-gray-400 hover:text-gray-200 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleNewThread}
                disabled={!newThreadTitle.trim() || !newThreadContent.trim()}
                className="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed text-white text-xs rounded-lg transition-colors"
              >
                Create Thread
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Thread List */}
      <div className="flex-1 overflow-y-auto">
        {threads.length === 0 ? (
          <div className="p-8 text-center">
            <Icons.Chat className="w-12 h-12 mx-auto text-gray-500 mb-3" />
            <p className="text-sm text-gray-400 mb-2">No threads yet</p>
            <p className="text-xs text-gray-500">Start a conversation by creating a new thread</p>
          </div>
        ) : (
          <div className="p-2 space-y-1">
            {threads.map((thread) => (
              <button
                key={thread.id}
                onClick={() => handleThreadClick(thread)}
                className="w-full text-left p-3 hover:bg-slate-800/40 rounded-lg transition-colors border border-transparent hover:border-slate-700/50"
              >
                <div className="flex items-start gap-3">
                  <div className="w-8 h-8 bg-blue-600 rounded-full flex items-center justify-center flex-shrink-0">
                    <span className="text-xs text-white font-medium">
                      {thread.author.charAt(0).toUpperCase()}
                    </span>
                  </div>
                  <div className="flex-1 min-w-0">
                    <h3 className="text-sm font-medium text-white truncate">{thread.title}</h3>
                    <p className="text-xs text-gray-400 line-clamp-2 mt-1">{thread.content}</p>
                    <div className="flex items-center gap-2 mt-2 text-xs text-gray-500">
                      <span>{thread.author}</span>
                      <span>•</span>
                      <span>{formatTimestamp(thread.timestamp)}</span>
                      <span>•</span>
                      <span>{thread.replies?.length || 0} replies</span>
                    </div>
                  </div>
                </div>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

export default Forum;