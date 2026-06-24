FROM rust:1.96.0-trixie@sha256:e7336b1e0bb2290b0d7bfd3ce1237bf11e5c2ae937ee3e250e6554b98338bea6 AS builder

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
    # renovate: repology=debian_13/cmake
    cmake=3.31.6-2 \
    # renovate: repology=debian_13/libasound2-dev
    libasound2-dev=1.2.14-1 \
    # renovate: repology=debian_13/libonig-dev
    libonig-dev=6.9.9-1+b1 \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/tranzistorak
COPY . .

RUN cargo install --locked --path .

FROM debian:trixie-20260623@sha256:d07d1b51c39f51188e60be9b64e6bf769fa94e187f092bc32b91305cfa34ba5a AS runner

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
    # renovate: repology=debian_13/libasound2-dev
    libasound2-dev=1.2.14-1 \
    # renovate: repology=debian_13/libonig-dev
    libonig-dev=6.9.9-1+b1 \
    # renovate: repology=debian_13/libopus-dev
    libopus-dev=1.5.2-2 \
    # renovate: repology=debian_13/pipx
    pipx=1.7.1-1 \
 && rm -rf /var/lib/apt/lists/* \
 && useradd -m -u 1000 botuser \
 && mkdir -p /srv/bot/logs /srv/bot/rusty_pipe_storage \
 && chown -R botuser:botuser /srv/bot

COPY --from=builder /usr/local/cargo/bin/tranzistorak /srv/bot/tranzistorak

USER botuser

# Unpinned in order to have the latest version.
# hadolint ignore=DL3013
RUN pipx install yt-dlp
ENV PATH="/home/botuser/.local/bin:${PATH}"

#checkov:skip=CKV_DOCKER_2: No healthcheck.

WORKDIR /srv/bot
CMD ["./tranzistorak"]
