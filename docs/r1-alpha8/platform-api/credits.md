# Credits API

Credits are Renzora's virtual currency used for marketplace purchases, tipping creators, and in-app transactions.

## Base URL

```
https://renzora.com/api/credits
```

## Check Balance

```bash
curl https://renzora.com/api/credits/balance \
  -H "Authorization: Bearer rz_..."
```

**Response:**

```json
{
  "balance": 2500,
  "pending_withdrawal": 0,
  "lifetime_earned": 8000
}
```

> **Note:** 1 credit = $0.01 USD. A balance of 2500 = $25.00.

## Top Up Credits

Initiate a credit purchase via Stripe:

```bash
curl -X POST https://renzora.com/api/credits/topup \
  -H "Authorization: Bearer rz_..." \
  -H "Content-Type: application/json" \
  -d '{"amount": 1000}'
```

**Response:**

```json
{
  "checkout_url": "https://checkout.stripe.com/...",
  "amount": 1000,
  "price_usd": "10.00"
}
```

Redirect the user to `checkout_url` to complete payment. Credits are added automatically after successful payment.

## Purchase (Transfer)

Transfer credits to another user (e.g., tipping a creator):

```bash
curl -X POST https://renzora.com/api/credits/transfer \
  -H "Authorization: Bearer rz_..." \
  -H "Content-Type: application/json" \
  -d '{"to_username": "artist42", "amount": 100, "note": "Great asset!"}'
```

## Withdraw

Request a withdrawal to your linked payout method:

```bash
curl -X POST https://renzora.com/api/credits/withdraw \
  -H "Authorization: Bearer rz_..." \
  -H "Content-Type: application/json" \
  -d '{"amount": 5000}'
```

**Response:**

```json
{
  "withdrawal_id": "uuid",
  "amount": 5000,
  "payout_usd": "50.00",
  "status": "pending",
  "estimated_arrival": "2026-04-03T00:00:00Z"
}
```

> Minimum withdrawal: 1000 credits ($10.00). Payouts are processed within 3 business days.

## Transaction History

```bash
curl "https://renzora.com/api/credits/transactions?page=1&limit=20" \
  -H "Authorization: Bearer rz_..."
```

**Response:**

```json
{
  "transactions": [
    {
      "id": "uuid",
      "type": "purchase",
      "amount": -500,
      "description": "Purchased: Low Poly Trees Pack",
      "created_at": "2026-03-30T14:22:00Z"
    },
    {
      "id": "uuid",
      "type": "topup",
      "amount": 1000,
      "description": "Credit top-up",
      "created_at": "2026-03-29T09:00:00Z"
    }
  ],
  "total": 45,
  "page": 1
}
```
