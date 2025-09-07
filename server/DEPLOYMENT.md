# Renzora Server Deployment Guide

## 🚀 Quick Setup

### Option 1: Run from Source
```bash
# Clone/download to any location
cd /path/to/server
cargo run
```

### Option 2: Portable Binary
```bash
# Build optimized binary
cargo build --release

# Copy binary anywhere
cp target/release/renzora-server.exe /any/directory/
cd /any/directory/
./renzora-server.exe
```

## 📂 Directory Structure

The server will automatically detect your Renzora engine by looking for:
- `package.json`
- `src/` directory
- `bridge/` directory (optional)
- `projects/` directory (optional)

### Flexible Deployment Options

1. **Same Machine**: Server finds engine automatically
2. **External Drive**: Use environment variables or config file
3. **Network Drive**: Configure paths manually
4. **Docker/Cloud**: Use config file with explicit paths

## ⚙️ Configuration Methods

### 1. Environment Variables (Highest Priority)
```bash
export RENZORA_BASE_PATH="/path/to/engine"
export RENZORA_PROJECTS_PATH="/path/to/projects"
export RENZORA_PORT=3002
export RENZORA_HOST="0.0.0.0"
export RUST_LOG="info"
```

### 2. Configuration File (Medium Priority)
Create `renzora.toml` in any of these locations:
- Same directory as executable
- `./renzora.toml`
- `./config/renzora.toml`
- `../renzora.toml`

```toml
[paths]
base_path = "C:\\Users\\YourName\\engine"
projects_path = "D:\\GameProjects"

[server]
port = 3002
host = "127.0.0.1"
```

### 3. Auto-Detection (Lowest Priority)
Server searches for engine directories automatically.

## 🌐 Frontend Integration

The frontend will automatically connect to available servers:

1. **WebSocket Server** (port 3002) - Preferred for performance
2. **HTTP Bridge** (port 3001) - Fallback compatibility

### Frontend Configuration

Set in `.env.local`:
```bash
# Force WebSocket (default)
VITE_USE_WEBSOCKET=true

# Force HTTP fallback
VITE_USE_WEBSOCKET=false
```

## 📡 Runtime Configuration

Use the frontend to configure server paths dynamically:

### Via UI
1. Open server configuration dialog
2. Click "Scan System" to auto-detect engines
3. Or manually set paths

### Via WebSocket API
```javascript
import { getWebSocketClient } from '@/api/websocket/WebSocketClient.jsx';

const client = getWebSocketClient();

// Scan for engine installations
const scan = await client.scanForEngineRoots();
console.log('Found engines:', scan.data.found_paths);

// Set engine location
await client.setBasePath('C:\\Users\\YourName\\engine');

// Set projects directory
await client.setProjectsPath('D:\\GameProjects');
```

## 🔧 Common Deployment Scenarios

### Scenario 1: Development Machine
**Setup**: Engine and server on same machine
**Config**: No config needed (auto-detection)
```bash
cd engine/server
cargo run
```

### Scenario 2: External SSD
**Setup**: Server on SSD, engine on main drive
**Config**: Set base path via environment or config
```bash
# On SSD
export RENZORA_BASE_PATH="C:\\Users\\YourName\\engine"
./renzora-server.exe
```

### Scenario 3: Network Setup
**Setup**: Server on one machine, engine on another
**Config**: Use network paths or shared drives
```toml
[paths]
base_path = "\\\\server\\engine"
projects_path = "\\\\storage\\projects"

[server]
host = "0.0.0.0"  # Allow external connections
port = 3002
```

### Scenario 4: Docker/Container
**Setup**: Containerized deployment
**Config**: Mount volumes and use config file
```dockerfile
FROM rust:1.70 as builder
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder target/release/renzora-server /usr/local/bin/
COPY renzora.toml /etc/renzora/
EXPOSE 3002
CMD ["renzora-server"]
```

## 🔍 Troubleshooting

### Server Not Finding Engine
1. Check current directory: `pwd`
2. Verify engine structure (package.json, src/)
3. Set explicit path:
   ```bash
   export RENZORA_BASE_PATH="/correct/path"
   ```

### Connection Issues
1. Check port availability: `netstat -an | grep 3002`
2. Verify firewall settings
3. Test health endpoint: `curl http://localhost:3002/health`

### Path Problems
1. Use forward slashes on Unix: `/home/user/engine`
2. Escape backslashes on Windows: `"C:\\\\Users\\\\Name\\\\engine"`
3. Use raw strings in TOML: `base_path = 'C:\Users\Name\engine'`

## 📊 Performance Tuning

### High-Performance Settings
```toml
[server]
workers = 16  # Match CPU cores
host = "0.0.0.0"

[performance]
file_change_buffer = 5000
message_timeout = 60

[features]
file_watching = true  # Real-time updates
system_stats = true   # Performance monitoring
```

### Resource-Constrained Settings
```toml
[server]
workers = 2

[performance]
file_change_buffer = 100
message_timeout = 10

[features]
system_stats = false
```

## 🔒 Security Considerations

### Local Development
- Default: `host = "127.0.0.1"` (localhost only)
- Safe for single-user development

### Network Deployment
- Set `host = "0.0.0.0"` to allow external connections
- Consider firewall rules
- Use HTTPS proxy for production
- Implement authentication if needed

## 📝 Logging

Set log levels via environment:
```bash
export RUST_LOG="renzora_server=debug,actix_web=info"
```

Or in config:
```toml
[logging]
level = "debug"
file_watching = true
connections = true
```

## 🎯 Success Indicators

Server is working correctly when you see:
```
🚀 Starting Renzora Server (High-Performance WebSocket)
📋 Loaded configuration from: renzora.toml
🌐 Server will run on: http://127.0.0.1:3002
📂 Base path: C:\Users\YourName\engine
📂 Projects path: C:\Users\YourName\engine\projects
👀 File watcher initialized
🎯 Renzora Server ready to accept connections
```

Frontend shows:
```
✅ Connected to Renzora WebSocket server
🚀 Transport: websocket Server: websocket
```