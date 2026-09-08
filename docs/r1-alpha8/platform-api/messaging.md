# Messaging API

The Messaging API provides direct messaging between users, including conversations and real-time message delivery.

## Base URL

```
https://renzora.com/api/messages
```

## Conversations

### List Conversations

```bash
curl https://renzora.com/api/messages/conversations \
  -H "Authorization: Bearer rz_..."
```

**Response:**

```json
{
  "conversations": [
    {
      "id": "uuid",
      "participants": [
        { "user_id": "uuid", "username": "player42", "avatar_url": "..." }
      ],
      "last_message": {
        "content": "See you in game!",
        "sender": "player42",
        "created_at": "2026-03-31T11:00:00Z"
      },
      "unread_count": 2,
      "updated_at": "2026-03-31T11:00:00Z"
    }
  ]
}
```

### Create Conversation

```bash
curl -X POST https://renzora.com/api/messages/conversations \
  -H "Authorization: Bearer rz_..." \
  -H "Content-Type: application/json" \
  -d '{"participant_ids": ["user-uuid-1", "user-uuid-2"]}'
```

## Messages

### Get Messages

```bash
curl "https://renzora.com/api/messages/conversations/{conversation_id}?limit=50&before={message_id}" \
  -H "Authorization: Bearer rz_..."
```

**Query Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `limit` | integer | Messages to return (default: 50, max: 100) |
| `before` | string | Cursor for pagination (message ID) |

**Response:**

```json
{
  "messages": [
    {
      "id": "uuid",
      "sender_id": "uuid",
      "sender_username": "player42",
      "content": "Hey, nice game!",
      "created_at": "2026-03-31T10:55:00Z"
    },
    {
      "id": "uuid",
      "sender_id": "uuid",
      "sender_username": "you",
      "content": "Thanks!",
      "created_at": "2026-03-31T10:56:00Z"
    }
  ],
  "has_more": true
}
```

### Send Message

```bash
curl -X POST https://renzora.com/api/messages/conversations/{conversation_id} \
  -H "Authorization: Bearer rz_..." \
  -H "Content-Type: application/json" \
  -d '{"content": "Hello!"}'
```

### Mark as Read

```bash
curl -X POST https://renzora.com/api/messages/conversations/{conversation_id}/read \
  -H "Authorization: Bearer rz_..."
```

## Real-time Messages

New messages are delivered in real-time via the [WebSocket API](/docs/developer/api-websocket). Subscribe to the `notifications` channel to receive `new_message` events:

```json
{
  "type": "new_message",
  "payload": {
    "conversation_id": "uuid",
    "message": {
      "id": "uuid",
      "sender": "player42",
      "content": "Want to team up?",
      "created_at": "2026-03-31T12:05:00Z"
    }
  }
}
```
