#!/usr/bin/env bash
set -euo pipefail

have() { command -v "$1" >/dev/null 2>&1; }

dnf_command() {
  if have dnf; then
    printf 'dnf\n'
  elif have dnf5; then
    printf 'dnf5\n'
  fi
}

missing_debian_packages() {
  local package

  for package in "$@"; do
    if ! dpkg-query -W -f='${db:Status-Abbrev}' "$package" 2>/dev/null | grep -q '^ii '; then
      printf '%s\n' "$package"
    fi
  done
}

missing_pacman_packages() {
  local package

  for package in "$@"; do
    if ! pacman -Qq "$package" >/dev/null 2>&1; then
      printf '%s\n' "$package"
    fi
  done
}

missing_dnf_packages() {
  local package

  for package in "$@"; do
    if ! rpm -q "$package" >/dev/null 2>&1 && ! rpm -q --whatprovides "$package" >/dev/null 2>&1; then
      printf '%s\n' "$package"
    fi
  done
}

if have apt-get && [[ -z "$(dnf_command)" ]]; then
  echo "Detected apt (Debian/Ubuntu). Checking packages..."
  debian_packages=(
    build-essential \
    cmake \
    cmake-curses-gui \
    cmake-qt-gui \
    cmake-format \
    cmake-extras \
    extra-cmake-modules \
    ninja-build \
    make \
    ccache \
    clang \
    clang-tools \
    clang-tidy \
    clang-format \
    cppcheck \
    doxygen \
    graphviz \
    cargo \
    rustc \
    rust-src \
    pkg-config \
    dpkg-dev \
    debhelper \
    desktop-file-utils \
    appstream \
    flatpak-builder \
    libclang-dev \
    libcairo2-dev \
    libcairo2 \
    libpango1.0-dev \
    libpango-1.0-0 \
    libgdk-pixbuf-2.0-dev \
    libgdk-pixbuf-2.0-0 \
    libgdk-pixbuf-xlib-2.0-dev \
    libgdk-pixbuf-xlib-2.0-0 \
    libglib2.0-dev \
    libglib2.0-0 \
    libgraphene-1.0-dev \
    libgraphene-1.0-0 \
    libgtk-4-dev \
    libgtk-4-1 \
    libgstreamer1.0-dev \
    libgstreamer1.0-0 \
    libgstreamer-plugins-base1.0-dev \
    libgstreamer-plugins-base1.0-0 \
    libadwaita-1-dev \
    libadwaita-1-0 \
    libdiscid-dev \
    libdiscid0 \
    libgpgme-dev \
    libgpgme11 \
    libgcrypt20-dev \
    libgcrypt20 \
    libcurl4-openssl-dev \
    libcurl4 \
    zsync \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-ugly \
    cdparanoia \
    cd-discid \
    eject \
    flac \
    lame \
    opus-tools \
    vorbis-tools
  )
  mapfile -t missing_packages < <(missing_debian_packages "${debian_packages[@]}")

  if (( ${#missing_packages[@]} == 0 )); then
    echo "All Debian/Ubuntu packages are already installed."
  else
    echo "Installing missing Debian/Ubuntu packages:"
    printf '  %s\n' "${missing_packages[@]}"
    sudo apt-get update
    sudo apt-get install -y "${missing_packages[@]}"
  fi
  echo "Done."
elif have pacman && [[ -z "$(dnf_command)" ]]; then
  echo "Detected pacman (Arch). Checking packages..."
  pacman_packages=(
    base-devel
    devtools
    cargo
    rust
    rust-src
    pkgconf
    clang
    desktop-file-utils
    appstream
    flatpak-builder
    glib2
    cairo
    pango
    gdk-pixbuf2
    graphene
    gtk4
    gstreamer
    gst-plugins-base
    gst-plugins-good
    gst-plugins-ugly
    libadwaita
    libdiscid
    cdparanoia
    cd-discid
    eject
    flac
    lame
    opus-tools
    vorbis-tools
  )
  mapfile -t missing_packages < <(missing_pacman_packages "${pacman_packages[@]}")

  if (( ${#missing_packages[@]} == 0 )); then
    echo "All Arch packages are already installed."
  else
    echo "Installing missing Arch packages:"
    printf '  %s\n' "${missing_packages[@]}"
    sudo pacman -S --needed "${missing_packages[@]}"
  fi
  echo "Done."
elif dnf_cmd="$(dnf_command)" && [[ -n "$dnf_cmd" ]]; then
  echo "Detected dnf (Fedora/RHEL). Checking packages..."
  # Note: 'lame' and gstreamer1-plugins-ugly-free may require RPM Fusion on Fedora:
  #   sudo dnf install https://download1.rpmfusion.org/free/fedora/rpmfusion-free-release-$(rpm -E %fedora).noarch.rpm
  # Fedora equivalents for Debian Build-Depends:
  #   debhelper-compat (= 13) -> debhelper
  #   rustc -> rust
  #   pkg-config -> pkgconf-pkg-config
  #   libclang-dev -> clang-devel
  #   libdiscid-dev -> libdiscid-devel
  #   libglib2.0-dev -> glib2-devel
  #   libgstreamer1.0-dev -> gstreamer1-devel
  #   libgstreamer-plugins-base1.0-dev -> gstreamer1-plugins-base-devel
  #   libgtk-4-dev -> gtk4-devel
  #   libadwaita-1-dev -> libadwaita-devel
  #   libgdk-pixbuf-2.0-dev -> gdk-pixbuf2-devel
  #   libgdk-pixbuf-xlib-2.0-dev -> gdk-pixbuf2-xlib-devel
  #   libcairo2-dev -> cairo-devel
  #   libpango1.0-dev -> pango-devel
  #   libgraphene-1.0-dev -> graphene-devel
  #   libgpgme-dev -> gpgme-devel
  #   libgcrypt20-dev -> libgcrypt-devel
  #   libcurl4-openssl-dev -> libcurl-devel
  #   libgstreamer1.0-0 -> gstreamer1
  #   libgstreamer-plugins-base1.0-0 -> gstreamer1-plugins-base
  #   libadwaita-1-0 -> libadwaita
  #   libdiscid0 -> libdiscid
  dnf_deps=(
    gcc
    gcc-c++
    make
    cmake
    cmake-gui
    cmakelang
    extra-cmake-modules
    ninja-build
    ccache
    cargo
    rust
    rust-src
    dpkg-dev
    debhelper
    rpm-build
    rpmdevtools
    pkgconf-pkg-config
    clang
    clang-devel
    clang-tools-extra
    cppcheck
    doxygen
    graphviz
    desktop-file-utils
    appstream
    libappstream-glib
    flatpak-builder
    squashfs-tools
    zsync
    curl
  )
  dnf_libs=(
    glib2-devel
    glib2
    cairo-devel
    cairo
    pango-devel
    pango
    gdk-pixbuf2-devel
    gdk-pixbuf2
    gdk-pixbuf2-xlib-devel
    gdk-pixbuf2-xlib
    graphene-devel
    graphene
    gtk4-devel
    gtk4
    gstreamer1-devel
    gstreamer1-plugins-base-devel
    gstreamer1-plugins-base
    gstreamer1-plugins-good
    gstreamer1-plugins-ugly-free
    libadwaita-devel
    libadwaita
    'pkgconfig(libdiscid)'
    libdiscid-devel
    libdiscid
    gpgme-devel
    gpgme
    libgcrypt-devel
    libgcrypt
    libcurl-devel
    libcurl
    cdparanoia
    cd-discid
    util-linux
    flac
    lame
    opus-tools
    vorbis-tools
  )
  dnf_packages=("${dnf_deps[@]}" "${dnf_libs[@]}")
  mapfile -t missing_packages < <(missing_dnf_packages "${dnf_packages[@]}")

  if (( ${#missing_packages[@]} == 0 )); then
    echo "All Fedora/RHEL packages are already installed."
  else
    echo "Installing missing Fedora/RHEL packages:"
    printf '  %s\n' "${missing_packages[@]}"
    sudo "$dnf_cmd" install -y "${missing_packages[@]}"
  fi
  echo "Done."
elif have zypper; then
  # Note: 'lame' and 'cd-discid' may require the Packman repository on openSUSE:
  #   sudo zypper ar -cfp 90 https://ftp.gwdg.de/pub/linux/misc/packman/suse/openSUSE_Tumbleweed/ packman
  #   sudo zypper dup --from packman --allow-vendor-change
  echo "Detected zypper (openSUSE). Installing packages..."
  sudo zypper install -y \
    gcc \
    make \
    rpm-build \
    rpmlint \
    cargo \
    rust \
    pkg-config \
    clang-devel \
    desktop-file-utils \
    AppStream \
    flatpak-builder \
    glib2-devel \
    cairo-devel \
    pango-devel \
    gdk-pixbuf-devel \
    libgraphene-devel \
    gtk4-devel \
    gstreamer-devel \
    gstreamer-plugins-base-devel \
    gstreamer-plugins-good \
    gstreamer-plugins-ugly \
    libadwaita-devel \
    libdiscid-devel \
    cdparanoia \
    cd-discid \
    eject \
    flac \
    lame \
    opus-tools \
    vorbis-tools
  sudo zypper install -y rust-src || \
    echo "rust-src was not available from configured openSUSE repositories; continuing."
  echo "Done."
else
  echo "Unsupported package manager. Please install dependencies manually." >&2
  exit 1
fi
