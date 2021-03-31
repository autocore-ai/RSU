FROM rust:latest as builder

WORKDIR /root/rsu

COPY . .

ARG BUILD_ARGS

RUN cargo build --all ${BUILD_ARGS}

FROM rust:slim

ARG REPO

LABEL org.opencontainers.image.source ${REPO}

COPY --from=builder /root/rsu/target/**/rsu /usr/local/bin/

COPY --from=builder /root/rsu/target/**/*.so /usr/local/bin/

COPY --from=builder /root/rsu/config /usr/local/bin/config

CMD ["rsu"]
