FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

# System libraries and build tooling.
# - build-essential/pkg-config: compile Rust crates and the PyO3 extension.
# - python3.12 + libpython3.12-dev: build/link the sts_sim (py_sts) extension.
# - libsqlite3-dev: needed if the full workspace (which includes sts_live) is built.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates curl git sudo \
        build-essential pkg-config \
        python3.12 python3.12-dev python3.12-venv libpython3.12-dev \
        libsqlite3-dev \
        xz-utils unzip procps \
    && rm -rf /var/lib/apt/lists/*

# ubuntu:24.04 ships a default non-root 'ubuntu' user (uid 1000); give it sudo.
RUN echo 'ubuntu ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/ubuntu \
    && chmod 0440 /etc/sudoers.d/ubuntu

# Shared Rust toolchain location, usable by both the build-time install and the
# agent at runtime. Stable is required (>= 1.87) because the repo commits no
# Cargo.lock and pulls edition2024 transitive deps that older toolchains reject.
ENV RUSTUP_HOME=/opt/rust/rustup \
    CARGO_HOME=/opt/rust/cargo \
    PATH=/opt/rust/cargo/bin:/home/ubuntu/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain stable --profile minimal \
    && rustup component add rustfmt clippy \
    && chmod -R a+rwX /opt/rust

# Make the toolchains discoverable in login shells too.
RUN printf '%s\n' \
      'export RUSTUP_HOME=/opt/rust/rustup' \
      'export CARGO_HOME=/opt/rust/cargo' \
      'export PATH=/opt/rust/cargo/bin:$HOME/.local/bin:$PATH' \
      > /etc/profile.d/10-sts-toolchains.sh \
    && chmod 0644 /etc/profile.d/10-sts-toolchains.sh

USER ubuntu

# uv drives the Python/PyO3 build and provides uvx for the HF corpus download.
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
