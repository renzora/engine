# Rate Limits

API requests are rate-limited per account on a daily basis. Limits vary by subscription tier.

## Limits by Tier

| Tier | Daily Requests | Burst (per minute) |
|------|---------------|-------------------|
| **Free** | 1,000 | 30 |
| **Indie** | 10,000 | 120 |
| **Studio** | 100,000 | 600 |
| **Enterprise** | Custom | Custom |

## Rate Limit Headers

Every API response includes rate limit information in the headers:

```
X-RateLimit-Limit: 10000
X-RateLimit-Remaining: 9847
X-RateLimit-Reset: 1711929600
```

| Header | Description |
|--------|-------------|
| `X-RateLimit-Limit` | Maximum requests for your tier |
| `X-RateLimit-Remaining` | Requests remaining in the current window |
| `X-RateLimit-Reset` | Unix timestamp when the limit resets (midnight UTC) |

## Exceeding Limits

When you exceed your rate limit, the API returns a `429 Too Many Requests` response:

```json
{
  "error": "rate_limit_exceeded",
  "message": "Daily request limit reached. Resets at midnight UTC.",
  "retry_after": 3600
}
```

## Best Practices

- **Cache responses** where possible to reduce unnecessary requests
- **Use WebSockets** for real-time data instead of polling
- **Batch operations** when the API supports it (e.g., bulk stat updates)
- **Monitor your usage** via the rate limit headers
- **Implement exponential backoff** when you receive 429 responses

## Upgrading

If you need higher limits, upgrade your subscription on the [Subscription](/subscription) page or contact us for Enterprise plans.
