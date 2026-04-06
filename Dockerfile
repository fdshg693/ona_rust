# Build stage
FROM rust:1.82-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /app

# Cache dependencies separately from source
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "fn main() {}" > src/api_main.rs \
    && cargo build --release --bin todo-server \
    && rm -rf src

# Build the actual binary
COPY src ./src
# Touch to force rebuild after dummy build above
RUN touch src/main.rs src/api_main.rs \
    && cargo build --release --bin todo-server

# Runtime stage
FROM alpine:3.20

RUN apk add --no-cache ca-certificates wget

# Non-root user — UID/GID 1000 must match the EFS AccessPoint posixUser
RUN addgroup -S -g 1000 app && adduser -S -u 1000 -G app app

WORKDIR /app

COPY --from=builder /app/target/release/todo-server ./todo-server

# EFS mount point for SQLite DB
RUN mkdir -p /data && chown app:app /data

USER app

ENV PORT=3000
ENV HOME=/data

EXPOSE 3000

CMD ["./todo-server"]
