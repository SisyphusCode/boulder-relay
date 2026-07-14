#!/usr/bin/env bash
# Set up and trigger a COPR build for boulder-relay on Fedora.
# Requires: copr-cli, git, rpmbuild
# Usage: bash packaging/setup-copr.sh
set -euo pipefail

PROJECT="boulder-relay"
OWNER="SisyphusAeolides"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"//;s/"//')
ARCHIVE="boulder-relay-${VERSION}.tar.gz"
SPECFILE="packaging/boulder-relay.spec"

echo "==> COPR build for ${OWNER}/${PROJECT} v${VERSION}"

# Ensure copr-cli is installed
if ! command -v copr-cli &>/dev/null; then
    echo "Installing copr-cli..."
    sudo dnf install -y copr-cli
fi

# Create source tarball
echo "==> Creating source archive..."
git archive --prefix="boulder-relay-${VERSION}/" HEAD | gzip > "/tmp/${ARCHIVE}"

mkdir -p ~/rpmbuild/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
cp "/tmp/${ARCHIVE}" ~/rpmbuild/SOURCES/
cp "${SPECFILE}" ~/rpmbuild/SPECS/boulder-relay.spec

# Build SRPM
echo "==> Building SRPM..."
rpmbuild -bs ~/rpmbuild/SPECS/boulder-relay.spec

SRPM=$(find ~/rpmbuild/SRPMS -name "boulder-relay-${VERSION}*.src.rpm" | head -1)
echo "==> SRPM: ${SRPM}"

# Create COPR project if it doesn't exist
copr-cli create ${PROJECT} \
    --chroot fedora-rawhide-x86_64 \
    --chroot fedora-41-x86_64 \
    --chroot fedora-42-x86_64 \
    --chroot fedora-43-x86_64 \
    --description "Boulder Relay — GTK4 IRC/Matrix client in Rust" \
    --instructions "dnf copr enable ${OWNER}/${PROJECT} && dnf install boulder-relay" \
    || echo "(Project may already exist — continuing)"

# Submit the build
echo "==> Submitting COPR build..."
copr-cli build ${PROJECT} "${SRPM}"

echo "==> Done. Check https://copr.fedorainfracloud.org/coprs/${OWNER}/${PROJECT}/builds/"
