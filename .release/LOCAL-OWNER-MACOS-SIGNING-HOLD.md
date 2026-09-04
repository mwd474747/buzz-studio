# Local owner macOS signing hold

Status: **HOLD**.

The local-owner profile requires a real Apple signing identity and Team ID,
but neither is currently available or ratified. Both fields therefore remain
`null` in `.release/local-owner-profile.json`.

Source builds and tests may run while the fields are empty. A distributable or
admitted Mac candidate must not be claimed until the existing Tauri macOS lane
has produced a real `Buzz.app` and an independent check has confirmed:

- exact source commit and local-owner profile digest;
- bundle identifier `xyz.block.buzz.app`;
- the later-ratified Team ID and codesign identity;
- strict codesign verification, Gatekeeper assessment, notarization, and a
  stapled ticket; and
- launch with the existing owner identity and a read-only `#local-dev` read.

Do not invent a Team ID, use an ad-hoc signature as production evidence, or
mint a replacement Buzz identity.

The generated source receipt is descriptive build input, not an independent
attestation. It becomes authenticated artifact metadata only when it is
embedded in, and independently checked against, a genuinely signed app.
