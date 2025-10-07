# Bridge API Migration Guide

## WebSocket vs HTTP Transport

The bridge API now automatically detects and switches between:

- **WebSocket Server** (port 3002) - High-performance, real-time communication  
- **HTTP Bridge** (port 3001) - Legacy HTTP-based communication with SSE

### Configuration

Configure transport preference via localStorage:

```javascript
// Force WebSocket (default: true)
localStorage.setItem('renzora_use_websocket', 'true');

// Force HTTP bridge  
localStorage.setItem('renzora_use_websocket', 'false');

// Or programmatically via bridge API
import { setWebSocketPreference } from '@/api/bridge';
setWebSocketPreference(true);  // Enable WebSocket
setWebSocketPreference(false); // Force HTTP
```

### Features Comparison

| Feature | WebSocket | HTTP |
|---------|-----------|------|
| File operations | ✅ | ✅ |
| Project management | ✅ | ✅ |
| Real-time file watching | ✅ | ✅ (SSE) |
| Bidirectional communication | ✅ | ❌ |
| Lower latency | ✅ | ❌ |
| Connection state awareness | ✅ | ❌ |
| Automatic reconnection | ✅ | ✅ (SSE only) |
| Performance | 🚀 High | 📊 Medium |

### API Usage

The bridge API automatically handles transport switching:

```javascript
import { 
  readFile, 
  writeFile, 
  listProjects,
  onFileChange,
  getCurrentTransport 
} from '@/api/bridge';

// All functions work the same regardless of transport
const projects = await listProjects();
const content = await readFile('path/to/file.js');

// Check which transport is being used
console.log('Using:', getCurrentTransport()); // 'websocket' or 'http'
```

### Server Status

Check server availability:

```javascript
import { getServerStatus } from '@/api/bridge';

console.log('Server:', getServerStatus()); 
// 'websocket' | 'http' | 'none'
```