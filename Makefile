# Build and install the TextForge C library (Linux; see docs/compatibility.md).
#
#   make                          build the release shared and static libraries
#   make install [PREFIX=/usr/local] [DESTDIR=]   install library, header and pkg-config file
#   make uninstall                remove what install added
#   make check                    full validation (scripts/check.py)
#   make test                     C test suites and header examples only

PREFIX     ?= /usr/local
LIBDIR     ?= $(PREFIX)/lib
INCLUDEDIR ?= $(PREFIX)/include
PKGCONFIGDIR ?= $(LIBDIR)/pkgconfig
CARGO      ?= cargo
PYTHON     ?= python3
INSTALL    ?= install

VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
ABI     := $(shell echo $(VERSION) | awk -F. '{ if ($$1 == 0) print "0." $$2; else print $$1 }')
TARGET  := target/release
SHARED  := libtextforge_c.so
STATIC  := libtextforge_c.a

.PHONY: all install uninstall check test clean

all: $(TARGET)/$(SHARED) $(TARGET)/$(STATIC)

$(TARGET)/$(SHARED) $(TARGET)/$(STATIC): Cargo.toml Cargo.lock build.rs $(wildcard src/*.rs) $(shell find vendor/textforge/src -name '*.rs')
	$(CARGO) build --release --locked --lib

textforge-c.pc: textforge-c.pc.in Cargo.toml
	sed -e 's|@PREFIX@|$(PREFIX)|' -e 's|@LIBDIR@|$(LIBDIR)|' \
	    -e 's|@INCLUDEDIR@|$(INCLUDEDIR)|' -e 's|@VERSION@|$(VERSION)|' $< > $@

install: all textforge-c.pc
	$(INSTALL) -d "$(DESTDIR)$(LIBDIR)" "$(DESTDIR)$(INCLUDEDIR)" "$(DESTDIR)$(PKGCONFIGDIR)"
	$(INSTALL) -m 644 include/textforge.h "$(DESTDIR)$(INCLUDEDIR)/textforge.h"
	$(INSTALL) -m 755 $(TARGET)/$(SHARED) "$(DESTDIR)$(LIBDIR)/$(SHARED).$(VERSION)"
	ln -sf $(SHARED).$(VERSION) "$(DESTDIR)$(LIBDIR)/$(SHARED).$(ABI)"
	ln -sf $(SHARED).$(ABI) "$(DESTDIR)$(LIBDIR)/$(SHARED)"
	$(INSTALL) -m 644 $(TARGET)/$(STATIC) "$(DESTDIR)$(LIBDIR)/$(STATIC)"
	$(INSTALL) -m 644 textforge-c.pc "$(DESTDIR)$(PKGCONFIGDIR)/textforge-c.pc"

uninstall:
	rm -f "$(DESTDIR)$(INCLUDEDIR)/textforge.h" "$(DESTDIR)$(PKGCONFIGDIR)/textforge-c.pc"
	rm -f "$(DESTDIR)$(LIBDIR)/$(SHARED)" "$(DESTDIR)$(LIBDIR)/$(SHARED).$(ABI)" "$(DESTDIR)$(LIBDIR)/$(SHARED).$(VERSION)"
	rm -f "$(DESTDIR)$(LIBDIR)/$(STATIC)"

check:
	$(PYTHON) scripts/check.py

test:
	$(PYTHON) scripts/run_c_tests.py

clean:
	$(CARGO) clean
	rm -f textforge-c.pc
