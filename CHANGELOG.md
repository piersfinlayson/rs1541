# Changelog
All notable changes to this project will be documented in this file.

## [0.2.0] - 2025-01-??
### Changed
- Moved away from opencbm bindings to A Rust native xum1541 implementation (xum1541)
- Added Cbm::load_file_ascii() and Cbm::load_file_petscii()
- Changed return type of format_disk to Result<(), CbmError> from Result<CbmStatus, CbmError> (and turned an error staus into CbmError::StatusError)
- Removed CommandError as it overlapped with StatusError
- Removed device from StatusError, as it now contains CbmStatus which includes the device

### Fixed
- Drive was left in an odd state after identify() (because M-R leaves drive in odd state).  Fixed by reading status immediately after and throwing it away.

## [0.1.1] - 2025-01-18
### Added
- Added `try_new()` constructor for more robust USB device initialization
- Support for device recovery after failed connections

### Changed
- Simplified `new()` constructor to basic device opening

## [0.1.0] - 2025-01-18
### Added
- Initial release
