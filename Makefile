.PHONY: build
SHELL := /usr/bin/env bash

# Build in release mode by default, unless RELEASE=false
ifeq ($(RELEASE), false)
		cargoflag :=
		targetdir := debug
else
		cargoflag := --release
		targetdir := release
endif

build:
	cargo build $(cargoflag)

fix:
	cargo fmt
	cargo clippy --fix

package:
	# Clean and prepare target/package folder
	rm -rf target/package
	mkdir -p target/package
	# Copy binaries
	cp target/$(targetdir)/awatcher target/package/aw-awatcher 
	# Copy everything into `dist/awatcher`
	mkdir -p dist
	rm -rf dist/awatcher
	cp -rf target/package dist/awatcher

clean:
	cargo clean
