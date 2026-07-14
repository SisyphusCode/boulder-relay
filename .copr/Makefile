VERSION := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*= *"//;s/"//')
ARCHIVE  := boulderX-$(VERSION).tar.gz
SPECFILE := packaging/boulderX.spec
TOPDIR   := $(shell pwd)

.PHONY: srpm
srpm:
	# Create a clean source tarball from the current working tree (no git needed)
	tar \
	    --exclude='.git' \
	    --exclude='.copr' \
	    --transform "s|^\./|boulderX-$(VERSION)/|" \
	    -czf $(outdir)/$(ARCHIVE) \
	    -C $(TOPDIR) .
	# Drop the spec next to the tarball so rpmbuild can find it
	cp $(SPECFILE) $(outdir)/boulderX.spec
	# Build the SRPM
	rpmbuild -bs \
	    --define "_sourcedir $(outdir)" \
	    --define "_srcrpmdir $(outdir)" \
	    --define "_specdir   $(outdir)" \
	    $(outdir)/boulderX.spec
