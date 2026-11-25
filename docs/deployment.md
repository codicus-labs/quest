# Deployment

`quest-server` runs as one container with one persistent volume. This guide covers the image, the configuration,
the identity provider, operation behind a reverse proxy, and backup and restore. The
[example manifest](#example-manifest) shows a Kubernetes setup; any container runtime works the same way.

## Image

`docker build -t quest-server .` builds it from the [`Dockerfile`](../Dockerfile):

- Build stage `rust:1.99.1-slim-trixie` (same toolchain as `rust-toolchain.toml`), `cargo build --release --locked
  -p quest-server`.
- Runtime `gcr.io/distroless/cc-debian13:nonroot`. The binary needs only glibc, libgcc and CA certificates: SQLite is
  bundled, TLS is rustls (ring), the migrations are compiled in. Distroless has no shell or package manager, so a
  compromised process has less to work with.
- Runs as the user `nonroot` (65532:65532). Port `8787`. Volume `/data`, owned by 65532.
- Preset: `QUEST_BIND=0.0.0.0:8787`, `QUEST_DATABASE_URL=sqlite:///data/quest.sqlite`, `QUEST_LOG_FORMAT=json`.

CI ([`.github/workflows/server.yml`](../.github/workflows/server.yml)) checks (fmt, clippy, tests of `quest-server` and
`quest-protocol`) and builds the image on every push and pull request that touches the server. Pushes to the default
branch and release tags publish it to `ghcr.io/<owner>/quest-server`:

| Trigger | Tags |
|---|---|
| Push to `main` | `:main`, `:sha-<short>` |
| Tag `server-vX.Y.Z` | `:X.Y.Z`, `:latest` |

Pin a version (`:X.Y.Z` or `:sha-…`) in your deployment rather than `:latest`.

## What it needs

- **A persistent volume** for `/data` (SQLite in WAL mode: `quest.sqlite`, `-wal`, `-shm`). Every start applies
  pending migrations. They only go forward: going back to an older image after a migration means restoring a backup.
- **An OpenID Connect provider** with a confidential client (see [below](#oidc-client)). Any standard provider works,
  e.g. Keycloak, Authelia or Authentik.
- **TLS in front**: a reverse proxy or load balancer that terminates https. `QUEST_PUBLIC_URL` is the https origin;
  it is the OAuth issuer, the audience of all tokens and the origin of the redirect URIs. Changing it later
  invalidates all tokens.
- **Optional: an AI provider** with an OpenAI-compatible API (e.g. OpenRouter or OpenAI) for AI capture; without
  `QUEST_AI_*` the feature is off.

## Environment

All variables also exist as flags (`quest-server --help`). Keep secrets in your platform's secret store; everything
else can sit in plain configuration.

| Variable | Example | Kind | |
|---|---|---|---|
| `QUEST_PUBLIC_URL` | `https://quest.example.com` | config | Required off loopback |
| `QUEST_BIND` | `0.0.0.0:8787` | image | |
| `QUEST_DATABASE_URL` | `sqlite:///data/quest.sqlite` | image | |
| `QUEST_LOG_FORMAT` | `json` | image | |
| `RUST_LOG` | `info,rmcp=warn` (default) | config | Log filter |
| `QUEST_TIME_ZONE` | `Europe/Berlin` | config | Default zone of MCP tool calls (default `UTC`) |
| `QUEST_CLIENT_IP_HEADER` | `x-real-ip` | config | Optional: rate limits by the one address in this header (see [client addresses](#client-addresses)). Excludes `QUEST_TRUST_FORWARDED_FOR` |
| `QUEST_TRUST_FORWARDED_FOR` | `true` | config | Optional: rate limits by the last `X-Forwarded-For` entry (see [client addresses](#client-addresses)) |
| `QUEST_ANDROID_CERT_SHA256` | `AA:BB:…` (32 bytes) | config | Optional: SHA-256 of an Android signing certificate for `/.well-known/assetlinks.json` (App Links, see [`auth.md`](auth.md#android-app-links-optional)); comma-separated for more |
| `QUEST_MCP_DISABLED` | unset | config | Switches `/mcp` off |
| `QUEST_OIDC_ISSUER` | `https://id.example.com` | secret | Required |
| `QUEST_OIDC_CLIENT_ID` | from the provider | secret | Required |
| `QUEST_OIDC_CLIENT_SECRET` | from the provider | secret | Required; never logged |
| `QUEST_AI_BASE_URL` | `https://openrouter.ai/api/v1` | secret | AI capture: all three or none |
| `QUEST_AI_API_KEY` | provider key | secret | Never logged |
| `QUEST_AI_MODEL` | see [`ai-models.md`](ai-models.md) | secret | |
| `QUEST_AI_FALLBACK_MODELS`, `QUEST_AI_REASONING_EFFORT`, `QUEST_AI_APP_TITLE`, `QUEST_AI_TIMEOUT_SECS` | optional | secret | [`quest-server/README.md`](../crates/quest-server/README.md#ai-capture-post-v1aicapture) |

The server refuses to start if sign-in is not configured or the AI configuration is incomplete (exit 1, a log line
naming the variables), or if a value cannot be parsed, e.g. a malformed fingerprint (exit 2, the flag and the reason
on stderr).

## OIDC client

Register one client for the server at your provider:

- A **confidential** client (the server keeps the secret), e.g. named `quest-server`.
- Redirect (callback) URI `https://quest.example.com/auth/callback`.
- Post-logout redirect URI `https://quest.example.com/auth/logged-out`.
- PKCE may be required: the server always sends `S256`.
- Scopes: the server asks for `openid profile email`.
- Every user the provider lets through gets a Quest account: restrict the client to a group if needed.

The apps never talk to the provider directly. They sign in at the server, which redirects them to `/auth/callback`
(desktop: loopback, Android: `app.quest.android:/oauth2redirect`). Those redirect URIs are registered at the server,
not at the provider. Details: [`auth.md`](auth.md).

The server must reach the issuer for discovery, keys and the code exchange.

## Operation

- **One instance.** SQLite on a single volume, rate limits and grant revocations are in memory. Run exactly one
  replica and stop the old one before the new one opens the database (in Kubernetes `replicas: 1`,
  `strategy: Recreate`).
- **Health:** `GET /health/live` answers 200 while the process serves requests. `GET /health/ready` answers 200 while
  the database answers a query, and 503 otherwise or once shutdown has begun. Both are cheap and unauthenticated.
- **Shutdown:** on SIGTERM (or Ctrl-C) the server stops accepting connections, closes every open sync WebSocket with a
  close frame (the apps reconnect with backoff), lets running requests finish (at most 20 s; an AI capture can take
  up to `QUEST_AI_TIMEOUT_SECS`) and closes the database. That fits a 30 s grace period; SQLite's WAL keeps the
  database consistent even if requests are cut off; the process then exits with code 1.
- **Logs:** with `QUEST_LOG_FORMAT=json`, one JSON object per line on stdout with `timestamp`, `level`, `message`,
  `target` and fields. They never contain request bodies, dictations, task titles, tool arguments or results,
  tokens, codes, secrets or query strings; at most ids, counts and error kinds.
- **Metrics:** none yet; health probes and logs cover operation.
- **Rate limits** (in memory, per client address) reset on every restart.

## Reverse proxy, TLS and WebSocket

- The proxy terminates TLS and forwards plain http to port 8787. Do not expose 8787 otherwise.
- Redirect http to https. The sign-in cookies are `Secure` on an https origin.
- WebSocket (`/v1/sync`) must be forwarded (`Upgrade`/`Connection` headers). The server pings every 15 s and drops a
  connection after 45 s without traffic, so idle timeouts of 60 s or more on the way are fine. Do not buffer or
  compress `/v1/sync` or `/mcp`.
- No path rewriting: the server needs `/`, `/.well-known/*`, `/authorize`, `/token`, `/revoke`, `/register`,
  `/auth/*`, `/v1/*`, `/mcp` and `/health/*` at the root of the host. `/health/*` may stay unrouted from outside.

### Client addresses

The sign-in rate limits must see each client's own address; otherwise all clients share one limit and one attacker
can lock everyone out of signing in. By default the server counts the connecting address, which behind a proxy is
the proxy's. Pick one option, and only if every request reaches the server through a proxy that sets the value
itself:

- `QUEST_TRUST_FORWARDED_FOR=true`: the last `X-Forwarded-For` entry counts, i.e. the address the proxy in front of
  the server saw. Make sure that is the client's address (e.g. a load balancer may need to preserve source addresses).
- `QUEST_CLIENT_IP_HEADER=<header>`: the one address in this header counts, for an edge that sets a single-address
  header and overwrites one a client sends (e.g. `x-real-ip`). A missing or unreadable header counts the connecting
  address.

Check after setup: a request from outside should be counted under your public address (debug logs of the rate
limiter or the proxy's access log).

## Backup

The database is one SQLite file in WAL mode. Copying it with `cp` while the server runs can give a broken copy; use
SQLite's online backup instead:

- **`sqlite3 .backup`** from a scheduled job that mounts the same volume. The image has no shell; use a small image
  with `sqlite3`:

  ```sh
  sqlite3 /data/quest.sqlite ".backup '/backup/quest-$(date +%F).sqlite'"
  ```

  Then copy `/backup` off the machine (restic, rclone or similar) and keep a few generations.
- **Litestream** (option): a sidecar streams the WAL continuously to S3-compatible storage and restores on start. It
  gives point-in-time recovery to seconds, at the cost of a second container and a bucket.

## Restore

Stop the server (scale to 0), put the backup at `/data/quest.sqlite`, delete `quest.sqlite-wal` and
`quest.sqlite-shm`, start it again. The server migrates the file forward if it is older. Sign-ins and AI connections
made after the backup are gone: those apps and MCP clients must sign in again. Devices upload their own unsynced
changes again after the restore (see [`ARCHITECTURE.md`](ARCHITECTURE.md)).

## Example manifest

A minimal Kubernetes template. Replace `quest.example.com`, `<owner>`, the image tag and the ingress settings with
your own; create the secret out of band.

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: quest-data
  namespace: quest
spec:
  accessModes: [ReadWriteOnce]
  resources:
    requests:
      storage: 1Gi
---
# Create out of band, never commit in plain text:
# QUEST_OIDC_ISSUER, QUEST_OIDC_CLIENT_ID, QUEST_OIDC_CLIENT_SECRET, QUEST_AI_BASE_URL, QUEST_AI_API_KEY, QUEST_AI_MODEL
apiVersion: v1
kind: Secret
metadata:
  name: quest-server
  namespace: quest
type: Opaque
stringData: {}
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: quest-server
  namespace: quest
spec:
  replicas: 1
  strategy:
    type: Recreate
  selector:
    matchLabels: { app: quest-server }
  template:
    metadata:
      labels: { app: quest-server }
    spec:
      securityContext:
        runAsNonRoot: true
        runAsUser: 65532
        runAsGroup: 65532
        fsGroup: 65532
        seccompProfile: { type: RuntimeDefault }
      containers:
        - name: quest-server
          image: ghcr.io/<owner>/quest-server:<version>
          ports:
            - { name: http, containerPort: 8787 }
          env:
            - { name: QUEST_PUBLIC_URL, value: "https://quest.example.com" }
            - { name: QUEST_TIME_ZONE, value: "UTC" }
            # Behind a proxy that appends the client address to X-Forwarded-For:
            - { name: QUEST_TRUST_FORWARDED_FOR, value: "true" }
          envFrom:
            - secretRef: { name: quest-server }
          volumeMounts:
            - { name: data, mountPath: /data }
            - { name: tmp, mountPath: /tmp }
          securityContext:
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: true
            capabilities: { drop: [ALL] }
          readinessProbe:
            httpGet: { path: /health/ready, port: http }
            periodSeconds: 10
          livenessProbe:
            httpGet: { path: /health/live, port: http }
            periodSeconds: 20
            failureThreshold: 3
          resources:
            requests: { cpu: 50m, memory: 64Mi }
            limits: { memory: 256Mi }
      volumes:
        - name: data
          persistentVolumeClaim: { claimName: quest-data }
        - name: tmp
          emptyDir: {}
---
apiVersion: v1
kind: Service
metadata:
  name: quest-server
  namespace: quest
spec:
  selector: { app: quest-server }
  ports:
    - { name: http, port: 80, targetPort: http }
---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: quest-server
  namespace: quest
spec:
  ingressClassName: <your-ingress-class>
  tls:
    - hosts: [quest.example.com]
      secretName: quest-example-com-tls
  rules:
    - host: quest.example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: quest-server
                port: { name: http }
```

After the first rollout: open `https://quest.example.com/health/ready` from inside the network, then sign in with the
desktop app.
