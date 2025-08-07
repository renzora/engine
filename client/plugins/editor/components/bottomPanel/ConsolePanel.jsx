import { useState, useRef, useEffect } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';
import { projectManager } from '@/plugins/projects/projectManager.js';

function ConsolePanel() {
  const [logs, setLogs] = useState([]);
  const [filter, setFilter] = useState('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [command, setCommand] = useState('');
  const [logCategory, setLogCategory] = useState('all'); // 'all', 'client', 'server'
  const [showSuggestions, setShowSuggestions] = useState(false);
  const consoleEndRef = useRef(null);
  
  // Available commands for autocompletion
  const availableCommands = [
    'help',
    'restart',
    'server:restart', 
    'status',
    'server:status',
    'memory',
    'server:memory',
    'projects:list',
    'logs:clear',
    'version'
  ];

  // Add file change notifications to console
  useEffect(() => {
    const handleFileChange = (changeData) => {
      const timestamp = new Date().toLocaleTimeString('en-US', { hour12: false });
      let message = '';
      let type = 'info';

      switch (changeData.type) {
        case 'file_added':
          message = `File added: ${changeData.path}`;
          type = 'info';
          break;
        case 'file_changed':
          message = `File modified: ${changeData.path}`;
          type = 'info';
          break;
        case 'file_deleted':
          message = `File deleted: ${changeData.path}`;
          type = 'warning';
          break;
        case 'directory_added':
          message = `Directory created: ${changeData.path}`;
          type = 'info';
          break;
        case 'directory_deleted':
          message = `Directory deleted: ${changeData.path}`;
          type = 'warning';
          break;
        case 'assets_directory_recreated':
          message = `Assets directory automatically recreated: ${changeData.path}`;
          type = 'info';
          break;
        default:
          message = `File system change: ${changeData.path} (${changeData.type})`;
          type = 'info';
      }

      const newLog = {
        id: Date.now() + Math.random(),
        type,
        timestamp,
        message,
        source: 'FileWatcher',
        category: 'client'
      };

      setLogs(prev => [...prev, newLog]);
    };

    projectManager.addFileChangeListener(handleFileChange);

    return () => {
      projectManager.removeFileChangeListener(handleFileChange);
    };
  }, []);

  // Connect to server logs SSE
  useEffect(() => {
    const eventSource = new EventSource('/api/server/logs');
    
    eventSource.onmessage = (event) => {
      try {
        const serverLog = JSON.parse(event.data);
        
        // Skip connection messages
        if (serverLog.type === 'connected') return;
        
        // Convert server log format to client log format
        const clientLog = {
          id: serverLog.id,
          type: serverLog.level === 'warn' ? 'warning' : serverLog.level,
          timestamp: new Date(serverLog.timestamp).toLocaleTimeString('en-US', { hour12: false }),
          message: serverLog.message,
          source: serverLog.source,
          category: 'server'
        };
        
        setLogs(prev => [...prev, clientLog]);
      } catch (error) {
        console.warn('Failed to parse server log:', error);
      }
    };
    
    eventSource.onerror = (error) => {
      console.warn('Server logs SSE error:', error);
    };
    
    return () => {
      eventSource.close();
    };
  }, []);

  const filteredLogs = logs.filter(log => {
    const matchesFilter = filter === 'all' || log.type === filter;
    const matchesCategory = logCategory === 'all' || log.category === logCategory || (!log.category && logCategory === 'client');
    const matchesSearch = log.message.toLowerCase().includes(searchQuery.toLowerCase()) ||
                         log.source.toLowerCase().includes(searchQuery.toLowerCase());
    return matchesFilter && matchesCategory && matchesSearch;
  });

  const executeCommand = async () => {
    if (!command.trim()) return;
    
    // Add command to log
    const commandLog = {
      id: Date.now() + Math.random(),
      type: 'command',
      timestamp: new Date().toLocaleTimeString('en-US', { hour12: false }),
      message: `> ${command}`,
      source: 'Console',
      category: 'client'
    };
    
    setLogs(prev => [...prev, commandLog]);
    
    // Parse command and arguments
    const parts = command.trim().split(' ');
    const cmd = parts[0];
    const args = parts.slice(1);
    
    try {
      // Send command to server
      const response = await fetch('/api/console/command', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({ command: cmd, args })
      });
      
      const result = await response.json();
      
      // Add result to log
      const resultLog = {
        id: Date.now() + Math.random() + 1,
        type: result.success ? 'info' : 'error',
        timestamp: new Date().toLocaleTimeString('en-US', { hour12: false }),
        message: result.message,
        source: 'Console',
        category: 'client'
      };
      
      setLogs(prev => [...prev, resultLog]);
      
    } catch (error) {
      // Add error to log
      const errorLog = {
        id: Date.now() + Math.random() + 2,
        type: 'error',
        timestamp: new Date().toLocaleTimeString('en-US', { hour12: false }),
        message: `Command failed: ${error.message}`,
        source: 'Console',
        category: 'client'
      };
      
      setLogs(prev => [...prev, errorLog]);
    }
    
    setCommand('');
  };

  const clearConsole = () => {
    setLogs([]);
  };

  // Get command suggestions based on current input
  const getCommandSuggestions = () => {
    if (!command.trim()) return [];
    return availableCommands.filter(cmd => 
      cmd.toLowerCase().startsWith(command.toLowerCase())
    ).slice(0, 5); // Show max 5 suggestions
  };

  const handleCommandChange = (e) => {
    const value = e.target.value;
    setCommand(value);
    setShowSuggestions(value.trim().length > 0);
  };

  const handleKeyPress = (e) => {
    if (e.key === 'Enter') {
      executeCommand();
      setShowSuggestions(false);
    } else if (e.key === 'Escape') {
      setShowSuggestions(false);
    } else if (e.key === 'Tab') {
      e.preventDefault();
      const suggestions = getCommandSuggestions();
      if (suggestions.length === 1) {
        setCommand(suggestions[0] + ' ');
        setShowSuggestions(false);
      }
    }
  };

  const selectSuggestion = (suggestion) => {
    setCommand(suggestion + ' ');
    setShowSuggestions(false);
  };

  useEffect(() => {
    consoleEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  const getLogIcon = (type) => {
    switch (type) {
      case 'error': return <Icons.XMark className="w-3 h-3 text-red-400" />;
      case 'warning': return <div className="w-3 h-3 bg-yellow-400 rounded-full" />;
      case 'command': return <Icons.CommandLine className="w-3 h-3 text-blue-400" />;
      default: return <Icons.Circle className="w-3 h-3 text-green-400" />;
    }
  };

  const getLogColor = (type) => {
    switch (type) {
      case 'error': return 'text-red-300';
      case 'warning': return 'text-yellow-300';
      case 'command': return 'text-blue-300';
      default: return 'text-gray-300';
    }
  };

  return (
    <div className="h-full flex flex-col bg-slate-800">
      {/* Console Header */}
      <div className="p-3 border-b border-slate-700 bg-slate-900/50">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h3 className="text-sm font-medium text-white">Console</h3>
            
            {/* Category Toggle */}
            <div className="flex items-center gap-1">
              {['all', 'client', 'server'].map((category) => (
                <button
                  key={category}
                  onClick={() => setLogCategory(category)}
                  className={`px-2 py-1 text-xs rounded transition-colors ${
                    logCategory === category
                      ? 'bg-purple-600 text-white'
                      : 'text-gray-400 hover:text-white hover:bg-slate-700'
                  }`}
                >
                  {category === 'all' ? 'All' : category.charAt(0).toUpperCase() + category.slice(1)}
                  {category !== 'all' && (
                    <span className="ml-1 text-xs">
                      ({logs.filter(log => log.category === category || (!log.category && category === 'client')).length})
                    </span>
                  )}
                </button>
              ))}
            </div>
            
            {/* Log Level Filter */}
            <div className="flex items-center gap-1">
              {['all', 'info', 'warning', 'error'].map((filterType) => (
                <button
                  key={filterType}
                  onClick={() => setFilter(filterType)}
                  className={`px-2 py-1 text-xs rounded transition-colors ${
                    filter === filterType
                      ? 'bg-blue-600 text-white'
                      : 'text-gray-400 hover:text-white hover:bg-slate-700'
                  }`}
                >
                  {filterType === 'all' ? 'All' : filterType.charAt(0).toUpperCase() + filterType.slice(1)}
                  {filterType !== 'all' && (
                    <span className="ml-1 text-xs">
                      ({logs.filter(log => log.type === filterType).length})
                    </span>
                  )}
                </button>
              ))}
            </div>
          </div>
          
          <div className="flex items-center gap-2">
            <div className="relative">
              <Icons.MagnifyingGlass className="w-3 h-3 absolute left-2 top-1.5 text-gray-400" />
              <input
                type="text"
                placeholder="Search logs..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="pl-6 pr-2 py-1 bg-slate-800 border border-slate-600 rounded text-xs text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 w-32"
              />
            </div>
            <button
              onClick={clearConsole}
              className="text-xs text-gray-400 hover:text-white transition-colors"
            >
              Clear
            </button>
          </div>
        </div>
      </div>
      
      {/* Console Output */}
      <div className="flex-1 overflow-y-auto scrollbar-thin font-mono">
        <div className="p-2 space-y-1">
          {filteredLogs.map((log) => (
            <div
              key={log.id}
              className={`flex items-start gap-2 px-2 py-1 text-xs hover:bg-slate-700/30 rounded transition-colors ${getLogColor(log.type)}`}
            >
              <span className="text-gray-500 shrink-0">{log.timestamp}</span>
              <div className="shrink-0 mt-0.5">{getLogIcon(log.type)}</div>
              <div className="shrink-0 mt-0.5">
                {log.category === 'server' ? (
                  <div className="w-2 h-2 bg-green-500 rounded-full" title="Server Log" />
                ) : (
                  <div className="w-2 h-2 bg-blue-500 rounded-full" title="Client Log" />
                )}
              </div>
              <span className="text-gray-400 shrink-0 min-w-[80px]">[{log.source}]</span>
              <span className="break-all">{log.message}</span>
            </div>
          ))}
          <div ref={consoleEndRef} />
        </div>
      </div>
      
      {/* Command Input */}
      <div className="border-t border-slate-700 p-2 bg-slate-900/50">
        <div className="relative">
          <div className="flex items-center gap-2">
            <span className="text-gray-400 text-sm">&gt;</span>
            <input
              type="text"
              value={command}
              onChange={handleCommandChange}
              onKeyDown={handleKeyPress}
              placeholder="Enter command..."
              className="flex-1 bg-transparent text-white text-sm font-mono focus:outline-none placeholder-gray-500"
              autoComplete="off"
            />
            <button
              onClick={executeCommand}
              disabled={!command.trim()}
              className="text-xs text-blue-400 hover:text-blue-300 disabled:text-gray-600 disabled:cursor-not-allowed transition-colors"
            >
              Execute
            </button>
          </div>
          
          {/* Command suggestions dropdown */}
          {showSuggestions && getCommandSuggestions().length > 0 && (
            <div className="absolute bottom-full left-0 right-0 mb-1 bg-slate-800 border border-slate-600 rounded shadow-lg z-50">
              {getCommandSuggestions().map((suggestion, index) => (
                <button
                  key={suggestion}
                  onClick={() => selectSuggestion(suggestion)}
                  className="w-full text-left px-3 py-1 text-xs text-gray-300 hover:bg-slate-700 hover:text-white first:rounded-t last:rounded-b transition-colors"
                >
                  {suggestion}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default ConsolePanel;