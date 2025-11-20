# This file is a generic Rust builder for AlmaLinux
# You can swap out args with your preferences

ARG BASE="almalinux:9.6"
ARG BIN="mystbot"
ARG BUILD_DEPS="ffmpeg-devel openssl-devel clang"
ARG DEPS="ffmpeg openssl"
ARG RUST_VERSION="1.89.0"

FROM $BASE AS builder

ARG BIN
ARG BUILD_DEPS
ARG RUST_VERSION

ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal --default-toolchain $RUST_VERSION --default-host x86_64-unknown-linux-gnu

RUN dnf install -y yum-utils && \
    dnf config-manager --set-enabled crb && \
    dnf install -y --nogpgcheck https://dl.fedoraproject.org/pub/epel/epel-release-latest-$(rpm -E %rhel).noarch.rpm && \
    dnf install -y --nogpgcheck https://mirrors.rpmfusion.org/free/el/rpmfusion-free-release-$(rpm -E %rhel).noarch.rpm https://mirrors.rpmfusion.org/nonfree/el/rpmfusion-nonfree-release-$(rpm -E %rhel).noarch.rpm && \
    dnf install -y gcc $BUILD_DEPS && \
    dnf clean all

WORKDIR /app
COPY . .
RUN \
    --mount=type=cache,target=$CARGO_HOME/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release && cp target/release/$BIN .

FROM $BASE

ARG DEPS
ARG BIN
ENV BIN=$BIN

RUN dnf install -y yum-utils && \
    dnf config-manager --set-enabled crb && \
    dnf install -y --nogpgcheck https://dl.fedoraproject.org/pub/epel/epel-release-latest-$(rpm -E %rhel).noarch.rpm && \
    dnf install -y --nogpgcheck https://mirrors.rpmfusion.org/free/el/rpmfusion-free-release-$(rpm -E %rhel).noarch.rpm https://mirrors.rpmfusion.org/nonfree/el/rpmfusion-nonfree-release-$(rpm -E %rhel).noarch.rpm && \
    dnf install -y $DEPS && \
    dnf clean all

WORKDIR /data
RUN mkdir /app
COPY --from=builder /app/$BIN /app

CMD ["/bin/sh", "-c", "/app/$BIN"]
