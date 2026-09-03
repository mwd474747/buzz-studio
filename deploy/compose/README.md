# Buzz Docker Compose deployment

Local Docker prod relay is the studio transport: `compose.yml` only,
`ws://localhost:3300`, NIP-29 room. There is no second required compose,
no Prometheus overlay, and no A-01E activation step.

Root `docker-compose.yml` remains contributor infrastructure for
`just setup` / CI. It is not the advertised studio path.

## Quick start

```bash
cd deploy/compose
cp .env.example .env
# RELAY_OWNER_PUBKEY is the existing #local-dev pin. Do not remint.
# Fill remaining CHANGE_ME secrets. Do not rotate the owner pin.
./run.sh start
```

Relay: `ws://localhost:3300`.

For a public VPS with automatic Let's Encrypt certificates:

```bash
cd deploy/compose
BUZZ_COMPOSE_TLS=true ./run.sh start
```

Do not generate a new owner keypair. The identity target is the existing
`#local-dev` pin in `.env.example`.

## Production notes

- Requires Docker Compose v2.24.4 or newer; the TLS override uses Compose's
  `!reset` tag to remove the direct relay port when Caddy terminates HTTPS.
- Default `BUZZ_IMAGE` tracks `ghcr.io/block/buzz:main` for early testing. Pin it to `ghcr.io/block/buzz:sha-<7>` or a semver release tag for production once available.
- Keep `BUZZ_RELAY_PRIVATE_KEY`, `BUZZ_GIT_HOOK_HMAC_SECRET`, database/Redis,
  and S3 secrets stable across restarts. Do not remint the owner pin.
- `RELAY_OWNER_PUBKEY` is intentionally not prefixed with `BUZZ_`; it must be the
  full 64-character hex Nostr pubkey (not an 8-hex prefix).
- `BUZZ_AUTO_MIGRATE` is opt-in. Set `BUZZ_AUTO_MIGRATE=true` or run
  `buzz-admin migrate` before starting the relay when bootstrapping a fresh
  database. Auto-migration requires an image that includes embedded SQLx
  migrations.
- The stack uses Postgres, Redis, MinIO, and a git data volume because
  those are real Buzz dependencies today.
- The bundled Compose stack fixes the relay endpoint to `http://minio:9000` and
  `BUZZ_S3_ADDRESSING_STYLE=path`: Docker DNS resolves `minio`, not
  `<bucket>.minio`. It is not configurable for an external S3 provider through
  `.env`; use the Helm chart or a custom Compose configuration for providers
  such as new Railway Storage Buckets that require `virtual` addressing.

Run `./run.sh backup-hint` for the backup checklist.

## Validation

```bash
cd deploy/compose
cp .env.example .env
# keep the #local-dev owner pin; fill remaining CHANGE_ME secrets
./run.sh config
./run.sh start
curl -fsS "http://127.0.0.1:$(grep -E '^BUZZ_HTTP_PORT=' .env | cut -d= -f2-)/_liveness"
./run.sh status
```
