# Using bullseye because of the yt-dlp version.

FROM rust:1.80-bullseye AS builder

RUN apt-get update
RUN apt-get install -y cmake

WORKDIR /usr/src/tranzistorak
COPY . .

RUN cargo install --path .

FROM debian:bullseye-slim AS runner

RUN apt-get update

RUN apt-get install -y python3-pip
RUN python3 -m pip install -U yt-dlp

RUN apt-get install -y libopus-dev

COPY --from=builder /usr/local/cargo/bin/tranzistorak /usr/local/bin/tranzistorak
COPY ./.env /usr/local/bin/.env

WORKDIR /usr/local/bin
CMD ["tranzistorak"]
