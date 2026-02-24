# Detect the OS to set the executable name and binary directory
ifeq ($(OS),Windows_NT)
    EXECUTABLE = Hephaestus.exe
    TARGET_DIR = target/release
else
    EXECUTABLE = Hephaestus
    TARGET_DIR = target/release
endif

.PHONY: all build clean

# Default target
all: build

# Build the project using cargo and copy the executable to the root
build:
	cargo build --release
	@if [ -f $(TARGET_DIR)/$(EXECUTABLE) ]; then \
		cp $(TARGET_DIR)/$(EXECUTABLE) ./$(EXECUTABLE); \
		echo "Executable $(EXECUTABLE) is ready in the root directory."; \
	elif [ -f $(TARGET_DIR)/hephaestus ]; then \
		cp $(TARGET_DIR)/hephaestus ./$(EXECUTABLE); \
		echo "Executable $(EXECUTABLE) is ready in the root directory."; \
	elif [ -f $(TARGET_DIR)/hephaestus.exe ]; then \
		cp $(TARGET_DIR)/hephaestus.exe ./$(EXECUTABLE); \
		echo "Executable $(EXECUTABLE) is ready in the root directory."; \
	else \
		echo "Error: Binary not found in $(TARGET_DIR)"; \
		exit 1; \
	fi

# Clean target
clean:
	cargo clean
	rm -f $(EXECUTABLE)
