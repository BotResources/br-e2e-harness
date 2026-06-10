# Runtime-only image for oidc-test-idp — TEST FIXTURE ONLY.
#
# The binary is compiled OUTSIDE Docker (by CI's cargo-zigbuild, static musl)
# and copied in. No RUN instructions: keeps the Dockerfile cross-arch
# buildable without QEMU emulation. No ca-certificates needed — the fixture
# makes no outbound calls.
#
# Build args:
#   BIN_PATH — path to the pre-compiled binary
#
# ⚠️ This image signs any requested token and has no authentication on its
# admin surface. Never deploy it outside an isolated test network.

FROM debian:bookworm-slim

ARG BIN_PATH=target/release/oidc-test-idp

COPY ${BIN_PATH} /usr/local/bin/oidc-test-idp

WORKDIR /app
EXPOSE 9100
ENTRYPOINT ["/usr/local/bin/oidc-test-idp"]
