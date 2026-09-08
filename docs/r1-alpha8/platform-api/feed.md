# Feed API

The Feed API powers the social feed, allowing users to create posts, comment, and interact with community content.

## Base URL

```
https://renzora.com/api/feed
```

## Posts

### Get Feed

```bash
curl "https://renzora.com/api/feed?page=1&limit=20" \
  -H "Authorization: Bearer rz_..."
```

**Query Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `page` | integer | Page number (default: 1) |
| `limit` | integer | Posts per page (default: 20, max: 50) |
| `user_id` | string | Filter by user (for profile feeds) |

**Response:**

```json
{
  "posts": [
    {
      "id": "uuid",
      "author": {
        "user_id": "uuid",
        "username": "creator99",
        "avatar_url": "..."
      },
      "content": "Just released a new terrain shader pack!",
      "images": ["/uploads/assets/img123.webp"],
      "like_count": 42,
      "comment_count": 7,
      "liked_by_me": false,
      "created_at": "2026-03-31T08:00:00Z"
    }
  ],
  "total": 230,
  "page": 1
}
```

### Create Post

```bash
curl -X POST https://renzora.com/api/feed \
  -H "Authorization: Bearer rz_..." \
  -H "Content-Type: application/json" \
  -d '{"content": "Working on something cool!", "images": []}'
```

### Delete Post

```bash
curl -X DELETE https://renzora.com/api/feed/{post_id} \
  -H "Authorization: Bearer rz_..."
```

## Comments

### Get Comments

```bash
curl "https://renzora.com/api/feed/{post_id}/comments?page=1&limit=20" \
  -H "Authorization: Bearer rz_..."
```

### Add Comment

```bash
curl -X POST https://renzora.com/api/feed/{post_id}/comments \
  -H "Authorization: Bearer rz_..." \
  -H "Content-Type: application/json" \
  -d '{"content": "This looks amazing!"}'
```

### Delete Comment

```bash
curl -X DELETE https://renzora.com/api/feed/{post_id}/comments/{comment_id} \
  -H "Authorization: Bearer rz_..."
```

## Likes

### Like a Post

```bash
curl -X POST https://renzora.com/api/feed/{post_id}/like \
  -H "Authorization: Bearer rz_..."
```

### Unlike a Post

```bash
curl -X DELETE https://renzora.com/api/feed/{post_id}/like \
  -H "Authorization: Bearer rz_..."
```

## Real-time Updates

Feed activity for your network is delivered via [WebSocket](/docs/developer/api-websocket) on the `notifications` channel. Events include `new_like`, `new_comment`, and `new_follow`.
