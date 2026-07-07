FROM rust:1.96.1-alpine3.24@sha256:a41f7740f8b45d45795624eec13a8b42263cc700f19f7e4e86e04d3dda08a479 AS builder

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
    openssl-dev=3.5.7-r0

WORKDIR /usr/src/tranzistorak
COPY . .

ENV RUSTFLAGS="-C target-feature=-crt-static"
ENV CXXFLAGS="-D_LARGEFILE64_SOURCE -Dstat64=stat -Dfstat64=fstat"
RUN cargo install --locked --path .

FROM alpine:3.24.1@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b AS runner

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
    openssl=3.5.7-r0 \
    # renovate: repology=alpine_3_24/opus
    opus=1.6.1-r0 \
    # renovate: repology=alpine_3_24/yt-dlp
    yt-dlp=2026.07.04-r0 \
 && adduser -D -u 1000 botuser \
 && mkdir -p /srv/bot/logs /srv/bot/rusty_pipe_storage \
 && chown -R botuser:botuser /srv/bot

COPY --from=builder /usr/local/cargo/bin/tranzistorak /srv/bot/tranzistorak

USER botuser

#checkov:skip=CKV_DOCKER_2: No healthcheck.

WORKDIR /srv/bot
CMD ["./tranzistorak"]
