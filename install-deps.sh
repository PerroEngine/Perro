#!/bin/bash
# Install system dependencies for Perro project

set -e

echo "Installing system dependencies for Perro..."

# Check if running on Ubuntu/Debian
if command -v apt &> /dev/null; then
    echo "Detected Ubuntu/Debian system"
    echo "Attempting to update package list (may fail due to GPG issues, will continue anyway)..."
    sudo apt update || echo "Warning: apt update failed, but continuing with installation..."
    echo "Installing dependencies..."
    sudo apt install -y \
        libdbus-1-dev \
        pkg-config \
        libudev-dev \
        libusb-1.0-0-dev \
        libx11-dev \
        libxrandr-dev \
        libxcb-render0-dev \
        libxcb-shape0-dev \
        libxcb-xfixes0-dev \
        libxkbcommon-dev \
        libwayland-dev \
        libasound2-dev
    echo "Dependencies installed successfully!"
elif command -v dnf &> /dev/null; then
    echo "Detected Fedora/RHEL system"
    sudo dnf install -y \
        dbus-devel \
        pkgconf-pkg-config \
        systemd-devel \
        libusb1-devel \
        libX11-devel \
        libXrandr-devel \
        libxcb-devel \
        libxkbcommon-devel \
        wayland-devel \
        alsa-lib-devel
    echo "Dependencies installed successfully!"
elif command -v pacman &> /dev/null; then
    echo "Detected Arch Linux system"
    sudo pacman -S --noconfirm \
        dbus \
        pkgconf \
        libudev \
        libusb \
        libx11 \
        libxrandr \
        libxcb \
        libxkbcommon \
        wayland \
        alsa-lib
    echo "Dependencies installed successfully!"
else
    echo "Unknown package manager. Please install the following packages manually:"
    echo "  - libdbus-1-dev (or dbus-devel)"
    echo "  - pkg-config (or pkgconf)"
    echo "  - libudev-dev (or systemd-devel/libudev)"
    echo "  - libusb-1.0-0-dev (or libusb1-devel)"
    echo "  - Graphics libraries (libx11-dev, libxrandr-dev, etc.)"
    exit 1
fi

echo ""
echo "Verifying dependencies..."
if pkg-config --exists dbus-1; then
    echo "✓ dbus-1 is now available"
    pkg-config --modversion dbus-1
else
    echo "✗ dbus-1 still not found. You may need to run:"
    echo "  export PKG_CONFIG_PATH=\$(pkg-config --variable pc_path pkg-config)"
    exit 1
fi

if pkg-config --exists libudev; then
    echo "✓ libudev is now available"
    pkg-config --modversion libudev
else
    echo "✗ libudev still not found. You may need to run:"
    echo "  export PKG_CONFIG_PATH=\$(pkg-config --variable pc_path pkg-config)"
    exit 1
fi

