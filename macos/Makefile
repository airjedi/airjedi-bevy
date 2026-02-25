# AirJedi macOS Application Bundle
# Usage:
#   make icons  - Generate .icns from SVG
#   make app    - Build release binary and assemble .app bundle
#   make run    - Build and launch the .app
#   make clean  - Remove build/ directory

.PHONY: icons app install run clean

ROOT_DIR := $(shell cd .. && pwd)
BUILD_DIR := build
APP_NAME := AirJedi
ICONSET_DIR := icons/AppIcon.iconset
ICNS_FILE := icons/AppIcon.icns

icons:
	@echo "Generating icons..."
	bash scripts/gen-icons.sh
	@echo "Icons generated at $(ICNS_FILE)"

app: icons
	@echo "Building application bundle..."
	bash scripts/build-app.sh
	@echo "Application bundle created at $(BUILD_DIR)/$(APP_NAME).app"

install: app
	@echo "Installing to /Applications..."
	rm -rf /Applications/$(APP_NAME).app
	cp -R $(BUILD_DIR)/$(APP_NAME).app /Applications/$(APP_NAME).app
	@echo "Installed /Applications/$(APP_NAME).app"

run: app
	@echo "Launching $(APP_NAME)..."
	open $(BUILD_DIR)/$(APP_NAME).app

clean:
	@echo "Cleaning build directory..."
	rm -rf $(BUILD_DIR)
	rm -rf $(ICONSET_DIR)
	rm -f $(ICNS_FILE)
	@echo "Clean complete."
