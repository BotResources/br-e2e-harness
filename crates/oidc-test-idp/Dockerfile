FROM debian:bookworm-slim

ARG BIN_PATH=target/release/oidc-test-idp

COPY ${BIN_PATH} /usr/local/bin/oidc-test-idp

WORKDIR /app
EXPOSE 9100
ENTRYPOINT ["/usr/local/bin/oidc-test-idp"]
