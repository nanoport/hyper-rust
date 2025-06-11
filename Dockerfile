# Dockerfile for Hyperdrive Rust
FROM rust:1.70 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

# Build the application
RUN cargo build --release

# Runtime image
FROM ubuntu:22.04

# Install dependencies
RUN apt-get update && apt-get install -y \
    curl \
    iptables \
    iproute2 \
    sudo \
    && rm -rf /var/lib/apt/lists/*

# Install Firecracker
RUN curl -LOJ https://github.com/firecracker-microvm/firecracker/releases/download/v1.4.0/firecracker-v1.4.0-x86_64.tgz \
    && tar -xzf firecracker-v1.4.0-x86_64.tgz \
    && mv release-v1.4.0-x86_64/firecracker-v1.4.0-x86_64 /usr/bin/firecracker \
    && chmod +x /usr/bin/firecracker \
    && rm -rf firecracker-v1.4.0-x86_64.tgz release-v1.4.0-x86_64

# Copy binary
COPY --from=builder /app/target/release/hyperdrive-rust /usr/local/bin/hyperdrive-rust

# Create directories
RUN mkdir -p /opt/firecracker /tmp/hyperdrive-rust-vms

# Copy kernel and rootfs (you'll need to provide these)
# COPY vmlinux.bin /opt/firecracker/
# COPY rootfs.ext4 /opt/firecracker/
# COPY v8-host /opt/firecracker/

EXPOSE 8090

CMD ["/usr/local/bin/hyperdrive-rust"]