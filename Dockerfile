FROM rust:1.99.1-alpine3.24@sha256:dc578ff3ca62ef489ad9ae743f477b94d98739c3092d0023490ecd417c2eebea AS builder

RUN apk add --no-cache \
    # renovate: repology=alpine_3_24/alsa-lib-dev
    alsa-lib-dev=1.2.15.3-r0 \
    # renovate: repology=alpine_3_24/cmake
    cmake=4.2.3-r0 \
    # renovate: repology=alpine_3_24/build-base
    build-base=0.5-r4 \
    # renovate: repology=alpine_3_24/g++
    g++=15.2.0-r5 \
    # renovate: repology=alpine_3_24/oniguruma-dev
    oniguruma-dev=6.9.10-r0 \
    # renovate: repology=alpine_3_24/openssl-dev
    openssl-dev=3.5.9-r0

WORKDIR /usr/src/tranzistorak
COPY . .

ENV RUSTFLAGS="-C target-feature=-crt-static"
ENV CXXFLAGS="-D_LARGEFILE64_SOURCE -Dstat64=stat -Dfstat64=fstat"
RUN cargo install --locked --path .

FROM alpine:3.24.2@sha256:294b683cb724975bec92580e1e685676bd4b50bda910ddb8c51d4cabeaec77e6 AS runner

RUN apk add --no-cache \
    # renovate: repology=alpine_3_24/alsa-lib
    alsa-lib=1.2.15.3-r0 \
    # renovate: repology=alpine_3_24/libgcc
    libgcc=15.2.0-r5 \
    # renovate: repology=alpine_3_24/libstdc++
    libstdc++=15.2.0-r5 \
    # renovate: repology=alpine_3_24/oniguruma
    oniguruma=6.9.10-r0 \
    # renovate: repology=alpine_3_24/openssl
    openssl=3.5.9-r0 \
    # renovate: repology=alpine_3_24/opus
    opus=1.6.1-r0 \
    # renovate: repology=alpine_3_24/yt-dlp
    yt-dlp=2026.08.19-r0 \
 && adduser -D -u 1000 botuser \
 && mkdir -p /srv/bot/logs /srv/bot/rusty_pipe_storage \
 && chown -R botuser:botuser /srv/bot

COPY --from=builder /usr/local/cargo/bin/tranzistorak /srv/bot/tranzistorak

# hadolint ignore=DL3066
USER botuser

#checkov:skip=CKV_DOCKER_2: No healthcheck.

WORKDIR /srv/bot
CMD ["./tranzistorak"]
