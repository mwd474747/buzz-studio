use futures_util::StreamExt;

use super::MALFORMED_RESPONSE_MESSAGE;

pub(super) const MAX_RELAY_JSON_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
pub(super) const MAX_RELAY_ERROR_RESPONSE_BYTES: usize = 64 * 1024;

pub(super) async fn read_bounded_response(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err("relay returned oversized response".to_string());
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| MALFORMED_RESPONSE_MESSAGE.to_string())?;
        if body.len().saturating_add(chunk.len()) > max_bytes {
            return Err("relay returned oversized response".to_string());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
