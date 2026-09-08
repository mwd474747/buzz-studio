/// Return the command-facing error for a relay redirect response.
pub(super) fn redirect_refusal_error(status: reqwest::StatusCode) -> Option<String> {
    status.is_redirection().then(|| {
        format!(
            "media fetch refused: relay returned a {status} redirect, which is \
             not followed for authenticated downloads (redirect-hop SSRF guard)"
        )
    })
}
