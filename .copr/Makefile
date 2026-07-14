# COPR Makefile for boulder-relay
# Called automatically by COPR infrastructure.

VERSION := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*= *"//;s/"//')
ARCHIVE  := boulder-relay-$(VERSION).tar.gz
SPECFILE := packaging/boulder-relay.spec

.PHONY: srpm
srpm:
	git archive --prefix=boulder-relay-$(VERSION)/ HEAD | gzip > /tmp/$(ARCHIVE)
	cp /tmp/$(ARCHIVE) $(outdir)/
	cp $(SPECFILE) $(outdir)/boulder-relay.spec
	rpmbuild -bs \
	    --define "_sourcedir $(outdir)" \
	    --define "_srcrpmdir $(outdir)" \
	    --define "_specdir   $(outdir)" \
	    $(outdir)/boulder-relay.spec
