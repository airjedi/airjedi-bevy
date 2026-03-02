# AirJedi top-level Makefile
# Delegates to macos/ for application bundle targets

.PHONY: app install run icons clean

app:
	$(MAKE) -C macos app

install:
	$(MAKE) -C macos install

run:
	$(MAKE) -C macos run

icons:
	$(MAKE) -C macos icons

clean:
	$(MAKE) -C macos clean
