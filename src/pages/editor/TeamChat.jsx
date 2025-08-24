import { createSignal, For, onMount } from 'solid-js';
import { 
  Send, Plus, Settings, Search, Paperclip, 
  Photo, File, ArrowUp, ArrowDown, Edit 
} from '@/ui/icons';

function TeamChat() {
  const [messages, setMessages] = createSignal([
    {
      id: 1,
      user: 'Alice',
      avatar: '👩‍💻',
      message: 'Hey team, I just finished the new lighting setup for the main scene. Want to take a look?',
      timestamp: Date.now() - 3600000,
      type: 'text'
    },
    {
      id: 2,
      user: 'Bob',
      avatar: '👨‍🎨',
      message: 'Great work! The new materials look amazing.',
      timestamp: Date.now() - 3300000,
      type: 'text'
    },
    {
      id: 3,
      user: 'Alice',
      avatar: '👩‍💻',
      message: 'main_scene_v3.blend',
      timestamp: Date.now() - 3000000,
      type: 'file',
      fileType: 'blend',
      fileSize: '15.2 MB'
    },
    {
      id: 4,
      user: 'Charlie',
      avatar: '🎯',
      message: 'Should we increase the particle count for the fire effect?',
      timestamp: Date.now() - 2700000,
      type: 'text'
    },
    {
      id: 5,
      user: 'You',
      avatar: '👤',
      message: 'Let me check the performance impact first',
      timestamp: Date.now() - 2400000,
      type: 'text'
    }
  ]);

  const [newMessage, setNewMessage] = createSignal('');
  const [teamMembers, setTeamMembers] = createSignal([
    { id: 1, name: 'Alice', avatar: '👩‍💻', status: 'online', role: 'Lead Artist' },
    { id: 2, name: 'Bob', avatar: '👨‍🎨', status: 'online', role: 'Material Artist' },
    { id: 3, name: 'Charlie', avatar: '🎯', status: 'away', role: 'VFX Artist' },
    { id: 4, name: 'Diana', avatar: '👩‍🔬', status: 'offline', role: 'Technical Artist' }
  ]);

  const [activeChannel, setActiveChannel] = createSignal('general');
  const [channels] = createSignal([
    { id: 'general', name: 'General', unread: 0 },
    { id: 'assets', name: 'Assets', unread: 2 },
    { id: 'feedback', name: 'Feedback', unread: 0 },
    { id: 'random', name: 'Random', unread: 1 }
  ]);

  const sendMessage = () => {
    const message = newMessage().trim();
    if (!message) return;

    const newMsg = {
      id: Date.now(),
      user: 'You',
      avatar: '👤',
      message,
      timestamp: Date.now(),
      type: 'text'
    };

    setMessages(prev => [...prev, newMsg]);
    setNewMessage('');
    
    // Auto-scroll to bottom
    setTimeout(() => {
      const chatArea = document.querySelector('.chat-area');
      if (chatArea) {
        chatArea.scrollTop = chatArea.scrollHeight;
      }
    }, 50);
  };

  const formatTimestamp = (timestamp) => {
    const now = Date.now();
    const diff = now - timestamp;
    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (minutes < 1) return 'now';
    if (minutes < 60) return `${minutes}m`;
    if (hours < 24) return `${hours}h`;
    return `${days}d`;
  };

  const getStatusColor = (status) => {
    switch (status) {
      case 'online': return 'bg-success';
      case 'away': return 'bg-warning';
      case 'offline': return 'bg-base-300';
      default: return 'bg-base-300';
    }
  };

  const getFileIcon = (fileType) => {
    switch (fileType) {
      case 'blend': return '🎨';
      case 'png': case 'jpg': case 'jpeg': return '🖼️';
      case 'fbx': case 'obj': case 'gltf': return '🏗️';
      case 'mp4': case 'mov': return '🎥';
      default: return '📄';
    }
  };

  onMount(() => {
    // Auto-scroll to bottom on mount
    const chatArea = document.querySelector('.chat-area');
    if (chatArea) {
      chatArea.scrollTop = chatArea.scrollHeight;
    }
  });

  return (
    <div class="h-full flex flex-col bg-base-100">
      {/* Chat Header */}
      <div class="flex items-center justify-between p-3 border-b border-base-300">
        <div class="flex items-center space-x-2">
          <div class="w-4 h-4 bg-gradient-to-r from-blue-400 to-purple-500 rounded-full"></div>
          <span class="text-sm font-medium text-base-content">Team Chat</span>
          <span class="text-xs text-base-content/60">#{activeChannel()}</span>
        </div>
        
        <div class="flex items-center space-x-1">
          <button class="btn btn-xs btn-ghost" title="Search">
            <Search class="w-3 h-3" />
          </button>
          <button class="btn btn-xs btn-ghost" title="Settings">
            <Settings class="w-3 h-3" />
          </button>
        </div>
      </div>

      <div class="flex-1 flex min-h-0">
        {/* Channels & Team Members */}
        <div class="w-48 border-r border-base-300 flex flex-col">
          {/* Channels */}
          <div class="border-b border-base-300">
            <div class="p-2">
              <div class="text-xs text-base-content/60 uppercase tracking-wide mb-2">Channels</div>
              <div class="space-y-1">
                <For each={channels()}>
                  {(channel) => (
                    <button
                      class={`w-full text-left p-1 px-2 rounded text-xs hover:bg-base-200 flex items-center justify-between ${
                        activeChannel() === channel.id ? 'bg-primary text-primary-content' : ''
                      }`}
                      onClick={() => setActiveChannel(channel.id)}
                    >
                      <span>#{channel.name}</span>
                      {channel.unread > 0 && (
                        <span class="badge badge-xs badge-error">{channel.unread}</span>
                      )}
                    </button>
                  )}
                </For>
              </div>
            </div>
          </div>

          {/* Team Members */}
          <div class="flex-1 overflow-y-auto">
            <div class="p-2">
              <div class="text-xs text-base-content/60 uppercase tracking-wide mb-2">Team</div>
              <div class="space-y-1">
                <For each={teamMembers()}>
                  {(member) => (
                    <div class="flex items-center space-x-2 p-1 hover:bg-base-200 rounded">
                      <div class="relative">
                        <span class="text-sm">{member.avatar}</span>
                        <div class={`absolute -bottom-0.5 -right-0.5 w-2 h-2 rounded-full border border-base-100 ${getStatusColor(member.status)}`}></div>
                      </div>
                      <div class="flex-1 min-w-0">
                        <div class="text-xs font-medium truncate">{member.name}</div>
                        <div class="text-[10px] text-base-content/40 truncate">{member.role}</div>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </div>
        </div>

        {/* Chat Area */}
        <div class="flex-1 flex flex-col">
          {/* Messages */}
          <div class="flex-1 overflow-y-auto p-3 space-y-3 chat-area">
            <For each={messages()}>
              {(message) => (
                <div class={`flex space-x-2 ${message.user === 'You' ? 'justify-end' : ''}`}>
                  {message.user !== 'You' && (
                    <div class="flex-shrink-0">
                      <span class="text-lg">{message.avatar}</span>
                    </div>
                  )}
                  
                  <div class={`flex flex-col max-w-xs ${message.user === 'You' ? 'items-end' : ''}`}>
                    {message.user !== 'You' && (
                      <div class="flex items-center space-x-2 mb-1">
                        <span class="text-xs font-medium text-base-content">{message.user}</span>
                        <span class="text-[10px] text-base-content/40">
                          {formatTimestamp(message.timestamp)}
                        </span>
                      </div>
                    )}
                    
                    <div class={`p-2 rounded-lg text-xs ${
                      message.user === 'You' 
                        ? 'bg-primary text-primary-content' 
                        : 'bg-base-200 text-base-content'
                    }`}>
                      {message.type === 'text' ? (
                        <p>{message.message}</p>
                      ) : message.type === 'file' ? (
                        <div class="flex items-center space-x-2">
                          <span class="text-sm">{getFileIcon(message.fileType)}</span>
                          <div>
                            <div class="font-medium">{message.message}</div>
                            <div class="text-[10px] opacity-60">{message.fileSize}</div>
                          </div>
                        </div>
                      ) : null}
                    </div>
                    
                    {message.user === 'You' && (
                      <span class="text-[10px] text-base-content/40 mt-1">
                        {formatTimestamp(message.timestamp)}
                      </span>
                    )}
                  </div>
                  
                  {message.user === 'You' && (
                    <div class="flex-shrink-0">
                      <span class="text-lg">{message.avatar}</span>
                    </div>
                  )}
                </div>
              )}
            </For>
          </div>

          {/* Message Input */}
          <div class="border-t border-base-300 p-3">
            <div class="flex items-center space-x-2">
              <button class="btn btn-xs btn-ghost" title="Attach File">
                <Paperclip class="w-3 h-3" />
              </button>
              <button class="btn btn-xs btn-ghost" title="Add Image">
                <Photo class="w-3 h-3" />
              </button>
              
              <div class="flex-1">
                <input
                  type="text"
                  placeholder={`Message #${activeChannel()}`}
                  class="input input-xs input-bordered w-full text-xs"
                  value={newMessage()}
                  onInput={(e) => setNewMessage(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && !e.shiftKey) {
                      e.preventDefault();
                      sendMessage();
                    }
                  }}
                />
              </div>
              
              <button
                class="btn btn-xs btn-primary"
                onClick={sendMessage}
                disabled={!newMessage().trim()}
                title="Send Message"
              >
                <Send class="w-3 h-3" />
              </button>
            </div>
            
            <div class="flex items-center justify-between mt-2 text-[10px] text-base-content/40">
              <span>Press Enter to send, Shift+Enter for new line</span>
              <span>{teamMembers().filter(m => m.status === 'online').length} online</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default TeamChat;