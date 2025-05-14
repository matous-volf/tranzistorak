# 1.92.0-bookworm
FROM rust@sha256:e90e846de4124376164ddfbaab4b0774c7bdeef5e738866295e5a90a34a307a2 AS builder

RUN apt-get update \
 && apt-get install -y --no-install-recommends cmake=3.25.1-1 libasound2-dev=1.2.8-1+b1 libonig-dev=6.9.8-1 \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/tranzistorak
COPY . .

RUN cargo install --locked --path .

# bookworm-20250428-slim
FROM debian@sha256:4b50eb66f977b4062683ff434ef18ac191da862dbe966961bc11990cf5791a8d AS runner

RUN apt-get update \
 && apt-get install -y --no-install-recommends libasound2-dev=1.2.8-1+b1 libonig-dev=6.9.8-1 libopus-dev=1.3.1-3 pipx=1.1.0-1 \
 && rm -rf /var/lib/apt/lists/* \
 && useradd -m -u 1000 botuser \
 && mkdir -p /srv/bot/logs /srv/bot/rusty_pipe_storage \
 && chown -R botuser:botuser /srv/bot

COPY --from=builder /usr/local/cargo/bin/tranzistorak /srv/bot/tranzistorak

USER botuser

# hadolint ignore=DL3013
RUN pipx install yt-dlp
ENV PATH="/home/botuser/.local/bin:${PATH}"

#checkov:skip=CKV_DOCKER_2: No healthcheck.

WORKDIR /srv/bot
CMD ["./tranzistorak"]
