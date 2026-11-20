# Changelog

Notable user-visible changes to AudiofuseLinuxControlCenter. Updated with every merge to local `dev`; dates are merge dates (2026). Ticket IDs refer to the Tephra ledger.

## Unreleased

**Verified** 2026-11-21: independent review 85.9/100 (threshold 85), same-family reviewer (Claude Sonnet 5.5 on Claude Opus work, Codex was out). Open scope question (not scored): hardware preset slots, master mix, hardware channel names and runtime DFU are not supported yet.

### Fixed
- Package install hook now warns and carries on when Secure Boot rejects the module or an older kernel fails to build, instead of leaving the package half-configured (AF-008, 11-21).
- Live sample-rate controls are read-only for users who lack permission, instead of showing a raw permission error (AF-001, 11-21).

### Added
- Native Linux control center and daemon for the Arturia AudioFuse 8Pre (1c75:af0e): live hardware transport and wire protocol, 152-byte VU telemetry, an ALSA-coexistent control path, preset import/export, and Debian, RPM and Arch packaging (AF-001..008, 11-19).
