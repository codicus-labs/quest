# Sign-in (Phase 11): OpenID Connect

Quest has no passwords of its own. `quest-server` signs users in at an external OpenID Connect provider
(any standard provider, e.g. Keycloak, Authelia or Authentik; locally `quest-oidc-fake` or Keycloak) and is itself the OAuth 2.1 authorization server for
its clients: the Quest apps (first party) and AI clients over MCP. Accounts are identified by `(issuer, subject)` of
the ID token and created at the first sign-in; email and name are only shown.

## Roles and tokens

| | Who | Token audience (RFC 8707) | Scope |
|---|---|---|---|
| Identity provider | your OIDC provider / Keycloak / fake | – | `openid profile email` (server → provider) |
| `quest-desktop` | desktop app, public client | `{origin}/v1` | `quest:write offline_access` |
| `quest-android` | Android app, public client | `{origin}/v1` | `quest:write offline_access` |
| MCP clients | DCR or Client ID Metadata Document | `{origin}/mcp` | `quest:read` or `quest:write` |

`{origin}` is `QUEST_PUBLIC_URL` or, on a loopback bind without it, `http://<bind>`. A token only works at its own
resource: `/v1/*` (sync, account, AI capture, connections) takes `/v1` tokens, `/mcp` takes `/mcp` tokens. Access
tokens live 1 h, refresh tokens 30 days with rotation (a reused refresh token revokes the grant). Codes and tokens are
random 256-bit values stored as SHA-256 hashes.

Scopes: `quest:write` includes reading. The old scope `quest` is still accepted and means `quest:write` (migration
0004 rewrote stored grants). A read-only MCP token sees only reading tools and gets `403` with
`WWW-Authenticate: Bearer error="insufficient_scope", scope="quest:write"` for others. A refresh cannot widen the scope.

## Flows

### App sign-in (desktop and Android)

```
App                    Browser                       quest-server                     Identity provider
 | PKCE + state         |                                |                                  |
 |--open /authorize---->|--GET /authorize?client_id=quest-desktop&resource={origin}/v1-------->|
 |                      |                                | no session: store state, nonce,   |
 |                      |<--303 + cookie quest_login_*---|  PKCE (hashed), the request       |
 |                      |--GET /authorize?…&nonce&code_challenge&prompt?------------------->|
 |                      |                     user signs in at the provider                  |
 |                      |<-------------------303 {origin}/auth/callback?code&state-----------|
 |                      |--GET /auth/callback (cookie)-->|                                  |
 |                      |                                |--POST token (secret, verifier)-->|
 |                      |                                |<--ID token------------------------|
 |                      |                                | check signature (JWKS), iss, aud, |
 |                      |                                | exp, nonce; account (iss, sub)    |
 |                      |<--303 redirect_uri?code&state&iss + cookie quest_session--------- |
 |<--redirect-----------|                                |                                  |
 |--POST /token (code, verifier, resource {origin}/v1)-->|                                  |
 |<--access + refresh token------------------------------|                                  |
```

- Desktop: redirect `http://127.0.0.1:<any port>/callback` (RFC 8252 loopback; the app listens there).
  Android: `app.quest.android:/oauth2redirect` (private-use scheme) in a Custom Tab, in debug and release builds.
  The server can additionally offer a verified App Link (optional, see
  [Android App Links](#android-app-links-optional)).
- **"Continue as …"** (*Continue*, *Cancel*, *Use another account*) comes before the code instead of issuing it
  silently: a stray or forged request cannot get a code, and the user can switch accounts. *Use another account*
  ends the session and sends the user to the provider with `prompt=login`.
  - Desktop: only if the browser already had a Quest session (`quest_session`, 12 h); right after a fresh sign-in the
    code comes without the page (only the app listening on the loopback port receives it).
  - Android with the private-use scheme: **always**, also right after a fresh sign-in. Any installed app can
    register the scheme `app.quest.android:` and start its own authorization with its own PKCE; PKCE does not help
    then, because the attacker holds the verifier. The page makes the user confirm that the Quest app asked.
  - Android with an App Link (optional): like desktop. Only the app signed with a certificate listed in `assetlinks.json`
    receives `https://…/auth/android`, so no other app can claim the redirect.
- Sync (`/v1/sync`) takes the access token in the `Authorization` header when connecting. An open connection ends
  when its token expires (at `expires_at`) or its grant is revoked (`/revoke`, a reused refresh token or code, removal
  under connected clients; at once): the server sends `Error { code: unauthorized }` and closes. The apps refresh and
  reconnect.

### MCP client authorization

As in Phase 9 ([`mcp.md`](mcp.md)), with the password form replaced by the sign-in above: `GET /authorize` without a
session goes to the provider first; then the consent page names the client, the redirect host and the account and
offers **Read only** (`quest:read`) or **Read and write** (`quest:write`), preselected with what the client asked for
(read only if it named no Quest scope: least privilege). *Use another account* switches the account as above. The form carries a CSRF
token derived from the session.

### Sign-out

```
App/Browser --GET /auth/logout?return_to=<first-party redirect>--> quest-server
   quest-server: delete the browser session, clear the cookie
   --303 provider end_session_endpoint?id_token_hint&post_logout_redirect_uri={origin}/auth/logged-out&state=<return_to>
Provider --303--> {origin}/auth/logged-out?state=<return_to> --303--> return_to (or the page "Signed out of Quest")
```

The apps also revoke their refresh token (`POST /revoke`, RFC 7009) and forget it locally. `return_to` is ignored
unless it is a first-party redirect URI. Without an `end_session_endpoint` the provider session stays.

### Login CSRF and replay

A sign-in in progress is stored server-side by the hash of its `state` and bound to the browser that started it by a
`quest_login_<state-hash prefix>` cookie (HttpOnly, SameSite=Lax, path `/auth/callback`, 10 min; one per sign-in, so
parallel sign-ins in one browser do not overwrite each other). The callback takes it once; a callback
in another browser, with an unknown or used `state`, or after 10 min shows an error page and signs nobody in.

## Configuration

| Variable | Default | |
|---|---|---|
| `QUEST_OIDC_ISSUER` | – | Provider issuer (https; http only on localhost). Discovery at `{issuer}/.well-known/openid-configuration` |
| `QUEST_OIDC_CLIENT_ID` | – | The server's client at the provider |
| `QUEST_OIDC_CLIENT_SECRET` | – | Its secret (confidential client); never logged, hidden in `--help` |
| `QUEST_PUBLIC_URL` | – | The https origin, required unless the server listens on loopback |
| `QUEST_TRUST_FORWARDED_FOR` | `false` | Rate limits by the last `X-Forwarded-For` entry (behind your proxy only) |
| `QUEST_CLIENT_IP_HEADER` | – | Rate limits by the one address in this header, for an edge that sets one address per request and overwrites a client's (e.g. `x-real-ip`) |
| `QUEST_ANDROID_CERT_SHA256` | – | Android signing certificates for `assetlinks.json` (`AB:CD:…`, comma-separated); checked at start |

All three `QUEST_OIDC_*` are required; the server refuses to start without them. The redirect URI to register at the
provider is `{origin}/auth/callback`, the post-logout redirect `{origin}/auth/logged-out`. Discovery and keys are
fetched lazily and cached for 10 minutes; an unknown signing key refetches them once.

Rate limits per client address (in memory): `/register` 20 per hour, `GET /authorize` 30 per minute,
`POST /authorize` 30 per 15 minutes, `/token` 60 per minute, `/auth/callback` 20 per minute; then `429` with
`Retry-After`.

## Android App Links (optional)

The Android app signs in through the private-use scheme `app.quest.android:/oauth2redirect` and needs nothing from
the server for that. The server additionally supports a verified App Link (RFC 8252 §7.2) for builds that are made for
one server and signed with a known key:

- Redirect URIs of `quest-android`: always `app.quest.android:/oauth2redirect`; additionally
  `{origin}/auth/android` (`quest_protocol::ANDROID_APP_LINK_PATH`), but only when the origin (`QUEST_PUBLIC_URL`) is
  https, because Android verifies App Links only there. The same two are accepted as `return_to` of `/auth/logout`.
- `GET /.well-known/assetlinks.json` lists `app.quest.android` with the fingerprints of `QUEST_ANDROID_CERT_SHA256`
  (relation `delegate_permission/common.handle_all_urls`; the SHA-256 of the signing certificate, `AB:CD:…`, from
  `keytool -list -v` or `apksigner verify --print-certs`). Without the variable the file is missing (404) and Android
  does not verify the link. It must be reachable over https without redirects.
- `GET /auth/android` is a fallback page: the browser only lands there if Android did not hand the link to the app
  (not verified, app not installed, or the browser did not switch apps without a tap). Its "Open Quest" link
  points at the same URL, so a tap (a user gesture) lets Android open the app with the code. The page never echoes or
  logs the query (it holds the code) and is sent with `Cache-Control: no-store` and `Referrer-Policy: no-referrer`.
- The app only uses the App Link if its build declares the server's origin as its App Link origin (an intent filter
  with `android:autoVerify="true"` for `https://<host>/auth/android`). The published builds declare none and always
  use the private-use scheme ([`mobile.md`](mobile.md)).
- Check the verification on a device: `adb shell pm get-app-links app.quest.android` should show the host as
  `verified`.

## Production provider

1. At your OIDC provider, add a client: name `quest-server`; redirect URI `https://quest.example.com/auth/callback`;
   post-logout redirect URI `https://quest.example.com/auth/logged-out`; a confidential client (the server keeps a
   secret). PKCE may be required: the server always sends `S256`.
2. Copy client ID and secret into the server's environment:

   ```sh
   QUEST_PUBLIC_URL=https://quest.example.com
   QUEST_OIDC_ISSUER=https://id.example.com
   QUEST_OIDC_CLIENT_ID=<client id>
   QUEST_OIDC_CLIENT_SECRET=<secret>
   ```

3. The server asks for the scopes `openid profile email`. Restrict who may sign in (e.g. a user group on the client)
   if needed: every user the provider lets through gets a Quest account.

Full deployment guide: [`deployment.md`](deployment.md).

## Local development

**Fake provider** (no Docker, no passwords; users anna and ben, picked on a page):

```sh
mise run oidc-fake        # http://127.0.0.1:8792, client quest-server / quest-dev-secret
mise run server           # QUEST_OIDC_* point at the fake
```

`quest-oidc-fake --auto-login anna` skips the picker. The crate is also the test helper:
`quest_oidc_fake::app_login(server_url, "anna", AppClient::Desktop)` runs a complete app sign-in without a browser;
`Browser::new(server_url).login_as("anna").authorize(authorize_url, redirect_uri)` runs any app authorization request
to the redirect back to the app (signing in and confirming "Continue as …"); `Browser` with `fault=wrong_nonce|wrong_aud|wrong_issuer|expired|wrong_key` makes the provider issue bad ID tokens.
Never use the fake in production: it signs in anyone it is told to.

**Keycloak** (closer to a real provider): `docker compose up keycloak` imports `keycloak/quest-realm.json` (realm
`quest`, client `quest-server` / `quest-dev-secret`, users `anna`/`anna` and `ben`/`ben`). Keycloak matches redirect
URIs exactly (wildcards only at the end of the path), so the realm lists the callback and post-logout URIs of the
dev servers `127.0.0.1:8787`, `localhost:8787`, `127.0.0.1:8788`, `127.0.0.1:8799` and of `https://quest.example.com`;
for another port add `http://127.0.0.1:<port>/auth/callback` and `…/auth/logged-out` to the client (admin console or
the JSON). Then

```sh
QUEST_OIDC_ISSUER=http://localhost:8091/realms/quest QUEST_OIDC_CLIENT_ID=quest-server \
QUEST_OIDC_CLIENT_SECRET=quest-dev-secret cargo run -p quest-server
```

Admin console: http://localhost:8091 (admin/admin).

## Decisions

- **`openidconnect` crate** (4.x, default features off) for the relying party: it implements discovery, PKCE, the
  code exchange and ID-token validation (JWS signature against the JWKS, `iss`, `aud`/`azp`, `exp`/`iat`, `nonce`)
  with typed metadata, so no security-critical JWT code is written by hand. HTTP goes through the server's own
  `reqwest` client (rustls with the platform verifier, no redirects, 10 s timeout) behind an adapter
  (`src/oidc.rs`, trait `IdentityProvider`), so the provider stays exchangeable and tests can substitute it.
  Alternatives: hand-rolled with `jsonwebtoken` (more code to get right) or `openid` (less maintained).
- **Android: private-use scheme, App Link optional.** The private-use redirect `app.quest.android:/oauth2redirect`
  (RFC 8252 §7.1) works with any server (http, 127.0.0.1, any origin) without a verified domain, so one published
  build fits every server. Another app could claim the scheme, so it always gets the "Continue as …"
  confirmation. An App Link `https://<host>/auth/android` is bound to the signed app by `assetlinks.json` and needs no
  confirmation, but it works only for the one domain a build declares; the server keeps it as an option.
- **The server is the authorization server for the apps** (instead of the apps talking to the provider directly): one
  token model for apps and MCP, revocation and audiences in one place, the provider's client secret stays on the server,
  and the provider can change without an app update.

## Open

- Accounts from before Phase 11 (password accounts) keep their data but have no identity; linking them to an
  identity (e.g. by verified email) is not automatic.
- `GET /auth/logout` can be triggered cross-site (it only signs out; no data changes).
- Rate limits are per address and in memory (see [`mcp.md`](mcp.md#open)).
