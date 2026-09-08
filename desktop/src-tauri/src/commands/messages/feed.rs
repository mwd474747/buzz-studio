use crate::models::FeedItemInfo;

pub(super) fn feed_item_from_event(event: &nostr::Event, category: &str) -> FeedItemInfo {
    let channel_id = event.tags.iter().find_map(|tag| {
        let values = tag.as_slice();
        (values.len() >= 2 && values[0] == "h").then(|| values[1].clone())
    });
    FeedItemInfo {
        id: event.id.to_hex(),
        kind: event.kind.as_u16() as u32,
        pubkey: event.pubkey.to_hex(),
        content: event.content.clone(),
        created_at: event.created_at.as_secs(),
        channel_id,
        channel_name: String::new(),
        channel_type: None,
        tags: event
            .tags
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect(),
        category: category.to_string(),
    }
}
