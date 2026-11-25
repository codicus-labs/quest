# quest-server

Sync and auth server for Quest (axum, SQLite). The protocol is in `quest-protocol`. Users sign in at an OpenID
Connect provider; the server is the OAuth 2.1 authorization server for the apps and MCP clients
([`docs/auth.md`](../../docs/auth.md)).

```sh
mise run oidc-fake                             # local identity provider, 127.0.0.1:8792 (users anna, ben)
mise run server                                # 127.0.0.1:8787, ./quest-server.sqlite, AI via OpenRouter
cargo run -p quest-server -- --help            # all options; each also has an environment variable
```

| Variable | Default | |
|---|---|---|
| `QUEST_BIND` | `127.0.0.1:8787` | Listen address |
| `QUEST_DATABASE_URL` | `sqlite://quest-server.sqlite` | Created if missing |
| `RUST_LOG` | `info` | Log filter |
| `QUEST_LOG_FORMAT` | `text` | `json`: one JSON object per line (the container image sets it) |
| `QUEST_OIDC_ISSUER` | – | Identity provider (required; https, http only on localhost) |
| `QUEST_OIDC_CLIENT_ID` | – | The server's client there (required) |
| `QUEST_OIDC_CLIENT_SECRET` | – | Its secret (required; hidden in `--help`, never logged) |
| `QUEST_PUBLIC_URL` | – | https origin (OAuth issuer, `{origin}/auth/callback`); required unless the server listens on loopback |
| `QUEST_TIME_ZONE` | `UTC` | Default zone of MCP tool calls |
| `QUEST_MCP_DISABLED` | `false` | Switches `/mcp` off (sign-in stays) |
| `QUEST_TRUST_FORWARDED_FOR` | `false` | Rate-limit sign-in and OAuth by the last `X-Forwarded-For` entry (only behind your proxy) |
| `QUEST_CLIENT_IP_HEADER` | – | Rate-limit sign-in and OAuth by the one address in this header, e.g. `x-real-ip` (only if the proxy in front sets it on every request and overwrites a client's) |
| `QUEST_ANDROID_CERT_SHA256` | – | Android signing certificates (`AB:CD:…`, comma-separated) for `/.well-known/assetlinks.json` |

Container image, health probes (`/health/live`, `/health/ready`), shutdown on SIGTERM, reverse proxy, backup and an
example Kubernetes manifest:
[`docs/deployment.md`](../../docs/deployment.md).

## Sign-in

The apps sign in through the browser: `/authorize` → identity provider → `/auth/callback` → back to the app with a
code, exchanged at `/token` for an access token (1 h, audience `{origin}/v1`) and a refresh token (30 days, rotated).
Sync, `/v1/account` and AI capture take that token; `/auth/logout` signs out of the browser session and at the
provider. Flows, provider setup and local Keycloak (`compose.yaml`): [`docs/auth.md`](../../docs/auth.md).

## MCP server (`/mcp`)

AI clients (Claude Code, Claude Desktop, Claude.ai, MCP Inspector) use the account through MCP tools, authorised with
OAuth 2.1 (PKCE, client metadata documents or dynamic registration, refresh rotation). The tools run the same domain
commands as the apps and sync live. Setup, flow, tools and security: [`docs/mcp.md`](../../docs/mcp.md).

```sh
claude mcp add --transport http quest https://quest.example.com/mcp
```

Sign-in and OAuth are rate-limited per client address (in memory): `POST /register` 20 per hour, `GET /authorize`
30 per minute, `POST /authorize` 30 per 15 minutes, `POST /token` 60 per minute, `GET /auth/callback` 20 per minute;
over the limit `429` with `Retry-After`. Behind a reverse
proxy set `QUEST_TRUST_FORWARDED_FOR=true` so the last `X-Forwarded-For` entry counts, or behind an edge that sets one
address per request `QUEST_CLIENT_IP_HEADER=<header>` (only if every request comes through the proxy or edge); otherwise all
clients behind it share one budget. Details: [`docs/mcp.md`](../../docs/mcp.md#security-and-privacy).

## AI capture (`POST /v1/ai/capture`)

Turns free text (typed or dictated) into a proposal the user reviews in the app. The server calls an
OpenAI-compatible Chat Completions API; the key stays on the server. Without `QUEST_AI_BASE_URL`,
`QUEST_AI_API_KEY` and `QUEST_AI_MODEL` the feature is off and the route answers `503 ai_not_configured`
(the apps then hide the AI action). Setting only some of the three is a startup error. AI features are also off per
account until the user turns them on (`PUT /v1/account/settings`, app settings "AI features"); until then capture
answers `403 ai_disabled` and MCP hides `capture_text`. `GET /v1/account` reports `ai_enabled` and `ai_available`.

| Variable | Default | |
|---|---|---|
| `QUEST_AI_BASE_URL` | – | API root without `/chat/completions` |
| `QUEST_AI_API_KEY` | – | Provider key; never logged, redacted in `Debug` and `--help` |
| `QUEST_AI_MODEL` | – | Model name as the provider spells it |
| `QUEST_AI_TIMEOUT_SECS` | `45` | Whole provider call, including the fallback |
| `QUEST_AI_HTTP_REFERER` | – | OpenRouter: `HTTP-Referer` (app URL, for their stats) |
| `QUEST_AI_APP_TITLE` | – | OpenRouter: `X-Title` (app name) |
| `QUEST_AI_ALLOW_HTTP` | `false` | Allow `http://` to a non-loopback host; loopback (a local mock) is always allowed |
| `QUEST_AI_FALLBACK_MODELS` | – | OpenRouter: models to try in order when the main one fails, comma-separated |
| `QUEST_AI_REASONING_EFFORT` | `none` | OpenRouter: `none`, `minimal`, `low`, `medium`, `high`, or `default` (send nothing, for models without reasoning support) |
| `QUEST_AI_OPENROUTER` | detected | Send OpenRouter's request fields; detected from the host `openrouter.ai`, set it for a proxy in front of OpenRouter |

### Starting with AI (OpenRouter)

The way to run the server with AI capture is the repo's mise task:

```sh
echo 'OPENROUTER_API_KEY=<your-openrouter-key>' >> .env   # once; .env is git-ignored
mise run server                                          # 127.0.0.1:8787, AI via OpenRouter
```

It loads the key from `.env` and sets base URL, model, fallback models and app title (`mise.toml`; the model
choice is explained in [`docs/ai-models.md`](../../docs/ai-models.md)). Other settings from the tables here can be
added as environment variables, e.g. `QUEST_BIND=127.0.0.1:8788 mise run server`. Set the spending limit in
OpenRouter's dashboard.

By hand:

```sh
QUEST_AI_BASE_URL=https://openrouter.ai/api/v1 \
QUEST_AI_API_KEY=<your-openrouter-key> \
QUEST_AI_MODEL=<provider/model> \
QUEST_AI_APP_TITLE=Quest \
cargo run -p quest-server
```

To OpenRouter the server also sends `provider.require_parameters: true` (only providers that support
structured output and the other parameters are picked; without it a provider may ignore `json_schema`
silently), `models` (main model plus `QUEST_AI_FALLBACK_MODELS`) and `reasoning.effort` (default `none`:
capture needs no reasoning, it only adds latency and cost). OpenAI and other APIs never get these fields.

OpenAI:

```sh
QUEST_AI_BASE_URL=https://api.openai.com/v1 \
QUEST_AI_API_KEY=<your-openai-key> \
QUEST_AI_MODEL=<model, e.g. gpt-4.1-mini> \
cargo run -p quest-server
```

The server asks for structured output (`response_format: json_schema`, strict). If the provider rejects
that for the model (HTTP 400 saying `response_format`/`json_schema` is not supported, or OpenRouter's 404
"No endpoints found that can handle the requested parameters"), it retries once with a forced tool call
and, if that works, uses tool calling from then on. A provider that silently ignores
`response_format` still works as long as the model answers with JSON; answers in code fences or with
surrounding prose are accepted. The answer is
validated strictly (`quest_capture::validate`) before it goes to the app.

### Limits

- Per account: 10 requests per minute and 200 per day (in memory, reset on restart). Every accepted
  attempt counts, even if it fails later. Over the limit: `429 rate_limited`.
- Request body at most 512 KiB, text at most 4000 characters (`413 text_too_long`), context at most 50
  quests and 150 tasks with short titles (`400 invalid_request`).
- Errors from the provider: `502`/`503`/`504 ai_unavailable` (unreachable, rate-limited, timed out,
  rejected, e.g. a wrong key); unusable answers: `502 ai_invalid_response`.

Cost control beyond the rate limit (spending caps) belongs in the provider's dashboard.

### Privacy

Sent to the provider per request: the text, the client's local date, time and time zone, the
language, and the compact context the app built: active quests (id, title, description shortened
to 200 characters) and tasks around today (id, title, quest id, day, done). Nothing else, and no
account data (email, user id).

Logged per request: user id, duration, token counts, cost (if the provider reports it), the outcome code (e.g. `ok`, `provider_timeout`)
and for rejections the provider's HTTP status. Never logged: the text, the prompt, the answer, or
provider error bodies (they may echo the input).

### Trying it with the mock provider

`examples/ai_mock.rs` answers every request with the proposal for the example text ("Morgen Angebot
für Müller schreiben … Heute von neun bis elf am Relaunch-Konzept gearbeitet."): a task tomorrow
(assigned to a context quest named "…Relaunch…" if there is one), a new quest "Steuererklärung" with
a task on Friday, and work today from 9 to 11. Nothing leaves the machine.

Run it next to a second server instance, so a server on 8787 stays untouched:

```sh
cargo run -p quest-server --example ai_mock        # http://127.0.0.1:8790/v1

QUEST_BIND=127.0.0.1:8788 \
QUEST_DATABASE_URL=sqlite:///tmp/quest-ai.sqlite \
QUEST_OIDC_ISSUER=http://127.0.0.1:8792 QUEST_OIDC_CLIENT_ID=quest-server QUEST_OIDC_CLIENT_SECRET=quest-dev-secret \
QUEST_AI_BASE_URL=http://127.0.0.1:8790/v1 \
QUEST_AI_API_KEY=mock QUEST_AI_MODEL=mock \
cargo run -p quest-server
```

The Android emulator reaches it at `http://10.0.2.2:8788`. `QUEST_AI_MOCK_BIND` changes the mock's
address.

## Tests

```sh
cargo test -p quest-server
```

`tests/auth.rs` covers the app sign-in against `quest-oidc-fake` (bad ID tokens: nonce, audience, issuer, expiry,
key; state and login CSRF; refresh, revocation, sign-out, account switch; sync only with a valid token),
`tests/oauth.rs` the OAuth flow of MCP clients, `tests/mcp.rs` every tool through an rmcp client,
`tests/migration.rs` migration 0004 on a password-era database.
`tests/ai.rs` runs against a local mock (`tests/ai/mock.rs`) and a fake provider, never a real one.
`live_provider` is ignored; it sends the example text to the provider in `QUEST_AI_*`:
`cargo test -p quest-server --test ai -- --ignored live_provider`.

Model quality is measured with the eval set in `crates/quest-ai-eval` (`mise run ai-eval --model <id>`, paid
calls with synthetic texts); results in [`docs/ai-models.md`](../../docs/ai-models.md).
