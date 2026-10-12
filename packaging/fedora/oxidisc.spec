Name:           oxidisc
Version:        2.0.0
Release:        1%{?dist}
Summary:        Modern and fast audio CD ripper in Rust with egui GUI and Ratatui TUI

License:        MIT
URL:            https://github.com/dral/OxiDisc
Source0:        %{url}/archive/refs/tags/v%{version}/%{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  clang-devel
BuildRequires:  pkgconfig
BuildRequires:  desktop-file-utils
BuildRequires:  libappstream-glib
BuildRequires:  pkgconfig(libdiscid)

Requires:       cdparanoia
Requires:       cd-discid
Requires:       eject
Requires:       flac
Requires:       lame
Requires:       libdiscid
Requires:       opus-tools
Requires:       vorbis-tools

%description
OxiDisc detects audio CDs, looks up metadata via MusicBrainz, GnuDB, and
AcoustID, and extracts selected tracks to FLAC, MP3, WAV, Ogg Vorbis, or Opus.

%prep
%autosetup -n %{name}-%{version}

%build
cargo build --release --locked --features "egui-ui tui"

%check
cargo test --locked --features "egui-ui tui"
desktop-file-validate resources/oxidisc.desktop
appstream-util validate-relax --nonet resources/metainfo/io.github.dral.oxidisc.metainfo.xml

%install
install -Dm0755 target/release/oxidisc %{buildroot}%{_bindir}/oxidisc
install -Dm0644 resources/oxidisc.desktop %{buildroot}%{_datadir}/applications/oxidisc.desktop
install -Dm0644 resources/images/oxidisc.png %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/io.github.dral.oxidisc.png
install -Dm0644 resources/metainfo/io.github.dral.oxidisc.metainfo.xml %{buildroot}%{_datadir}/metainfo/io.github.dral.oxidisc.metainfo.xml
install -Dm0644 LICENSE %{buildroot}%{_datadir}/licenses/%{name}/LICENSE

%files
%license %{_datadir}/licenses/%{name}/LICENSE
%{_bindir}/oxidisc
%{_datadir}/applications/oxidisc.desktop
%{_datadir}/icons/hicolor/256x256/apps/io.github.dral.oxidisc.png
%{_datadir}/metainfo/io.github.dral.oxidisc.metainfo.xml

%changelog
* Sun Aug 30 2026 Alexander Drichel <adrichel@users.noreply.github.com> - 2.0.0-1
- Update to 2.0.0 with Opus support, AcoustID Chromaprint recognition, GnuDB cascade, and Windows support.

* Fri May 08 2026 Alexander Drichel <adrichel@users.noreply.github.com> - 1.1.0-1
- Initial Fedora/COPR recipe.
