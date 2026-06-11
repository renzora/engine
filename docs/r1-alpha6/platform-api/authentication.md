# Authentication

All API requests require authentication via Bearer tokens in the `Authorization` header.

## Token Types

Renzora uses two types of API tokens:

| Type | Prefix | Purpose | Scope |
|------|--------|---------|-------|
| **User Token (JWT)** | `eyJ...` | Browser sessions, login flow | Full user context |
| **API Token** | `rz_` | Server-to-server, CLI tools | Scoped by creation |
| **App Token** | `rza_` | Published apps/games | App-specific operations |

## Obtaining Tokens

### User Tokens (JWT)

Authenticate via the login endpoint to receive a JWT:

```bash
curl -X POST https://renzora.com/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email": "you@example.com", "password": "your-password"}'
```

Response:

```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": {
    "id": "uuid",
    "username": "you",
    "email": "you@example.com"
  }
}
```

### API Tokens

Generate API tokens from the [Developers](/developers) page or via the API:

```bash
curl -X POST https://renzora.com/api/tokens \
  -H "Authorization: Bearer eyJhbG..." \
  -H "Content-Type: application/json" \
  -d '{"name": "My CI Token"}'
```

Response:

```json
{
  "token": "rz_a1b2c3d4e5f6...",
  "name": "My CI Token",
  "created_at": "2026-03-31T00:00:00Z"
}
```

> **Important:** API tokens are shown only once at creation. Store them securely.

### App Tokens

App tokens are created when you register an application:

```bash
curl -X POST https://renzora.com/api/apps \
  -H "Authorization: Bearer eyJhbG..." \
  -H "Content-Type: application/json" \
  -d '{"name": "My Game", "description": "An awesome game"}'
```

The response includes an `rza_` prefixed token scoped to that app's operations.

## Using Tokens

Include the token in the `Authorization` header for all API requests:

```bash
curl https://renzora.com/api/me \
  -H "Authorization: Bearer rz_a1b2c3d4e5f6..."
```

## Token Revocation

Revoke a token by its ID:

```bash
curl -X DELETE https://renzora.com/api/tokens/{token_id} \
  -H "Authorization: Bearer eyJhbG..."
```

## Error Responses

| Status | Meaning |
|--------|---------|
| `401 Unauthorized` | Missing or invalid token |
| `403 Forbidden` | Token lacks required scope |
| `429 Too Many Requests` | Rate limit exceeded |
