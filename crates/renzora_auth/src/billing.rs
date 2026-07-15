//! renzora.com credits, donations, and creator-onboarding API client.
//!
//! Everything is denominated in **credits** (1 credit = $0.10). Real money
//! enters only via Stripe: buying credits (`topup`) and creator payout
//! onboarding (`connect`) both return a hosted URL the editor opens in the
//! browser — card details are never handled in-app. Native-only blocking HTTP.

use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use crate::client::{api_base, get_json, post_json, require_token};
use crate::session::AuthSession;

/// 1 credit = $0.10 USD.
pub const CREDIT_USD_CENTS: i64 = 10;

/// Pull a hosted URL out of a Stripe endpoint response, tolerating the exact
/// key the server uses (`url` / `checkout_url` / `onboard_url` / …).
#[cfg(not(target_arch = "wasm32"))]
fn extract_url(v: &serde_json::Value) -> Result<String, String> {
    for k in ["url", "checkout_url", "onboard_url", "account_link", "link"] {
        if let Some(s) = v.get(k).and_then(|x| x.as_str()) {
            return Ok(s.to_string());
        }
    }
    Err("Server did not return a payment URL".to_string())
}

// ── Creator onboarding ───────────────────────────────────────────────────────

/// Creator-onboarding state (three booleans the wizard steps off).
/// `GET /api/creator/onboard-status`.
#[derive(Deserialize, Clone, Copy, Default, Debug)]
pub struct OnboardStatus {
    #[serde(default)]
    pub policy_accepted: bool,
    #[serde(default)]
    pub stripe_connected: bool,
    #[serde(default)]
    pub stripe_onboarded: bool,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn onboard_status(session: &AuthSession) -> Result<OnboardStatus, String> {
    let token = require_token(session)?;
    get_json(&format!("{}/api/creator/onboard-status", api_base()), Some(token))
}

/// Accept the creator agreement. `POST /api/creator/accept-policy`.
#[cfg(not(target_arch = "wasm32"))]
pub fn accept_policy(session: &AuthSession) -> Result<serde_json::Value, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/creator/accept-policy", api_base()),
        &serde_json::json!({}),
        Some(token),
    )
}

/// Begin Stripe Connect payout onboarding; returns a hosted URL to open in the
/// browser. `POST /api/credits/connect/onboard`.
#[cfg(not(target_arch = "wasm32"))]
pub fn start_connect_onboarding(session: &AuthSession) -> Result<String, String> {
    let token = require_token(session)?;
    let v: serde_json::Value = post_json(
        &format!("{}/api/credits/connect/onboard", api_base()),
        &serde_json::json!({}),
        Some(token),
    )?;
    extract_url(&v)
}

// ── Buy credits (Stripe Checkout) ────────────────────────────────────────────

/// Start a credit top-up; returns a Stripe Checkout URL to open in the browser.
/// `POST /api/credits/topup` (min 50 credits).
#[cfg(not(target_arch = "wasm32"))]
pub fn topup_checkout_url(session: &AuthSession, amount: i64) -> Result<String, String> {
    let token = require_token(session)?;
    let v: serde_json::Value = post_json(
        &format!("{}/api/credits/topup", api_base()),
        &serde_json::json!({ "amount": amount }),
        Some(token),
    )?;
    extract_url(&v)
}

// ── Donations (spend credits, support the platform) ──────────────────────────

#[derive(Serialize)]
struct DonateBody<'a> {
    amount: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'a str>,
    anonymous: bool,
}

#[derive(Deserialize, Debug)]
pub struct DonateResponse {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub amount: i64,
    #[serde(default)]
    pub total_donated: i64,
}

/// Donate credits to the platform. `POST /api/credits/donate` (min 1 credit).
#[cfg(not(target_arch = "wasm32"))]
pub fn donate(
    session: &AuthSession,
    amount: i64,
    message: Option<&str>,
    anonymous: bool,
) -> Result<DonateResponse, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/credits/donate", api_base()),
        &DonateBody { amount, message, anonymous },
        Some(token),
    )
}

#[derive(Deserialize, Default)]
pub struct DonationTotal {
    #[serde(default)]
    pub total: i64,
}

/// Total credits donated to the platform (public). `GET /api/credits/donate/total`.
#[cfg(not(target_arch = "wasm32"))]
pub fn donation_total() -> Result<DonationTotal, String> {
    get_json(&format!("{}/api/credits/donate/total", api_base()), None)
}

#[derive(Deserialize, Clone, Debug)]
pub struct DonationLeader {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub total: i64,
}

/// Top donors (public; anonymous entries hide identity). `GET /api/credits/donate/leaderboard`.
#[cfg(not(target_arch = "wasm32"))]
pub fn donation_leaderboard() -> Result<Vec<DonationLeader>, String> {
    get_json(&format!("{}/api/credits/donate/leaderboard", api_base()), None)
}
