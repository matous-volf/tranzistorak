FROM rust:1.69 as builder
WORKDIR /usr/src/tranzistorak
COPY . .

RUN apt-get update
RUN apt-get install -y cmake

RUN cargo install --path .

FROM debian:bullseye-slim

RUN apt-get update

RUN apt-get install -y python3-pip
RUN python3 -m pip install -U yt-dlp

RUN apt-get install -y libopus-dev ffmpeg libcurl4

COPY --from=builder /usr/local/cargo/bin/tranzistorak /usr/local/bin/tranzistorak
CMD ["tranzistorak"]
