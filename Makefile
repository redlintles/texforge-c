# Convenience wrapper around the CMake build (CMakeLists.txt is the source of truth).
#
#   make                                     build the release static and shared libraries
#   make install [PREFIX=/usr/local] [DESTDIR=]   install both, the header, the CMake
#                                            package and the pkg-config module
#   make check                               full validation (scripts/check.py)
#   make test                                C test suites and header examples only
#
# Package managers (Conan, vcpkg) call CMake directly with one linkage per build.

PREFIX  ?= /usr/local
CMAKE   ?= cmake
PYTHON  ?= python3
BUILD   ?= build

.PHONY: all install uninstall check test clean

all:
	$(CMAKE) -S . -B "$(BUILD)/static" -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DCMAKE_INSTALL_PREFIX="$(PREFIX)"
	$(CMAKE) -S . -B "$(BUILD)/shared" -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=ON -DCMAKE_INSTALL_PREFIX="$(PREFIX)"
	$(CMAKE) --build "$(BUILD)/static"
	$(CMAKE) --build "$(BUILD)/shared"

# The shared install runs last, so the CMake package selects the shared library.
install: all
	DESTDIR="$(DESTDIR)" $(CMAKE) --install "$(BUILD)/static"
	DESTDIR="$(DESTDIR)" $(CMAKE) --install "$(BUILD)/shared"

uninstall:
	for manifest in "$(BUILD)/static/install_manifest.txt" "$(BUILD)/shared/install_manifest.txt"; do \
	    [ -f "$$manifest" ] && xargs -d '\n' rm -f < "$$manifest"; done; true

check:
	$(PYTHON) scripts/check.py

test:
	$(PYTHON) scripts/run_c_tests.py

clean:
	rm -rf "$(BUILD)"
