CARGO ?= cargo
SCDOC ?= scdoc

all: bare doc/bare.1 doc/bare.5

bare:
	$(CARGO) build --release

bench:
	$(CARGO) test --release -- --test-threads 1 --ignored --nocapture

doc/bare.1: doc/bare.1.scd
	$(SCDOC) <doc/bare.1.scd >doc/bare.1

doc/bare.5: doc/bare.5.scd
	$(SCDOC) <doc/bare.5.scd >doc/bare.5

.PHONY: bare bench
