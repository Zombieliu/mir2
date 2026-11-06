# syntax=docker/dockerfile:1.7

ARG NODE_IMAGE=node:22.18.0-bookworm-slim@sha256:752ea8a2f758c34002a0461bd9f1cee4f9a3c36d48494586f60ffce1fc708e0e
FROM ${NODE_IMAGE}

ARG RUST_VERSION=1.89.0
ARG BEVY_RUNTIME_RUST_VERSION=1.95.0
ARG WASM_BINDGEN_VERSION=0.2.118
ARG NPM_VERSION=11.13.0
ARG GH_VERSION=2.96.0
ARG MIR2_DEVELOPER_IMAGE_REVISION=unknown

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/home/node/.cargo \
    RUSTUP_TOOLCHAIN=${RUST_VERSION} \
    MIR2_BEVY_RUNTIME_RUST_TOOLCHAIN=${BEVY_RUNTIME_RUST_VERSION} \
    PATH=/usr/local/cargo/bin:/home/node/.cargo/bin:${PATH}

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        git \
        gosu \
        gzip \
        jq \
        libssl-dev \
        pkg-config \
        procps \
        tar \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /usr/local/cargo /usr/local/rustup /home/node/.cargo \
    && CARGO_HOME=/usr/local/cargo \
       curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
       | CARGO_HOME=/usr/local/cargo sh -s -- \
           -y \
           --default-toolchain "${RUST_VERSION}" \
           --profile minimal \
    && CARGO_HOME=/usr/local/cargo rustup component add rustfmt \
    && CARGO_HOME=/usr/local/cargo rustup target add wasm32-unknown-unknown \
    && CARGO_HOME=/usr/local/cargo rustup toolchain install \
         "${BEVY_RUNTIME_RUST_VERSION}" \
         --profile minimal \
         --component rustfmt \
         --target wasm32-unknown-unknown \
    && CARGO_HOME=/usr/local/cargo \
       RUSTUP_TOOLCHAIN="${BEVY_RUNTIME_RUST_VERSION}" \
       cargo install \
         wasm-bindgen-cli \
         --version "${WASM_BINDGEN_VERSION}" \
         --locked \
    && npm install --global "npm@${NPM_VERSION}" \
    && printf '%s\n' \
         'export PATH="/usr/local/cargo/bin:/home/node/.cargo/bin:${PATH}"' \
         > /etc/profile.d/mir2-rust-path.sh \
    && chown -R node:node /home/node

RUN architecture="$(dpkg --print-architecture)" \
    && case "${architecture}" in \
         amd64) gh_sha256="83d5c2ccad5498f58bf6368acb1ab32588cf43ab3a4b1c301bf36328b1c8bd60" ;; \
         arm64) gh_sha256="06f86ec7103d41993b76cd78072f43595c34aaa56506d971d9860e67140bf909" ;; \
         *) echo "Unsupported GitHub CLI architecture: ${architecture}" >&2; exit 1 ;; \
       esac \
    && gh_archive="gh_${GH_VERSION}_linux_${architecture}.tar.gz" \
    && curl --fail --location --silent --show-error \
         "https://github.com/cli/cli/releases/download/v${GH_VERSION}/${gh_archive}" \
         --output "/tmp/${gh_archive}" \
    && printf '%s  %s\n' "${gh_sha256}" "/tmp/${gh_archive}" | sha256sum --check --strict \
    && tar -xzf "/tmp/${gh_archive}" -C /tmp \
    && install -m 0755 "/tmp/gh_${GH_VERSION}_linux_${architecture}/bin/gh" /usr/local/bin/gh \
    && rm -rf "/tmp/${gh_archive}" "/tmp/gh_${GH_VERSION}_linux_${architecture}"

COPY developer-entrypoint.sh /usr/local/bin/mir2-developer-entrypoint
COPY developer-asset-fetch.sh /usr/local/bin/mir2-developer-asset-fetch
RUN chmod 0755 \
      /usr/local/bin/mir2-developer-entrypoint \
      /usr/local/bin/mir2-developer-asset-fetch

WORKDIR /workspace/mir2-web3

LABEL org.opencontainers.image.source="https://github.com/Zombieliu/mir2" \
      org.opencontainers.image.revision="${MIR2_DEVELOPER_IMAGE_REVISION}" \
      org.opencontainers.image.description="Pinned Mir2 Web developer toolchain"

ENTRYPOINT ["/usr/local/bin/mir2-developer-entrypoint"]
CMD ["sleep", "infinity"]
