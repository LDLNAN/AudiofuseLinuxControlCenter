//! Firmware archive (`.af8pre` ZIP container with `info.json` and `.bin` image)
//! inspector and USB DFU 1.10 (`Interface 3`, `wTransferSize = 64`) status and framing.

use crate::protocol::{DFU_INTERFACE, DFU_TRANSFER_SIZE};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;
use thiserror::Error;
use zip::ZipArchive;

/// USB DFU 1.10 Functional Descriptor attributes (`bmAttributes = 0x0d`: WillDetach | ManifestationTolerant | CanDnload).
pub const DFU_BM_ATTRIBUTES: u8 = 0x0d;

/// Standard USB DFU Class-Specific Request Codes (`bInterfaceClass = 0xfe`, `bInterfaceSubClass = 0x01`).
pub mod dfu_request {
    pub const DFU_DETACH: u8 = 0x00;
    pub const DFU_DNLOAD: u8 = 0x01;
    pub const DFU_UPLOAD: u8 = 0x02;
    pub const DFU_GETSTATUS: u8 = 0x03;
    pub const DFU_CLRSTATUS: u8 = 0x04;
    pub const DFU_GETSTATE: u8 = 0x05;
    pub const DFU_ABORT: u8 = 0x06;
}

/// Standard USB DFU 1.10 State values (`bState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum DfuState {
    #[default]
    AppIdle = 0,
    AppDetach = 1,
    DfuIdle = 2,
    DfuDnloadSync = 3,
    DfuDnbusy = 4,
    DfuDnloadIdle = 5,
    DfuManifestSync = 6,
    DfuManifest = 7,
    DfuManifestWaitReset = 8,
    DfuUploadIdle = 9,
    DfuError = 10,
}

impl DfuState {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::AppIdle,
            1 => Self::AppDetach,
            2 => Self::DfuIdle,
            3 => Self::DfuDnloadSync,
            4 => Self::DfuDnbusy,
            5 => Self::DfuDnloadIdle,
            6 => Self::DfuManifestSync,
            7 => Self::DfuManifest,
            8 => Self::DfuManifestWaitReset,
            9 => Self::DfuUploadIdle,
            _ => Self::DfuError,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AppIdle => "appIDLE",
            Self::AppDetach => "appDETACH",
            Self::DfuIdle => "dfuIDLE",
            Self::DfuDnloadSync => "dfuDNLOAD-SYNC",
            Self::DfuDnbusy => "dfuDNBUSY",
            Self::DfuDnloadIdle => "dfuDNLOAD-IDLE",
            Self::DfuManifestSync => "dfuMANIFEST-SYNC",
            Self::DfuManifest => "dfuMANIFEST",
            Self::DfuManifestWaitReset => "dfuMANIFEST-WAIT-RESET",
            Self::DfuUploadIdle => "dfuUPLOAD-IDLE",
            Self::DfuError => "dfuERROR",
        }
    }
}

/// 6-byte USB DFU `DFU_GETSTATUS` response payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DfuStatusReport {
    pub interface_number: u8,
    pub status_code: u8,
    pub poll_timeout_ms: u32,
    pub state: DfuState,
    pub string_index: u8,
    pub transfer_size: usize,
    pub attributes: u8,
}

impl Default for DfuStatusReport {
    fn default() -> Self {
        Self {
            interface_number: DFU_INTERFACE,
            status_code: 0,
            poll_timeout_ms: 10,
            state: DfuState::AppIdle,
            string_index: 0,
            transfer_size: DFU_TRANSFER_SIZE,
            attributes: DFU_BM_ATTRIBUTES,
        }
    }
}

impl DfuStatusReport {
    /// Decode a 6-byte `DFU_GETSTATUS` buffer (`bStatus`, `bwPollTimeout[3]`, `bState`, `iString`).
    pub fn from_bytes(bytes: &[u8; 6]) -> Self {
        let poll_timeout_ms =
            (bytes[1] as u32) | ((bytes[2] as u32) << 8) | ((bytes[3] as u32) << 16);
        Self {
            interface_number: DFU_INTERFACE,
            status_code: bytes[0],
            poll_timeout_ms,
            state: DfuState::from_u8(bytes[4]),
            string_index: bytes[5],
            transfer_size: DFU_TRANSFER_SIZE,
            attributes: DFU_BM_ATTRIBUTES,
        }
    }
}

/// Raw `info.json` metadata inside an Arturia `.af8pre` firmware archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Af8PreManifest {
    pub date: String,
    pub vendorid: String,
    pub productid: String,
    pub version_number: String,
    pub images: Vec<Af8PreImageEntry>,
}

/// Image entry inside `info.json` of an `.af8pre` archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Af8PreImageEntry {
    pub file_name: String,
    pub image_num: u32,
}

/// Inspected summary of an `.af8pre` firmware package, including extracted image sizes,
/// FNV-1a / CRC32 checksums, and 64-byte USB DFU block count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmwareInspection {
    pub archive_path: String,
    pub manifest: Af8PreManifest,
    pub images: Vec<InspectedFirmwareImage>,
    pub dfu_interface: u8,
    pub dfu_transfer_size: usize,
    pub targets_audiofuse_8pre: bool,
}

/// Detailed inspection of a single firmware binary image inside an `.af8pre` archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectedFirmwareImage {
    pub image_num: u32,
    pub file_name: String,
    pub size_bytes: usize,
    pub dfu_block_count_64b: usize,
    pub fnv1a_64_hex: String,
}

#[derive(Debug, Error)]
pub enum FirmwareError {
    #[error("Empty firmware image: {0}")]
    EmptyImage(String),
    #[error("Failed to open firmware archive: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid ZIP archive structure in .af8pre file: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Missing or invalid info.json inside .af8pre archive: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Referenced firmware binary '{0}' not found in .af8pre archive")]
    MissingBinary(String),
}

/// Compute deterministic 64-bit FNV-1a digest formatted as `0x{:016x}`.
pub fn fnv1a_64_hex(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("0x{:016x}", hash)
}

/// Inspect an `.af8pre` firmware archive on disk.
pub fn inspect_af8pre_file(path: impl AsRef<Path>) -> Result<FirmwareInspection, FirmwareError> {
    let path_ref = path.as_ref();
    let mut file = File::open(path_ref)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    inspect_af8pre_bytes(&bytes, path_ref.to_string_lossy().into_owned())
}

/// Inspect an `.af8pre` firmware archive from in-memory bytes.
pub fn inspect_af8pre_bytes(
    bytes: &[u8],
    archive_label: impl Into<String>,
) -> Result<FirmwareInspection, FirmwareError> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)?;

    let manifest: Af8PreManifest = {
        let mut info_file = archive.by_name("info.json")?;
        let mut info_str = String::new();
        info_file.read_to_string(&mut info_str)?;
        serde_json::from_str(&info_str)?
    };

    let mut inspected_images = Vec::with_capacity(manifest.images.len());
    for entry in &manifest.images {
        let mut bin_file = archive
            .by_name(&entry.file_name)
            .map_err(|_| FirmwareError::MissingBinary(entry.file_name.clone()))?;
        let mut img_bytes = Vec::new();
        bin_file.read_to_end(&mut img_bytes)?;
        let size_bytes = img_bytes.len();
        if size_bytes == 0 {
            return Err(FirmwareError::EmptyImage(entry.file_name.clone()));
        }
        let dfu_block_count_64b = size_bytes.div_ceil(DFU_TRANSFER_SIZE);
        let fnv1a_64_hex = fnv1a_64_hex(&img_bytes);
        inspected_images.push(InspectedFirmwareImage {
            image_num: entry.image_num,
            file_name: entry.file_name.clone(),
            size_bytes,
            dfu_block_count_64b,
            fnv1a_64_hex,
        });
    }

    let targets_audiofuse_8pre = manifest.vendorid.eq_ignore_ascii_case("0x1c75")
        && manifest.productid.eq_ignore_ascii_case("0xaf0e")
        && !inspected_images.is_empty();

    Ok(FirmwareInspection {
        archive_path: archive_label.into(),
        manifest,
        images: inspected_images,
        dfu_interface: DFU_INTERFACE,
        dfu_transfer_size: DFU_TRANSFER_SIZE,
        targets_audiofuse_8pre,
    })
}

/// Slice a raw firmware binary into 64-byte USB DFU `DFU_DNLOAD` blocks (`wValue = block_num`).
pub fn chunk_firmware_into_dfu_blocks(firmware_bin: &[u8]) -> Vec<(u16, Vec<u8>)> {
    firmware_bin
        .chunks(DFU_TRANSFER_SIZE)
        .enumerate()
        .map(|(block_idx, slice)| (block_idx as u16, slice.to_vec()))
        .collect()
}
