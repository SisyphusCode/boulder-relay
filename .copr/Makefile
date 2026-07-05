# COPR make_srpm target — invoked as:
#   make -f .copr/Makefile srpm outdir="<outdir>" spec="<spec_path>"
VERSION ?= 0.3.0

srpm:
	@set -euo pipefail; \
	STAGING=$$(mktemp -d); \
	SRC_DIR="$$STAGING/boulder-relay-$(VERSION)"; \
	mkdir -p "$$SRC_DIR"; \
	tar -C . \
		--exclude='./target' \
		--exclude='./.git' \
		--exclude='./dist' \
		--exclude='./*.tar.gz' \
		-cf - . | tar -C "$$SRC_DIR" -xf -; \
	tar -C "$$STAGING" -czf "$$STAGING/boulder-relay-$(VERSION).tar.gz" "boulder-relay-$(VERSION)"; \
	rpmbuild -bs "$(spec)" \
		--define "_sourcedir $$STAGING" \
		--define "_srcrpmdir $(outdir)" \
		--define "_builddir /tmp/boulder-relay-copr-build"; \
	rm -rf "$$STAGING"
