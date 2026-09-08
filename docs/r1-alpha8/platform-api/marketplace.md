# Marketplace API

The Marketplace API allows you to browse, upload, and manage assets on the Renzora Marketplace.

## Base URL

```
https://renzora.com/api/marketplace
```

## Browse Assets

### List Assets

```bash
curl "https://renzora.com/api/marketplace/assets?page=1&limit=20&category=3d-models"
```

**Query Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `page` | integer | Page number (default: 1) |
| `limit` | integer | Items per page (default: 20, max: 100) |
| `category` | string | Filter by category slug |
| `search` | string | Search query |
| `sort` | string | `newest`, `popular`, `price_asc`, `price_desc` |
| `price` | string | `free`, `paid`, or omit for all |

**Response:**

```json
{
  "assets": [
    {
      "id": "uuid",
      "slug": "low-poly-trees",
      "title": "Low Poly Trees Pack",
      "description": "50 low-poly tree models",
      "price": 500,
      "thumbnail_url": "/uploads/thumbnails/abc123.webp",
      "author": { "username": "artist42", "avatar_url": "..." },
      "category": "3d-models",
      "download_count": 1234,
      "rating": 4.8,
      "created_at": "2026-03-15T10:30:00Z"
    }
  ],
  "total": 156,
  "page": 1,
  "pages": 8
}
```

### Get Asset Details

```bash
curl "https://renzora.com/api/marketplace/assets/{slug}"
```

### Get Categories

```bash
curl "https://renzora.com/api/marketplace/categories"
```

## Upload Assets

Upload a new asset to the marketplace. Requires authentication.

### Step 1: Create Asset Listing

```bash
curl -X POST https://renzora.com/api/marketplace/assets \
  -H "Authorization: Bearer rz_..." \
  -H "Content-Type: multipart/form-data" \
  -F "title=My Asset Pack" \
  -F "description=A great asset pack" \
  -F "category=3d-models" \
  -F "price=500" \
  -F "tags=lowpoly,trees,nature" \
  -F "file=@asset-pack.zip" \
  -F "thumbnail=@preview.png"
```

> **Note:** Prices are in credits. Set `price=0` for free assets.

### Step 2: Asset Review

After upload, assets enter a review queue. You'll receive a notification when your asset is approved or if changes are requested.

## Download Assets

### Purchase and Download

```bash
curl -X POST https://renzora.com/api/marketplace/assets/{slug}/download \
  -H "Authorization: Bearer rz_..."
```

For paid assets, credits are deducted from your balance. Free assets download immediately.

## Manage Your Assets

### Update Asset

```bash
curl -X PATCH https://renzora.com/api/marketplace/assets/{slug} \
  -H "Authorization: Bearer rz_..." \
  -H "Content-Type: application/json" \
  -d '{"description": "Updated description", "price": 600}'
```

### Delete Asset

```bash
curl -X DELETE https://renzora.com/api/marketplace/assets/{slug} \
  -H "Authorization: Bearer rz_..."
```

### List Your Assets

```bash
curl "https://renzora.com/api/marketplace/my-assets" \
  -H "Authorization: Bearer rz_..."
```
