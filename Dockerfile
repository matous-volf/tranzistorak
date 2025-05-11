# Using Bullseye because of the yt-dlp version.

# bullseye
FROM rust@sha256:b11e1edfad909f1df0b6e7c2df2ace12b5e76879a0da4c5f0b3fd6d239f59f75 AS builder

RUN apt-get update \
 && apt-get install -y cmake=3.18.4-2+deb11u1

WORKDIR /usr/src/tranzistorak
COPY . .

RUN cargo install --path .

# bullseye-slim
FROM debian@sha256:33b7c2e071c29e618182ec872c471f39d2dde3d8904d95f5b7a61acf3a592e7b AS runner

RUN apt-get update

RUN apt-get install -y libopus-dev=1.3.1-0.1 python3-pip=20.3.4-4+deb11u1 \
 && python3 -m pip install --no-cache-dir -U yt-dlp==2025.4.30

COPY --from=builder /usr/local/cargo/bin/tranzistorak /usr/local/bin/tranzistorak
COPY ./.env /usr/local/bin/.env

WORKDIR /usr/local/bin
CMD ["tranzistorak"]
