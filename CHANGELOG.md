# Changelog
All notable changes to this project will be documented in this file.

## [0.3.1] - 2025-02-08
### Changed
- Move to xum1541 0.3.1

## [0.3.0] - 2025-01-31
### Adeed
- [`Drive::read_file`]

### Changed
- Moved to using xum1541 generic types

## [0.2.0] - 2025-01-27
### Added
- [`Cbm::scan_bus`] and [`Cbm::scan_bus_range`]
- Added bus scan function to examples/cli.rs
- Added examples/statusloop.rs
- Change Rs1541Error to Error
- new modules for various objects
- Add [`CbmDriveUnit::try_from_bus`]
- Add [`CbmDriveunit::dir`]
- Changed [`CbmDriveUnit`] to take and sore [`CbmDeviceInfo`] rather than [`CbmDeviceType`]
- Added [`DosVersion`]
- Changed [`CbmDriveUnit::dir`] to return a CbmStatus alongside the directory listing

### Changed
- Moved away from opencbm bindings to A Rust native xum1541 implementation (xum1541)
- Added Cbm::load_file_ascii() and Cbm::load_file_petscii()
- Changed name of CbmError to Rs1541Error
- Changed return type of format_disk to Result<(), Rs1541Error> from Result<CbmStatus, Rs1541Error> (and turned an error staus into Rs1541Error::Status)
- Considerable changes to Rs1541Error
- Changed CbmDriveUnit::send_init() to return Vec<Result<CbmStatus, Rs1541Error>>

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
