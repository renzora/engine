//! Deep-link routing: notification `link` paths (site URLs) → social panels.

use renzora::core::SocialPanelRequest;
use renzora_auth::social::NotificationRow;

/// Best panel target for a notification: its `link` if we can route it,
/// otherwise a sensible default per notification type.
pub(crate) fn route_notification(row: &NotificationRow) -> SocialPanelRequest {
    if let Some(link) = &row.link {
        if let Some(req) = route_link(link) {
            return req;
        }
    }
    match row.kind.as_str() {
        "friend_request" => SocialPanelRequest::FriendRequests,
        "friend_accepted" | "follow" => SocialPanelRequest::Friends,
        "team_invite" | "team_member_joined" | "library_request" | "library_request_approved"
        | "library_request_denied" => SocialPanelRequest::Teams,
        "reply" => SocialPanelRequest::Forum { thread_slug: None },
        // Feed activity without a parseable link still lands on the feed.
        "mention" | "comment" | "like" | "reaction" => SocialPanelRequest::Feed { post_id: None },
        // No better target — the feed is the catch-all "what's happening" view.
        // (Not Notifications: that just re-opens the dropdown you clicked from.)
        _ => SocialPanelRequest::Feed { post_id: None },
    }
}

/// Map a site path (e.g. `/forum/thread/my-thread`) to a panel request.
pub(crate) fn route_link(link: &str) -> Option<SocialPanelRequest> {
    let path = link.split(['?', '#']).next().unwrap_or(link);
    let segs: Vec<&str> = path.trim_start_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    match segs.as_slice() {
        ["messages", ..] => Some(SocialPanelRequest::Chat { conversation_id: None }),
        ["forum", "thread", slug, ..] => {
            Some(SocialPanelRequest::Forum { thread_slug: Some((*slug).to_string()) })
        }
        ["forum", ..] => Some(SocialPanelRequest::Forum { thread_slug: None }),
        // Team invites currently link to the site's /settings page.
        ["teams", ..] | ["settings", ..] => Some(SocialPanelRequest::Teams),
        ["profile", user, ..] | ["u", user, ..] => {
            Some(SocialPanelRequest::Profile { username: Some((*user).to_string()) })
        }
        // Mention/comment notifications link to the post: `/feed/post/{id}`,
        // `/post/{id}` or `/feed/{id}` — capture the id so the feed panel can
        // expand and highlight that post instead of just opening the panel.
        ["feed", "post", id, ..] | ["post", id, ..] | ["feed", id, ..] => {
            Some(SocialPanelRequest::Feed { post_id: Some((*id).to_string()) })
        }
        ["feed"] | ["post"] => Some(SocialPanelRequest::Feed { post_id: None }),
        _ => None,
    }
}
