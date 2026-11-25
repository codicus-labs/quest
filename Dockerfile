# quest-server: the sync, auth and MCP server. Build: docker build -t quest-server .
# Runtime: distroless cc (glibc, libgcc, CA certificates, no shell or package manager) as the
# nonroot user. The binary needs nothing else: SQLite is bundled, TLS is rustls with ring, and the
# migrations are compiled in. debian-slim would only add a shell and apt we do not use.

FROM rust:1.99.1-slim-trixie AS build
# The image's toolchain; otherwise rustup reads rust-toolchain.toml and fetches clippy and rustfmt.
ENV RUSTUP_TOOLCHAIN=1.99.1
WORKDIR /src
# The whole workspace: Cargo resolves every member (and the patched Rune git sources) against
# Cargo.lock, even for one package. .dockerignore leaves out target/, apps/ and docs/.
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p quest-server \
    && cp target/release/quest-server /usr/local/bin/quest-server
# The volume mount point, owned by the nonroot user (65532) so a fresh volume is writable.
RUN mkdir /data

FROM gcr.io/distroless/cc-debian13:nonroot
COPY --from=build /usr/local/bin/quest-server /usr/local/bin/quest-server
COPY --from=build --chown=65532:65532 /data /data
ENV QUEST_BIND=0.0.0.0:8787 \
    QUEST_DATABASE_URL=sqlite:///data/quest.sqlite \
    QUEST_LOG_FORMAT=json
VOLUME /data
USER 65532:65532
EXPOSE 8787
ENTRYPOINT ["/usr/local/bin/quest-server"]
