FROM rust:latest as builder

WORKDIR /usr/src/rsu

COPY . .

RUN cargo install --path .

ARG REPO

FROM rust:slim

LABEL org.opencontainers.image.source ${REPO}

COPY --from=builder /usr/local/cargo/bin/rsu /usr/local/bin/rsu

CMD ["rsu"]
