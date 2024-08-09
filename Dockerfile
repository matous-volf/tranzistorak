FROM rust:1.80-bookworm AS builder

RUN apt-get update
RUN apt-get install -y cmake

WORKDIR /usr/src/tranzistorak
COPY . .

RUN cargo install --path .

FROM debian:bookworm-slim AS runner

RUN apt-get update

RUN apt-get install -y libopus-dev ffmpeg libcurl4 openssl yt-dlp

COPY --from=builder /usr/local/cargo/bin/tranzistorak /usr/local/bin/tranzistorak
COPY ./.env /usr/local/bin/.env

WORKDIR /usr/local/bin
CMD ["tranzistorak"]
