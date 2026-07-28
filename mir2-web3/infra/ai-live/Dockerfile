FROM debian:12-slim

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        chromium \
        curl \
        dbus-x11 \
        dumb-init \
        ffmpeg \
        fonts-noto-cjk \
        fonts-noto-color-emoji \
        pulseaudio \
        pulseaudio-utils \
        xvfb \
    && rm -rf /var/lib/apt/lists/*

COPY infra/ai-live/entrypoint.sh /usr/local/bin/ai-live-entrypoint
COPY infra/ai-live/healthcheck.sh /usr/local/bin/ai-live-healthcheck
RUN chmod 0755 /usr/local/bin/ai-live-entrypoint /usr/local/bin/ai-live-healthcheck \
    && mkdir -p /output /run/ai-live

HEALTHCHECK --interval=10s --timeout=4s --start-period=30s --retries=4 \
  CMD ["/usr/local/bin/ai-live-healthcheck"]

ENTRYPOINT ["/usr/bin/dumb-init", "--", "/usr/local/bin/ai-live-entrypoint"]
