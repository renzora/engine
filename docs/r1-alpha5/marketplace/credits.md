# Credits System

Credits are the virtual currency on renzora.com — you buy them with real money, spend them on marketplace assets, and earn them by selling your own work.

> **Pricing:** **1 credit = $0.10 USD** (10 credits = $1.00). A balance of 250 credits is worth $25.00.

## Your wallet

Everything to do with credits lives on the [Credits page](/wallet) (the `/wallet` route). It shows three numbers at the top:

| Stat | Meaning |
|---|---|
| **Balance** | Credits available to spend right now |
| **Total Spent** | Credits you've spent buying assets |
| **Total Earned** | Credits you've earned from your own sales |

Below that you can add credits, see your referral link, and browse your full transaction history.

## Buying credits

Credits are purchased through **Stripe Checkout**. From the [Credits page](/wallet), pick one of the quick presets or enter a custom amount, then you're redirected to Stripe to pay securely by card.

| Preset | Credits | Price |
|---|---|---|
| — | 50 | $5 |
| — | 100 | $10 |
| Popular | 250 | $25 |
| Best Value | 500 | $50 |

The price is always a flat **$0.10 per credit** — there are no bulk bonuses. For any other amount, use the custom field (**minimum 50 credits**, in steps of 10).

> Credits are added to your balance automatically once Stripe confirms the payment. If you cancel at the Stripe screen, nothing is charged.

## Spending credits

To buy an asset:

1. Open an asset on the [Marketplace](/marketplace).
2. Click **Buy** (the price is shown in credits).
3. Credits are deducted from your balance and the asset is added to your [Library](/library).

A few rules the purchase flow enforces:

- **Free assets** (priced at 0 credits) are simply added to your Library at no cost.
- You **can't buy the same asset twice** — once it's in your Library it's yours.
- Buying your **own** asset just grants you access; you're never charged for it.
- If your balance is too low, the purchase is rejected — top up first.

## Earning as a creator

When someone buys your asset, the sale is split between you and the platform:

- **You keep 80%** of the sale price.
- **Renzora takes a 20% platform fee** (hosting, bandwidth, and payment processing).

> Example: you list an asset for **500 credits**. Each sale credits you **400 credits** and the platform fee is **100 credits**.

Earnings land in your balance immediately as an `earning` transaction. You can spend them like any other credits, or withdraw them to your bank (below).

### Withdrawing to your bank

Convert earned credits back into real money via **Stripe Connect**:

1. In [Settings](/settings), connect a bank account and complete Stripe onboarding (a one-time Stripe Express setup).
2. On the [Credits page](/wallet), request a withdrawal.
3. The credits are deducted and a Stripe transfer is sent to your bank.

| Rule | Value |
|---|---|
| Minimum withdrawal | **500 credits ($50.00)** |
| Pending withdrawals | One at a time |
| Payout method | Bank account via Stripe Connect |

> You must finish Stripe onboarding before withdrawing. If a transfer fails, the credits are automatically refunded to your balance.

## Promo codes

Promo codes are entered **on the asset page when you buy**, in the *Promo code* field next to the Buy button.

> ⚠️ A promo code is **not** free credits and it does **not** lower the price you pay. It waives part of the platform's 20% fee (up to 20 percentage points), so **more of the sale goes to the creator**. The buyer always pays the asset's listed price.

## Vouchers

Vouchers are codes Renzora issues directly (events, giveaways, partnerships). There are two kinds:

| Voucher type | Effect |
|---|---|
| **Credit** | Adds a fixed number of credits straight to your balance |
| **Asset discount** | Gives a percentage off your next eligible purchase |

Vouchers can be limited per user and may have an expiry date.

## Gift cards

Send credits to other people from the [Gifts page](/gifts):

- **Minimum gift: 10 credits**, deducted from your balance.
- Send **directly to a username** — the credits are redeemed and delivered instantly, and the recipient is notified.
- Or generate a **gift code** (`GIFT-XXXXXXXX`) to share however you like; the recipient redeems it on the same page.
- Unredeemed gift codes **expire after 90 days**.

## Donations

You can donate credits to support Renzora from the [Donate page](/donate):

- **Minimum donation: 1 credit.**
- Donations can include a message and may be anonymous.
- Cumulative donations unlock **donor badges** at 100, 500, 1,000, and 5,000 credits (bronze → silver → gold → platinum).
- There's a public **donation leaderboard** (anonymous donors are hidden).

## Referrals

Every account has a referral link (`/register?ref=<code>`), shown on the [Credits page](/wallet). Anyone who signs up through your link is permanently linked to you, and you earn **5% of every purchase they make** as a `referral` credit. The reward is capped by the platform's margin on each sale.

## Transaction history

The [Credits page](/wallet) lists every credit movement, newest first, filterable by type. The transaction types you'll see:

| Type | Meaning |
|---|---|
| `topup` | Credits bought via Stripe |
| `purchase` | Credits spent buying an asset |
| `earning` | Credits earned from one of your sales |
| `referral` | 5% reward from a referred user's purchase |
| `voucher_credit` | Credits added by redeeming a voucher |
| `gift_sent` / `gift_received` | A gift card you sent or received |
| `donation` | Credits you donated to Renzora |
| `withdrawal` / `withdrawal_refund` | A payout to your bank (and a refund if it failed) |
| `refund` | An admin-issued refund (see below) |

## Refunds

> Credits purchased with real money are **non-refundable except where required by law**.

Asset refunds are handled as **disputes**, not self-service. If an asset is non-functional or misrepresented, you can request a refund and the Renzora team reviews it. Approved refunds are deducted from the creator's balance and returned to you as a `refund` transaction.

## Related

- [Browsing & Installing](browsing) — finding and buying assets
- [Publishing Assets](publishing) — listing your own work to earn credits
