# Shortcuts for common commands. Build logic lives in meson.build; this only calls Meson and the distribution's packaging tools.
# On first run, build/ is created automatically with the development settings (profile=development).

BUILDDIR := build
# The version is in meson.build (and Cargo.toml).
VERSION := $(shell sed -n "s/^  version: '\(.*\)',/\1/p" meson.build)
OUTPUTS := ../Outputs
RPMTOP := $(CURDIR)/build-rpm

.PHONY: build test run deb rpm clean help

build: $(BUILDDIR)/build.ninja
	meson compile -C $(BUILDDIR)

test: $(BUILDDIR)/build.ninja
	meson test -C $(BUILDDIR) --print-errorlogs

run: $(BUILDDIR)/build.ninja
	meson compile -C $(BUILDDIR) run

# .deb (Ubuntu / Debian): dpkg-buildpackage puts the output in the parent directory; move it to Outputs/.
deb:
	dpkg-buildpackage -b -us -uc
	mkdir -p $(OUTPUTS)
	mv ../anemobit_$(VERSION)-*.deb $(OUTPUTS)/
	rm -f ../anemobit_$(VERSION)-*.buildinfo ../anemobit_$(VERSION)-*.changes ../anemobit-dbgsym_$(VERSION)-*.ddeb

# .rpm (Fedora, run on Fedora): the source tarball includes LICENSE / NOTICE from the repo root.
rpm:
	rm -rf $(RPMTOP)
	mkdir -p $(RPMTOP)/SOURCES $(RPMTOP)/stage/anemobit-$(VERSION)
	tar -c --exclude=./build --exclude=./build-rpm --exclude=./target --exclude='./obj-*' --exclude=./debian . \
		| tar -x -C $(RPMTOP)/stage/anemobit-$(VERSION)
	cp ../LICENSE ../NOTICE $(RPMTOP)/stage/anemobit-$(VERSION)/
	tar -czf $(RPMTOP)/SOURCES/anemobit-$(VERSION).tar.gz -C $(RPMTOP)/stage anemobit-$(VERSION)
	rpmbuild -bb --define "_topdir $(RPMTOP)" build-aux/anemobit.spec
	mkdir -p $(OUTPUTS)
	mv $(RPMTOP)/RPMS/*/anemobit-$(VERSION)-*.rpm $(OUTPUTS)/

# Removes everything make produces: build folders, packaging intermediates, by-products dpkg-buildpackage
# leaves in the parent folder, and this version's packages in Outputs/ (other files there are kept).
clean:
	rm -rf $(BUILDDIR) target build-rpm obj-*
	rm -rf debian/.debhelper debian/anemobit debian/tmp debian/cargo-home debian/files debian/debhelper-build-stamp debian/*.substvars debian/*.log
	rm -f ../anemobit_$(VERSION)-*.deb ../anemobit_$(VERSION)-*.buildinfo ../anemobit_$(VERSION)-*.changes ../anemobit-dbgsym_$(VERSION)-*.ddeb
	rm -f $(OUTPUTS)/anemobit_$(VERSION)-*.deb $(OUTPUTS)/anemobit-$(VERSION)-*.rpm

help:
	@echo "make build   Build the development version into build/ (run it with make run, not directly)"
	@echo "make test    Run all tests (cargo test, clippy, translation and .desktop / metainfo / schema checks)"
	@echo "make run     Build, then launch the app in the development environment"
	@echo "make deb     Build the .deb (requires debhelper) into ../Outputs/"
	@echo "make rpm     Build the .rpm (on Fedora, requires rpm-build) into ../Outputs/"
	@echo "make clean   Remove all build output, including this version's .deb / .rpm in ../Outputs/"

$(BUILDDIR)/build.ninja:
	meson setup $(BUILDDIR) -Dprofile=development
