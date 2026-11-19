Name:           audiofuse-control-center
Version:        1.0.0
Release:        1%{?dist}
Summary:        Native Linux Control Center and CLI for the Arturia AudioFuse 8Pre (1c75:af0e)
License:        MIT
URL:            https://github.com/LDLNAN/AudiofuseLinuxControlCenter
BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  pkgconfig(libusb-1.0)
BuildRequires:  pkgconfig(alsa)
Requires:       libusbx
Requires:       alsa-lib
Requires:       systemd-udev
Requires:       libX11 libXcursor libXi libxkbcommon-x11 libglvnd-glx libglvnd-egl libwayland-client

Source0: %{name}-%{version}.tar.gz
BuildRequires: systemd-rpm-macros

%description
Native Linux replacement for the Arturia AudioFuse 8Pre Control Center (1c75:af0e).
Provides 8 analog input channel strips, 24-channel low-latency hardware cue mixer,
master monitor section, output/loopback routing matrix, 78-byte real-time VU metering,
hardware presets, .af8pre firmware inspection, CLI, and non-root udev rules.

%prep
%autosetup

%build
cargo build --frozen --release --workspace

%install
install -Dm755 target/release/audiofuse-control-center %{buildroot}%{_bindir}/audiofuse-control-center
install -Dm755 target/release/audiofuse-cli %{buildroot}%{_bindir}/audiofuse-cli
install -Dm644 udev/70-arturia-audiofuse.rules %{buildroot}%{_udevrulesdir}/70-arturia-audiofuse.rules
install -Dm644 packaging/audiofuse-control-center.desktop %{buildroot}%{_datadir}/applications/audiofuse-control-center.desktop
install -Dm644 packaging/audiofuse-control-center.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/audiofuse-control-center.svg

%post
if command -v udevadm >/dev/null; then udevadm control --reload-rules || :; fi

%files
%doc README.md
%{_bindir}/audiofuse-control-center
%{_bindir}/audiofuse-cli
%{_udevrulesdir}/70-arturia-audiofuse.rules
%{_datadir}/applications/audiofuse-control-center.desktop
%{_datadir}/icons/hicolor/scalable/apps/audiofuse-control-center.svg
