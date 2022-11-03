ARG ALPINE_VERSION=3.16

FROM rust:1-alpine${ALPINE_VERSION} AS builder

WORKDIR /usr/src/sb-mirror
RUN apk add --no-cache musl-dev openssl-dev

COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/sb-mirror/target \
    # copy the binary to /usr/local/bin because /usr/src/sb-mirror/target will be empty
    # in the second stage because it wont have the mount
    cargo build --release && cp target/release/sb-mirror /usr/local/bin/sb-mirror





FROM alpine:${ALPINE_VERSION} AS runner

RUN apk add --no-cache nginx

COPY --from=builder /usr/local/bin/sb-mirror /usr/local/bin/sb-mirror

RUN adduser -D -u 1000 sb-mirror

# setup nginx config and fix permissions
COPY nginx.conf /etc/nginx/nginx.conf

# copy entrypoint which starts sb-mirror and the nginx proxy
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh


ENTRYPOINT ["/entrypoint.sh"]