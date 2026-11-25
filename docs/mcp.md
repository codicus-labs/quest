# MCP server (Phase 9, sign-in and scopes Phase 11)

`quest-server` serves an MCP server at `/mcp` (Streamable HTTP, stateless, JSON responses; rmcp 3.11). AI clients such
as Claude Code, Claude Desktop and Claude.ai read and change the account's quests, tasks and time with it. It is an
adapter only: every write is the same `quest_domain::Command` the apps send, planned all-or-nothing
(`quest_domain::plan_all`), stored like a sync push and broadcast, so open apps see the change live.

## Setup

| Variable | Default | |
|---|---|---|
| `QUEST_PUBLIC_URL` | – | The https origin clients reach the server at, e.g. `https://quest.example.com` (no path). OAuth issuer; the resource is `{origin}/mcp`. |
| `QUEST_OIDC_ISSUER`, `QUEST_OIDC_CLIENT_ID`, `QUEST_OIDC_CLIENT_SECRET` | – | The identity provider users sign in at (required; see [`auth.md`](auth.md)). |
| `QUEST_TIME_ZONE` | `UTC` | Zone for tool calls that name none (tools take `time_zone`; the server instructions ask models to pass it). |
| `QUEST_MCP_DISABLED` | `false` | Switches `/mcp` off (the OAuth endpoints stay: the apps sign in through them). |
| `QUEST_TRUST_FORWARDED_FOR` | `false` | Rate-limit OAuth by the last `X-Forwarded-For` entry (behind your proxy only). |
| `QUEST_CLIENT_IP_HEADER` | – | Rate-limit OAuth by the one address in this header, e.g. `x-real-ip` (behind that edge only). |

Without `QUEST_PUBLIC_URL`, MCP runs only when the server listens on loopback, at `http://<bind>` (local use, tests).
**HTTPS is required** for anything but localhost: put a TLS reverse proxy in front and set `QUEST_PUBLIC_URL` to its
origin; `http://` public URLs are refused at start. `/mcp` only accepts `Host` headers of the public host or
loopback (DNS-rebinding protection).

```sh
mise run oidc-fake    # local identity provider (users anna, ben) on 127.0.0.1:8792
QUEST_BIND=127.0.0.1:8799 QUEST_DATABASE_URL=sqlite:///tmp/quest-mcp.sqlite QUEST_TIME_ZONE=Europe/Berlin \
QUEST_OIDC_ISSUER=http://127.0.0.1:8792 QUEST_OIDC_CLIENT_ID=quest-server QUEST_OIDC_CLIENT_SECRET=quest-dev-secret \
  cargo run -p quest-server
# MCP: http://127.0.0.1:8799/mcp; headless try-out: cargo run -p quest-server --example mcp_client
```

## Connecting clients

Only register clients yourself; nothing registers automatically.

- **Claude Code:** `claude mcp add --transport http quest https://quest.example.com/mcp` (locally
  `http://127.0.0.1:8799/mcp`). On first use Claude Code opens the browser for sign-in and consent.
- **Claude Desktop:** Settings → Connectors → Add custom connector → URL `https://quest.example.com/mcp`.
- **Claude.ai:** Settings → Connectors → Add custom connector → same URL. Claude.ai connects from Anthropic's
  servers, so the server must be reachable on the internet over HTTPS (not localhost).
- **MCP Inspector** (local test): `npx @modelcontextprotocol/inspector`, transport "Streamable HTTP", URL
  `http://127.0.0.1:8799/mcp`, then "Open Auth Settings" → "Quick OAuth Flow".

The apps list connected clients (grants whose code was exchanged for tokens; abandoned consents are not listed)
under "Connected AI clients" (Android: Settings; desktop: the account dialog, ⌘, or the avatar) and can revoke them
(`GET`/`DELETE /v1/ai/connections`, see `quest_protocol::AiConnections`).

## OAuth flow (MCP authorization spec 2026-07-28)

The server is the protected resource and a minimal authorization server for it.

1. `POST /mcp` without a token → `401` with
   `WWW-Authenticate: Bearer resource_metadata="{origin}/.well-known/oauth-protected-resource/mcp", scope="quest:write"`.
2. Protected resource metadata (RFC 9728) at that URL (also at the root path) names the resource and the issuer.
3. Authorization server metadata (RFC 8414) at `/.well-known/oauth-authorization-server`: endpoints, `S256` only,
   `client_id_metadata_document_supported`, `authorization_response_iss_parameter_supported`.
4. Client identity: a **Client ID Metadata Document** (`client_id` is an https URL; fetched with SSRF protection:
   public addresses only, pinned DNS, no proxy, no redirects, 5 s, 16 KiB, cached by `Cache-Control` 1 min–24 h,
   failures 1 min), or **Dynamic
   Client Registration** at `POST /register` (RFC 7591; public `none` or confidential `client_secret_basic`/`_post`).
   Redirect URIs must be https or loopback http and match exactly; loopback ones match on any port (RFC 8252).
   Private-use URI schemes (`com.example.app:/cb`) are not accepted.
5. `GET /authorize`: authorization code with **PKCE S256 required**, `resource` (RFC 8707) must be this server's
   `/mcp` (`{origin}/v1` is reserved for the Quest apps). Without a browser session the user first signs in at the
   identity provider ([`auth.md`](auth.md)); then the consent page names the client, the redirect host and the
   account, warns when a client can only redirect to this computer, and offers **Read only** (`quest:read`) or
   **Read and write** (`quest:write`), preselected with what the client asked for (read only if it named no Quest
   scope); *Use another account* switches
   the account. The answer carries `code`, `state` and `iss` (RFC 9207). Until the client and redirect URI are verified,
   errors show a page instead of redirecting.
6. `POST /token`: `authorization_code` (verifier, same redirect URI, same client, code ≤ 5 min, once) and
   `refresh_token` with **rotation**. Access tokens live 1 h, refresh tokens 30 days, both bound to the grant's
   resource. A reused code or refresh token revokes the whole grant.
7. `POST /revoke` (RFC 7009) revokes the grant of a token.

Scopes: `quest:read` (reading tools only) and `quest:write` (all tools; includes reading); the old `quest` means
`quest:write`, `offline_access` is accepted. The user's choice on the consent page is the grant's scope; a refresh
cannot widen it. A read-only token lists only the reading tools, and calling another one answers HTTP `403` with
`WWW-Authenticate: Bearer error="insufficient_scope", scope="quest:write", resource_metadata="…"` so the client can
ask for more. Codes and tokens are random 256-bit values, stored only as SHA-256 hashes. MCP tokens (audience
`{origin}/mcp`) do not work at the app routes and app tokens (`{origin}/v1`) do not work at `/mcp`; tokens are never
passed on to other services.

## Tools

All tools take an optional `time_zone` (IANA name). Days are ISO dates (`2026-11-24`), clock times `HH:MM` local,
instants RFC 3339 with the zone's offset. Inputs and outputs have JSON Schemas (outputs as `structuredContent`).

### Workspaces

An account can keep separate workspaces (e.g. work and private), each with its own quests, tasks, time and settings.
Every tool except `list_workspaces` takes an optional `workspace`: the workspace's id or its exact name
(case-insensitive, surrounding spaces ignored). Without it the tool works in the **default workspace**, so
integrations from before workspaces keep working unchanged. Reads see only that workspace, writes create in it, and
ids from another workspace answer `task_not_found` etc. An unknown or deleted workspace, or a name several workspaces
share, answers `workspace_not_found` (never a silent fallback to the default); `list_workspaces` names the live ones.
MCP does not create, rename or delete workspaces (the apps do); deleted ones are not listed.

The default workspace has a fixed id and can be renamed, deleted and restored in the apps like any other, as long as
one live workspace remains. Writes that name no workspace (integrations, older app versions) always mean the default
one. While it is deleted, a tool call without `workspace` answers `workspace_not_found` with a hint to pass
`workspace` (see `list_workspaces`) or to restore the default workspace; nothing is written elsewhere. Data an older
app version still writes then stays in the deleted default workspace and reappears when it is restored.

The timer is one per account, across workspaces: `start_timer` stops a timer running in any workspace,
`get_running_timer` and `stop_timer` ignore `workspace` and report the session together with its `workspace`
(id and title). A session has a `kind`: `task` (work on a task, or other work) or `organizing` (sorting out lists
between tasks; `origin_task` is the task to go back to).

A task has an optional plan date (`planned_on`) and deadline (`due_on`); the plan date is never after the deadline
(the domain rule `quest_domain::set_planned`/`set_due`). Giving one date moves the other along when needed
(planning past the deadline moves the deadline, a deadline before the plan date moves the plan date); giving both
with the plan date after the deadline answers `invalid_input`. `"none"` clears a date. Moving a date later records a
postponement, as in the apps. `create_task` without `planned_on` leaves the task unplanned.

| Tool | What it does | readOnly | destructive | idempotent |
|---|---|---|---|---|
| `list_tasks` | Tasks planned for a day or range (≤ 62 days), incl. planned repeats, or `unplanned` ones | ✓ | | |
| `get_task` | One task | ✓ | | |
| `get_checklist` | A task's checklist items | ✓ | | |
| `list_quests` | Active / completed / all quests | ✓ | | |
| `get_quest` | Quest with its tasks | ✓ | | |
| `search` | Words in quest titles and task titles/descriptions; optionally the archive | ✓ | | |
| `get_week_review` | Weekly review: minutes per day and quest, organizing, other, target, share | ✓ | | |
| `get_running_timer` | The running session, in any workspace, and its workspace | ✓ | | |
| `list_time_entries` | Sessions of days, task or quest | ✓ | | |
| `list_workspaces` | Live workspaces: id, name, `is_default`, quest and task counts, `tracked_hours` | ✓ | | |
| `create_task` | Plan date, deadline, quest, repeat | | – | – |
| `update_task` | Title, description, plan date, deadline, quest, repeat (overwrites) | | ✓ | ✓ |
| `complete_task` / `reopen_task` | Done / open again | | – | ✓ |
| `move_task` | Plan for a day (top/bottom) or unplan | | – | – |
| `archive_task` | Soft delete; `restore` undoes it | | ✓ | ✓ |
| `restore` | Task, quest, checklist item or time entry from the archive | | – | ✓ |
| `create_quest` / `update_quest` | Title, color, status | | – / ✓ | – / ✓ |
| `complete_quest` | Quest done (the apps show it under archived quests) | | – | ✓ |
| `add_checklist_item` / `check_checklist_item` / `rename_checklist_item` / `delete_checklist_item` | A task's checklist; each answers the whole checklist | | – / – / ✓ / ✓ | – / ✓ / ✓ / ✓ |
| `start_timer` / `stop_timer` | Track a task or organizing now / stop | | – | – / ✓ |
| `log_work` | Finished session for an existing or new (done) task | | – | – |
| `capture_text` | AI capture in the call's workspace (its quests and tasks as context): proposal, or with `apply: true` creates it all or nothing. Only when AI is configured (`QUEST_AI_*`) and the account turned AI features on in the app settings (otherwise hidden; a call answers `ai_disabled`); same rate limit, validation and commands as the apps | | – | – |

"Destructive" marks tools that overwrite or remove what the user wrote; completing, moving and timers are reversible
state changes. All tools are closed-world except `capture_text`, which calls the configured AI provider.

Planned repeats (`planned_repeat: true`) have stable ids; editing one saves it first, as in the apps' plan
(`Command::EditPlan`). A repeat set on an occurrence applies to its series.

### Errors

Tool errors come back as a result with `isError: true` and the text `{"code": "...", "message": "..."}`. Codes:
`invalid_input`, `task_not_found`, `quest_not_found`, `workspace_not_found`, `checklist_item_not_found`, `entry_not_found`, `not_in_archive`,
`invalid_time_range`, `invalid_repeat_rule`, `empty_title`, `ai_not_configured`, `ai_disabled`, `insufficient_scope`, `ai_unavailable`,
`ai_invalid_response`, `rate_limited`, `text_too_long`, `proposal_not_applicable`, `internal`. Messages say what to do
next (e.g. "find ids with list_tasks or search").

### Examples

"What's on today?" → `list_tasks {"time_zone": "Europe/Berlin"}`.
"Write the offer tomorrow, due Friday, in Website relaunch" → `search {"query": "relaunch"}`, then
`create_task {"title": "Write offer", "planned_on": "2026-11-25", "due_on": "2026-11-27", "quest_id": "…"}`.
"I worked on the concept from 9 to 11" → `log_work {"new_task_title": "Concept", "start": "09:00", "end": "11:00"}`.

## Security and privacy

- HTTPS outside localhost; HSTS/TLS are the reverse proxy's job.
- Logs: per tool call only tool name, duration and error code (`mcp tool call`); OAuth logs grant/client ids and
  scopes, never tokens, codes, ID tokens or tool content. `capture_text` adds no own log line.
- The consent page is `no-store`, not frameable (`X-Frame-Options: DENY`, CSP `frame-ancestors 'none'`) and
  sends no referrer.
- The consent form carries a CSRF token derived from the browser session (cookie `quest_session`, HttpOnly,
  SameSite=Lax, Secure on https); sign-ins in progress are bound to the browser (see [`auth.md`](auth.md)).
- Rate limits per client address, in memory (reset on restart): `POST /register` 20 per hour, `GET /authorize`
  30 per minute, `POST /authorize` 30 per 15 minutes, `POST /token` 60 per minute, `GET /auth/callback` 20 per
  minute; sliding windows, only accepted requests count. Over the limit: `429` with `Retry-After`; `/register` and
  `/token` answer `{"error": "rate_limited", …}`, the browser endpoints a page (no redirect to the client). The address is the TCP peer. Behind a reverse proxy set
  `QUEST_TRUST_FORWARDED_FOR=true` (`ClientAddress::LastForwardedFor`), then the last `X-Forwarded-For` entry counts,
  or behind an edge that sets one address per request `QUEST_CLIENT_IP_HEADER=<header>`
  (`ClientAddress::Header`); only when every request comes through the proxy and it sets that header, otherwise
  clients pick their own address.
  Off by default, so behind a proxy without it all clients share one budget. IPv6 counts per /64. Limits are
  configurable in `AuthConfig::rate_limits`.
- CORS is open on `/mcp` and the OAuth JSON endpoints (browser clients such as the Inspector); tokens travel in the
  `Authorization` header, never cookies, so this does not enable CSRF. `/authorize` has no CORS.

## Tests

`cargo test -p quest-server --test oauth` (discovery, DCR, CIMD via a fake document source, sign-in at the fake
identity provider, consent with read-only choice and `insufficient_scope`, CSRF, denial, PKCE, wrong verifier, wrong
redirect URI, expired and replayed codes, refresh rotation and reuse, no scope widening, revocation from the apps and at
`/revoke`, expired tokens, wrong audience, app tokens refused and vice versa, rate limits) and `--test mcp` (an rmcp
client with a bearer token: annotations and schemas, every tool once successful and once with a validation error, AI
capture with a fake provider and the AI opt-in, read-only tool list, workspace scoping, a change arriving live at a
second device). Pure decisions are unit-tested in `src/mcp/*_tests.rs` and
`src/oauth/policy_tests.rs` and `src/oauth/rate_limit_tests.rs`.

## Open

- Resources and prompts are not offered (optional in the task).
- Rate limits count per address only: `QUEST_TRUST_FORWARDED_FOR` trusts the header from any peer (no list of
  trusted proxy addresses), and the number of tracked addresses has no cap (entries leave with their window, at most
  an hour). Password guessing is the identity provider's concern now.
- DCR is open; unused clients are not cleaned up.
- Private-use redirect schemes (RFC 8252 §7.1) are refused for MCP clients; only the Quest Android app has one
  (built in). Loopback and https work.
- Each tool call loads the account's change history and rebuilds the state (writes under the account's sync lock),
  like a sync pull; fine for personal accounts, a cached per-account store would help large histories.
- The "edit a planned repeat" wrapping (`Command::EditPlan`) exists in desktop, mobile and MCP (`mcp/write.rs`
  `plan_edit`); a shared helper in `quest-view` would remove the copies.
- `capture_text` writes only the tool line; the per-request `ai capture` log line (user, tokens) is for the apps'
  route.
