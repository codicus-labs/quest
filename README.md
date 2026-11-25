# Quest

Quest is an offline-first planner and time tracker: long-term **quests**, the **tasks** that move them forward
(with plan date, deadline and checklist), a weekly plan, a focus timer and a weekly review. Everything works without an account;
signed in, devices sync live through a server you host yourself. Written in Rust; the macOS client uses the
[Rune](https://github.com/anomalyco/rune) UI framework. Inspired by [quest.app](https://quest.app).

## Components

| Path | What |
|---|---|
| `crates/quest-app` | macOS client (Rune) |
| `apps/android` | Android client (Jetpack Compose over the shared Rust core, UniFFI) |
| `crates/quest-server` | Sync server (axum, SQLite): sign-in via any OpenID Connect provider, OAuth 2.1 for the apps, MCP server for AI clients, optional AI capture through an OpenAI-compatible API |
| `crates/quest-domain`, `quest-store`, `quest-view`, `quest-capture`, `quest-protocol`, `quest-mobile` | Shared domain model, sync engine, presentation logic and bindings |
| `crates/quest-oidc-fake` | Fake OIDC provider for tests and local development |

Architecture: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). Sign-in: [`docs/auth.md`](docs/auth.md). MCP:
[`docs/mcp.md`](docs/mcp.md).

## Build and run locally

Tools come from [mise](https://mise.jdx.dev) and `rust-toolchain.toml`.

```sh
mise run oidc-fake                                          # local OIDC provider (users anna, ben), see docs/auth.md
mise run server                                             # 127.0.0.1:8787, SQLite quest-server.sqlite, sign-in at the fake
cargo run -p quest-app -- --demo --data-dir /tmp/quest-a    # instance A with demo data (guest)
cargo run -p quest-app -- --data-dir /tmp/quest-b           # instance B (avatar → Sign in, in the browser)
cargo test --workspace                                      # unit and integration tests
bash crates/quest-app/e2e/run.sh                            # end-to-end through the UI (starts its own fake and server)
mise run package-macos                                      # Quest.app + .dmg in target/package/ (ad-hoc signed)
```

`mise run server` reads an optional `.env` (git-ignored) for the AI capture key. Android: see
[`apps/android/README.md`](apps/android/README.md).

## Host the server yourself

The server ships as a container image (`ghcr.io/<owner>/quest-server`). It needs a volume, an OpenID Connect
provider (e.g. Keycloak, Authelia or Authentik) and TLS in front. Image, configuration, OIDC client, reverse proxy,
backup and restore: [`docs/deployment.md`](docs/deployment.md).

## Releases

- **Android:** signed APKs on the GitHub releases (tags `android-v*`). Install and update them with
  [Obtainium](https://github.com/ImranR98/Obtainium) (*Add app* → this repository's URL), or download the APK and
  install it directly. Details: [`apps/android/README.md`](apps/android/README.md#install-and-updates-obtainium).
- **macOS:** a DMG on the GitHub releases (tags `desktop-v*`, Apple silicon). It is ad-hoc signed, not notarized:
  Gatekeeper blocks the first launch. Right-click Quest.app → *Open*, or allow it under *System Settings → Privacy &
  Security*. Compare the download with its `.sha256` file.

The app asks for your server's URL when you sign in; no server is built in.

## License

[MIT](LICENSE)
