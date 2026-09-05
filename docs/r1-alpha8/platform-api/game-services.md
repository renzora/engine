# Game Services API

The Game Services API provides backend features for published games: app management, achievements, stats, leaderboards, and friends.

## Base URL

```
https://renzora.com/api
```

## Apps

### Register an App

```bash
curl -X POST https://renzora.com/api/apps \
  -H "Authorization: Bearer rz_..." \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My Game",
    "description": "An awesome multiplayer game",
    "website": "https://mygame.com"
  }'
```

**Response:**

```json
{
  "id": "uuid",
  "name": "My Game",
  "app_token": "rza_x9y8z7...",
  "created_at": "2026-03-31T00:00:00Z"
}
```

### List Your Apps

```bash
curl https://renzora.com/api/apps \
  -H "Authorization: Bearer rz_..."
```

## Achievements

Define and unlock achievements for your game's players.

### Define Achievements

```bash
curl -X POST https://renzora.com/api/apps/{app_id}/achievements \
  -H "Authorization: Bearer rza_..." \
  -H "Content-Type: application/json" \
  -d '{
    "name": "First Blood",
    "description": "Win your first battle",
    "icon_url": "https://example.com/icon.png",
    "points": 10,
    "hidden": false
  }'
```

### Unlock Achievement for Player

```bash
curl -X POST https://renzora.com/api/apps/{app_id}/achievements/{achievement_id}/unlock \
  -H "Authorization: Bearer rza_..." \
  -H "Content-Type: application/json" \
  -d '{"user_id": "player-uuid"}'
```

### Get Player Achievements

```bash
curl "https://renzora.com/api/apps/{app_id}/achievements?user_id={player_uuid}" \
  -H "Authorization: Bearer rza_..."
```

## Stats

Track per-player statistics like play time, scores, and custom metrics.

### Update Player Stats

```bash
curl -X POST https://renzora.com/api/apps/{app_id}/stats \
  -H "Authorization: Bearer rza_..." \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "player-uuid",
    "stats": {
      "kills": 42,
      "play_time_seconds": 3600,
      "level": 5
    }
  }'
```

### Get Player Stats

```bash
curl "https://renzora.com/api/apps/{app_id}/stats/{user_id}" \
  -H "Authorization: Bearer rza_..."
```

## Leaderboards

### Create a Leaderboard

```bash
curl -X POST https://renzora.com/api/apps/{app_id}/leaderboards \
  -H "Authorization: Bearer rza_..." \
  -H "Content-Type: application/json" \
  -d '{
    "name": "High Scores",
    "stat_key": "score",
    "sort": "desc",
    "reset_interval": "weekly"
  }'
```

### Submit Score

```bash
curl -X POST https://renzora.com/api/apps/{app_id}/leaderboards/{board_id}/submit \
  -H "Authorization: Bearer rza_..." \
  -H "Content-Type: application/json" \
  -d '{"user_id": "player-uuid", "score": 9500}'
```

### Get Leaderboard

```bash
curl "https://renzora.com/api/apps/{app_id}/leaderboards/{board_id}?limit=50" \
  -H "Authorization: Bearer rza_..."
```

**Response:**

```json
{
  "leaderboard": "High Scores",
  "entries": [
    { "rank": 1, "username": "pro_gamer", "score": 15000 },
    { "rank": 2, "username": "speedrunner", "score": 12300 }
  ]
}
```

## Friends

### Get Friends List

```bash
curl https://renzora.com/api/friends \
  -H "Authorization: Bearer rz_..."
```

### Send Friend Request

```bash
curl -X POST https://renzora.com/api/friends/request \
  -H "Authorization: Bearer rz_..." \
  -H "Content-Type: application/json" \
  -d '{"username": "other_player"}'
```

### Accept / Decline

```bash
curl -X POST https://renzora.com/api/friends/request/{request_id}/accept \
  -H "Authorization: Bearer rz_..."
```

### Get Online Friends

```bash
curl https://renzora.com/api/friends/online \
  -H "Authorization: Bearer rz_..."
```
