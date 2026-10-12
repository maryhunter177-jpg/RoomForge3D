FROM rust:1-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build --locked --release -p roomforge-api

FROM node:24-bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl libstdc++6 && \
    curl --fail --location --retry 3 https://github.com/KhronosGroup/KTX-Software/releases/download/v4.4.2/KTX-Software-4.4.2-Linux-x86_64.deb -o /tmp/ktx.deb && \
    apt-get install -y --no-install-recommends /tmp/ktx.deb && \
    rm -rf /var/lib/apt/lists/* /tmp/ktx.deb && \
    useradd --system --uid 10001 --create-home roomforge
WORKDIR /app
COPY tools/package.json tools/package-lock.json ./tools/
RUN npm ci --prefix tools --omit=dev && npm cache clean --force
COPY tools/optimize-model.mjs tools/setup-storage.mjs ./tools/
COPY apps/api/docker-entrypoint.sh /usr/local/bin/roomforge-entrypoint
RUN chmod +x /usr/local/bin/roomforge-entrypoint
COPY --from=build /app/target/release/roomforge-api /usr/local/bin/roomforge-api
USER roomforge
EXPOSE 8080
ENTRYPOINT ["roomforge-entrypoint"]
CMD ["roomforge-api"]
