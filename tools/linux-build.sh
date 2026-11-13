#!/bin/sh
# Builds the Linux release inside an AlmaLinux 8 container (see
# .github/workflows): glibc 2.28, so the programs run on RHEL 8 and 9,
# Debian 10 and later, Ubuntu 18.04 and later, and on most other
# distributions. Usage: sh tools/linux-build.sh cargo build --release ...
set -e
dnf install -y -q gcc gcc-c++ make pkgconf-pkg-config alsa-lib-devel systemd-devel
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal
. "$HOME/.cargo/env"
"$@"
