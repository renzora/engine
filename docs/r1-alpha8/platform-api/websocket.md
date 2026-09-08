# WebSocket API

The WebSocket API provides real-time communication for live updates, notifications, and game events.

## Connecting

Connect to the WebSocket endpoint with your authentication token:

```javascript
const ws = new WebSocket("wss://renzora.com/api/ws?token=rz_a1b2c3d4e5f6...");

ws.onopen = () => {
  console.log("Connected to Renzora WebSocket");
};

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log("Event:", data.type, data.payload);
};

ws.onclose = (event) => {
  console.log("Disconnected:", event.code, event.reason);
};
```

## Message Format

All messages follow this structure:

```json
{
  "type": "event_name",
  "payload": { }
}
```

## Sending Messages

### Subscribe to Channels

```json
{
  "type": "subscribe",
  "payload": {
    "channels": ["notifications", "app:my-game-id"]
  }
}
```

### Unsubscribe

```json
{
  "type": "unsubscribe",
  "payload": {
    "channels": ["app:my-game-id"]
  }
}
```

### Ping / Keepalive

Send a ping every 30 seconds to keep the connection alive:

```json
{ "type": "ping" }
```

The server responds with:

```json
{ "type": "pong" }
```

## Event Types

### Notifications

```json
{
  "type": "notification",
  "payload": {
    "id": "uuid",
    "kind": "friend_request",
    "message": "player42 sent you a friend request",
    "created_at": "2026-03-31T12:00:00Z"
  }
}
```

### Friend Status

```json
{
  "type": "friend_status",
  "payload": {
    "user_id": "uuid",
    "username": "player42",
    "status": "online"
  }
}
```

### New Message

```json
{
  "type": "new_message",
  "payload": {
    "conversation_id": "uuid",
    "message": {
      "id": "uuid",
      "sender": "player42",
      "content": "Hey, want to play?",
      "created_at": "2026-03-31T12:05:00Z"
    }
  }
}
```

### App Events (Game-specific)

When subscribed to an `app:{app_id}` channel:

```json
{
  "type": "app_event",
  "payload": {
    "app_id": "uuid",
    "event": "player_joined",
    "data": { "username": "new_player", "session_id": "abc" }
  }
}
```

## Connection Limits

| Tier | Max Connections |
|------|----------------|
| Free | 2 |
| Indie | 5 |
| Studio | 20 |
| Enterprise | Custom |

## Error Events

```json
{
  "type": "error",
  "payload": {
    "code": "invalid_channel",
    "message": "Channel 'app:invalid' does not exist"
  }
}
```

## Reconnection

If disconnected, implement exponential backoff:

```javascript
let retryDelay = 1000;

function connect() {
  const ws = new WebSocket("wss://renzora.com/api/ws?token=...");
  ws.onopen = () => { retryDelay = 1000; };
  ws.onclose = () => {
    setTimeout(connect, retryDelay);
    retryDelay = Math.min(retryDelay * 2, 30000);
  };
}
connect();
```
