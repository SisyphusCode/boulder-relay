#!/usr/bin/env bash
# Build an RPM for Boulder Relay on Fedora.
# Usage: bash packaging/build-rpm.sh
set -euo pipefail

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"//;s/"//')
ARCHIVE="boulder-relay-${VERSION}.tar.gz"
SPECFILE="packaging/boulder-relay.spec"

echo "==> Building boulder-relay v${VERSION} RPM"

# Create tarball from current tree
echo "==> Creating source archive ${ARCHIVE}..."
git archive --prefix="boulder-relay-${VERSION}/" HEAD | gzip > "/tmp/${ARCHIVE}"

# Ensure rpmbuild dirs exist
mkdir -p ~/rpmbuild/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

cp "/tmp/${ARCHIVE}" ~/rpmbuild/SOURCES/
cp "${SPECFILE}" ~/rpmbuild/SPECS/boulder-relay.spec

# Install build dependencies (requires sudo)
echo "==> Installing build dependencies..."
sudo dnf builddep -y ~/rpmbuild/SPECS/boulder-relay.spec || true

# Build the RPM
echo "==> Running rpmbuild..."
rpmbuild -ba ~/rpmbuild/SPECS/boulder-relay.spec

echo ""
echo "==> RPM built successfully:"
find ~/rpmbuild/RPMS -name "boulder-relay-*.rpm" -print
echo ""
echo "Install with:"
echo "  sudo dnf install ~/rpmbuild/RPMS/$(uname -m)/boulder-relay-${VERSION}-1.$(rpm --eval '%{?dist}' | tr -d '.').$(uname -m).rpm"
