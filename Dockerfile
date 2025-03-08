# Using Bullseye because of the yt-dlp version.

# bullseye
FROM rust@sha256:b11e1edfad909f1df0b6e7c2df2ace12b5e76879a0da4c5f0b3fd6d239f59f75 AS builder

RUN apt-get update
RUN apt-get install -y cmake

WORKDIR /usr/src/tranzistorak
COPY . .

RUN cargo install --path .

# bullseye-slim
FROM debian@sha256:33b7c2e071c29e618182ec872c471f39d2dde3d8904d95f5b7a61acf3a592e7b AS runner

RUN apt-get update

RUN apt-get install -y python3-pip
RUN python3 -m pip install -U yt-dlp

RUN apt-get install -y libopus-dev

COPY --from=builder /usr/local/cargo/bin/tranzistorak /usr/local/bin/tranzistorak
COPY ./.env /usr/local/bin/.env

WORKDIR /usr/local/bin
CMD ["tranzistorak"]
