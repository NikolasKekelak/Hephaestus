# Detect the OS to set the executable name and binary directory
ifeq ($(OS),Windows_NT)
    EXECUTABLE = heph.exe
    TARGET_DIR = target/release
    # Common user bin for Windows (e.g., if using Git Bash or similar)
    INSTALL_DIR ?= /usr/local/bin
else
    EXECUTABLE = heph
    TARGET_DIR = target/release
    INSTALL_DIR ?= /usr/local/bin
endif

.PHONY: all build clean install uninstall

# Default target
all: build

# Build the project using cargo and copy the executable to the root
build:
	cargo build --release
	@if [ -f $(TARGET_DIR)/$(EXECUTABLE) ]; then \
		cp $(TARGET_DIR)/$(EXECUTABLE) ./$(EXECUTABLE); \
		echo "Executable $(EXECUTABLE) is ready in the root directory."; \
	elif [ -f $(TARGET_DIR)/heph ]; then \
		cp $(TARGET_DIR)/heph ./$(EXECUTABLE); \
		echo "Executable $(EXECUTABLE) is ready in the root directory."; \
	elif [ -f $(TARGET_DIR)/heph.exe ]; then \
		cp $(TARGET_DIR)/heph.exe ./$(EXECUTABLE); \
		echo "Executable $(EXECUTABLE) is ready in the root directory."; \
	else \
		echo "Error: Binary not found in $(TARGET_DIR)"; \
		exit 1; \
	fi

# Clean target
clean:
	cargo clean
	rm -f $(EXECUTABLE)

# Install target: copies the executable to a system-wide bin directory
install: build
	@echo "Installing $(EXECUTABLE) to $(INSTALL_DIR)..."
	@mkdir -p $(INSTALL_DIR)
	@cp ./$(EXECUTABLE) $(INSTALL_DIR)/$(EXECUTABLE)
	@chmod +x $(INSTALL_DIR)/$(EXECUTABLE)
	@echo "Installation complete. You can now run '$(EXECUTABLE)' from anywhere."

# Uninstall target: removes the executable from the system-wide bin directory
uninstall:
	@echo "Removing $(EXECUTABLE) from $(INSTALL_DIR)..."
	@rm -f $(INSTALL_DIR)/$(EXECUTABLE)
	@echo "Uninstallation complete."
