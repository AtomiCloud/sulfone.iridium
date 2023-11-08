FROM rust:1.72.0

ENV CROSS_DOCKER_IN_DOCKER=true

RUN cargo install cross

WORKDIR /app