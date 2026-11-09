//! Android boot image parsing and the direct-kernel boot assets for the emulated engine.
//!
//! QEMU's fallback boot path (§3.4) needs a kernel, an initrd and a kernel command line. A phone
//! ships those inside `boot`, `init_boot` and `vendor_boot` images, so this module reads the v3/v4
//! formats with strict bounds checks and assembles the initrd exactly as the Android bootloader
//! would:
//!
//! ```text
//! initrd = vendor_boot.vendor_ramdisk ‖ init_boot.ramdisk ‖ bootconfig trailer
//! ```
//!
//! Nothing here trusts a size field: every offset is checked against the buffer, every section is
//! capped, and header versions outside v3/v4 are refused instead of guessed at. Parsing runs at
//! install time in Rust, never inside the QEMU process.

/// Boot v3/v4 sections are aligned to this page size (`BOOT_IMAGE_HEADER_V3_PAGESIZE`).
pub const BOOT_PAGE_SIZE: usize = 4096;
/// A boot image header v3 is 1580 bytes; v4 adds `signature_size`.
pub const BOOT_HEADER_V3_SIZE: u32 = 1580;
pub const BOOT_HEADER_V4_SIZE: u32 = 1584;
pub const VENDOR_BOOT_HEADER_V3_SIZE: u32 = 2112;
pub const VENDOR_BOOT_HEADER_V4_SIZE: u32 = 2128;
/// `mkbootimg` appends at most a 64 KiB bootconfig block.
pub const MAX_BOOTCONFIG_BYTES: usize = 64 * 1024;
/// Upper bound for a kernel, ramdisk or DTB section: larger means the image is not real.
const MAX_SECTION_BYTES: usize = 512 * 1024 * 1024;
/// Kernel command lines are 1536 (boot) and 2048 (vendor_boot) bytes.
const MAX_CMDLINE_BYTES: usize = 2048;

const BOOT_MAGIC: &[u8] = b"ANDROID!";
const VENDOR_BOOT_MAGIC: &[u8] = b"VNDRBOOT";
const BOOTCONFIG_MAGIC: &[u8] = b"#BOOTCONFIG\n";
/// Padding byte the kernel uses for a bootconfig block that is not a multiple of four bytes.
const BOOTCONFIG_PAD: u8 = b'#';

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootImageError {
    TooSmall { needed: usize, got: usize },
    BadMagic { expected: &'static str },
    UnsupportedHeaderVersion(u32),
    BadHeaderSize { expected: u32, found: u32 },
    BadPageSize(u32),
    SectionOutOfBounds { section: &'static str },
    SectionTooLarge { section: &'static str },
    InvalidCmdline,
    EmptyKernel,
    EmptyVendorRamdisk,
    InvalidBootConfigParameter { index: usize },
    BootConfigTooLarge,
}

impl BootImageError {
    pub fn message(&self) -> String {
        match self {
            BootImageError::TooSmall { needed, got } => {
                format!("image is {got} bytes; its header needs {needed}")
            }
            BootImageError::BadMagic { expected } => {
                format!("image does not start with {expected}")
            }
            BootImageError::UnsupportedHeaderVersion(version) => format!(
                "boot image header version {version} is unsupported; this engine reads v3 and v4"
            ),
            BootImageError::BadHeaderSize { expected, found } => {
                format!("header_size {found} does not match the {expected}-byte v3/v4 header")
            }
            BootImageError::BadPageSize(page) => format!("page size {page} is not 2048 or 4096"),
            BootImageError::SectionOutOfBounds { section } => {
                format!("the {section} section extends past the end of the image")
            }
            BootImageError::SectionTooLarge { section } => {
                format!("the {section} section is implausibly large")
            }
            BootImageError::InvalidCmdline => {
                "the kernel command line contains a non-printable character".into()
            }
            BootImageError::EmptyKernel => "the boot image carries no kernel".into(),
            BootImageError::EmptyVendorRamdisk => {
                "the vendor_boot image carries no vendor ramdisk".into()
            }
            BootImageError::InvalidBootConfigParameter { index } => {
                format!("bootconfig parameter {index} is empty or contains a control character")
            }
            BootImageError::BootConfigTooLarge => {
                "the bootconfig block exceeds the kernel's 64 KiB limit".into()
            }
        }
    }
}

/// A parsed boot v3/v4 image (`boot.img` or `init_boot.img`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootImage {
    pub version: u32,
    pub header_size: u32,
    pub cmdline: String,
    pub kernel: Vec<u8>,
    pub ramdisk: Vec<u8>,
    pub signature: Vec<u8>,
}

impl BootImage {
    pub fn parse(bytes: &[u8]) -> Result<Self, BootImageError> {
        if bytes.len() < BOOT_HEADER_V3_SIZE as usize {
            return Err(BootImageError::TooSmall {
                needed: BOOT_HEADER_V3_SIZE as usize,
                got: bytes.len(),
            });
        }
        if &bytes[0..8] != BOOT_MAGIC {
            return Err(BootImageError::BadMagic {
                expected: "ANDROID!",
            });
        }
        let kernel_size = read_u32(bytes, 8);
        let ramdisk_size = read_u32(bytes, 12);
        let header_size = read_u32(bytes, 20);
        let version = read_u32(bytes, 40);
        let (expected_header, signature_size) = match version {
            3 => (BOOT_HEADER_V3_SIZE, 0),
            4 => {
                if bytes.len() < BOOT_HEADER_V4_SIZE as usize {
                    return Err(BootImageError::TooSmall {
                        needed: BOOT_HEADER_V4_SIZE as usize,
                        got: bytes.len(),
                    });
                }
                (BOOT_HEADER_V4_SIZE, read_u32(bytes, 1580))
            }
            other => return Err(BootImageError::UnsupportedHeaderVersion(other)),
        };
        if header_size != expected_header {
            return Err(BootImageError::BadHeaderSize {
                expected: expected_header,
                found: header_size,
            });
        }
        let cmdline = parse_cmdline(&bytes[44..44 + 1536])?;

        let kernel_start = BOOT_PAGE_SIZE;
        let kernel_end = section_end(kernel_start, kernel_size, "kernel", bytes.len())?;
        let ramdisk_start = align_up(kernel_end as u32, BOOT_PAGE_SIZE as u32)?;
        let ramdisk_end = section_end(ramdisk_start, ramdisk_size, "ramdisk", bytes.len())?;
        let (signature_start, signature_end) = if signature_size == 0 {
            (ramdisk_end, ramdisk_end)
        } else {
            let start = align_up(ramdisk_end as u32, BOOT_PAGE_SIZE as u32)?;
            (
                start,
                section_end(start, signature_size, "signature", bytes.len())?,
            )
        };

        Ok(Self {
            version,
            header_size,
            cmdline,
            kernel: bytes[kernel_start..kernel_end].to_vec(),
            ramdisk: bytes[ramdisk_start..ramdisk_end].to_vec(),
            signature: bytes[signature_start..signature_end].to_vec(),
        })
    }
}

/// A parsed vendor_boot v3/v4 image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorBootImage {
    pub version: u32,
    pub page_size: u32,
    pub cmdline: String,
    pub name: String,
    pub vendor_ramdisk: Vec<u8>,
    pub dtb: Vec<u8>,
    pub ramdisk_table: Vec<u8>,
    pub bootconfig: Vec<u8>,
}

impl VendorBootImage {
    pub fn parse(bytes: &[u8]) -> Result<Self, BootImageError> {
        if bytes.len() < VENDOR_BOOT_HEADER_V3_SIZE as usize {
            return Err(BootImageError::TooSmall {
                needed: VENDOR_BOOT_HEADER_V3_SIZE as usize,
                got: bytes.len(),
            });
        }
        if &bytes[0..8] != VENDOR_BOOT_MAGIC {
            return Err(BootImageError::BadMagic {
                expected: "VNDRBOOT",
            });
        }
        let version = read_u32(bytes, 8);
        let expected_header = match version {
            3 => VENDOR_BOOT_HEADER_V3_SIZE,
            4 => VENDOR_BOOT_HEADER_V4_SIZE,
            other => return Err(BootImageError::UnsupportedHeaderVersion(other)),
        };
        if bytes.len() < expected_header as usize {
            return Err(BootImageError::TooSmall {
                needed: expected_header as usize,
                got: bytes.len(),
            });
        }
        let page_size = read_u32(bytes, 12);
        if page_size != 2048 && page_size != 4096 {
            return Err(BootImageError::BadPageSize(page_size));
        }
        let header_size = read_u32(bytes, 2096);
        if header_size != expected_header {
            return Err(BootImageError::BadHeaderSize {
                expected: expected_header,
                found: header_size,
            });
        }
        let vendor_ramdisk_size = read_u32(bytes, 24);
        let dtb_size = read_u32(bytes, 2100);
        let cmdline = parse_cmdline(&bytes[28..28 + 2048])?;
        let name = parse_name(&bytes[2080..2096])?;
        let (table_size, bootconfig_size) = if version >= 4 {
            (read_u32(bytes, 2112), read_u32(bytes, 2124))
        } else {
            (0, 0)
        };
        if bootconfig_size as usize > MAX_BOOTCONFIG_BYTES {
            return Err(BootImageError::BootConfigTooLarge);
        }

        let vendor_ramdisk_start = align_up(expected_header, page_size)?;
        let vendor_ramdisk_end = section_end(
            vendor_ramdisk_start,
            vendor_ramdisk_size,
            "vendor_ramdisk",
            bytes.len(),
        )?;
        let dtb_start = align_up(vendor_ramdisk_end as u32, page_size)?;
        let dtb_end = section_end(dtb_start, dtb_size, "dtb", bytes.len())?;
        let table_start = align_up(dtb_end as u32, page_size)?;
        let table_end = section_end(table_start, table_size, "vendor_ramdisk_table", bytes.len())?;
        let bootconfig_start = align_up(table_end as u32, page_size)?;
        let bootconfig_end =
            section_end(bootconfig_start, bootconfig_size, "bootconfig", bytes.len())?;

        Ok(Self {
            version,
            page_size,
            cmdline,
            name,
            vendor_ramdisk: bytes[vendor_ramdisk_start..vendor_ramdisk_end].to_vec(),
            dtb: bytes[dtb_start..dtb_end].to_vec(),
            ramdisk_table: bytes[table_start..table_end].to_vec(),
            bootconfig: bytes[bootconfig_start..bootconfig_end].to_vec(),
        })
    }
}

/// The kernel, initrd and command line QEMU's `-kernel`/`-initrd`/`-append` need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmulatedBootAssets {
    pub kernel: Vec<u8>,
    pub initrd: Vec<u8>,
    pub kernel_cmdline: String,
}

/// Assembles the direct-kernel boot assets.
///
/// `bootconfig` entries are written as a bootconfig trailer appended to the initrd, which is how
/// Android 12+ GKI kernels learn `androidboot.*` keys. The `bootconfig` token is added to the
/// command line when it is missing, because the kernel only looks for the trailer when asked.
pub fn assemble(
    boot: &BootImage,
    init_boot: Option<&BootImage>,
    vendor_boot: &VendorBootImage,
    bootconfig: &[&str],
) -> Result<EmulatedBootAssets, BootImageError> {
    if boot.kernel.is_empty() {
        return Err(BootImageError::EmptyKernel);
    }
    if vendor_boot.vendor_ramdisk.is_empty() {
        return Err(BootImageError::EmptyVendorRamdisk);
    }
    let mut initrd = Vec::new();
    initrd.extend_from_slice(&vendor_boot.vendor_ramdisk);
    if let Some(init_boot) = init_boot {
        initrd.extend_from_slice(&init_boot.ramdisk);
    }
    let mut cmdline = merge_cmdline(&vendor_boot.cmdline, &boot.cmdline);
    if !bootconfig.is_empty() {
        initrd.extend_from_slice(&bootconfig_block(bootconfig)?);
        if !cmdline
            .split_whitespace()
            .any(|token| token == "bootconfig")
        {
            if !cmdline.is_empty() {
                cmdline.push(' ');
            }
            cmdline.push_str("bootconfig");
        }
    }
    Ok(EmulatedBootAssets {
        kernel: boot.kernel.clone(),
        initrd,
        kernel_cmdline: cmdline,
    })
}

/// Builds the bootconfig trailer the kernel expects at the end of the initrd.
///
/// Layout: the parameters as `key=value\n` lines, padded to a four-byte boundary with `#`, then a
/// little-endian `size` and `checksum`. The kernel accepts the block only when the sum of every
/// 32-bit word, including `size` and `checksum`, is zero.
pub fn bootconfig_block(params: &[&str]) -> Result<Vec<u8>, BootImageError> {
    let mut data = Vec::new();
    for (index, param) in params.iter().enumerate() {
        let valid = !param.is_empty()
            && param.len() <= 256
            && param
                .chars()
                .all(|ch| ch.is_ascii_graphic() || ch == '=' || ch == ' ');
        if !valid || param.contains('\n') {
            return Err(BootImageError::InvalidBootConfigParameter { index });
        }
        data.extend_from_slice(param.as_bytes());
        data.push(b'\n');
    }
    while data.len() % 4 != 0 {
        data.push(BOOTCONFIG_PAD);
    }
    if data.len() > MAX_BOOTCONFIG_BYTES {
        return Err(BootImageError::BootConfigTooLarge);
    }
    let size = data.len() as u32;
    let mut sum = size;
    for chunk in data.chunks_exact(4) {
        sum = sum.wrapping_add(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    let checksum = 0u32.wrapping_sub(sum);
    data.extend_from_slice(BOOTCONFIG_MAGIC);
    data.extend_from_slice(&size.to_le_bytes());
    data.extend_from_slice(&checksum.to_le_bytes());
    Ok(data)
}

/// The bootconfig trailer: 11-byte magic, 4-byte size, 4-byte checksum.
pub const BOOTCONFIG_TRAILER_BYTES: usize = 20;

fn merge_cmdline(vendor: &str, boot: &str) -> String {
    let mut merged: Vec<&str> = Vec::new();
    for token in vendor.split_whitespace().chain(boot.split_whitespace()) {
        if !merged.contains(&token) {
            merged.push(token);
        }
    }
    merged.join(" ")
}

fn parse_cmdline(bytes: &[u8]) -> Result<String, BootImageError> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    let text = std::str::from_utf8(&bytes[..end]).map_err(|_| BootImageError::InvalidCmdline)?;
    if text.chars().any(|ch| ch.is_control() && ch != '\t') {
        return Err(BootImageError::InvalidCmdline);
    }
    if text.len() > MAX_CMDLINE_BYTES {
        return Err(BootImageError::InvalidCmdline);
    }
    Ok(text.trim().to_owned())
}

fn parse_name(bytes: &[u8]) -> Result<String, BootImageError> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..end])
        .map(str::to_owned)
        .map_err(|_| BootImageError::InvalidCmdline)
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn align_up(value: u32, alignment: u32) -> Result<usize, BootImageError> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(BootImageError::BadPageSize(alignment));
    }
    let aligned = value
        .checked_add(alignment - 1)
        .ok_or(BootImageError::SectionOutOfBounds { section: "offset" })?
        & !(alignment - 1);
    usize::try_from(aligned).map_err(|_| BootImageError::SectionOutOfBounds { section: "offset" })
}

fn section_end(
    start: usize,
    size: u32,
    section: &'static str,
    total: usize,
) -> Result<usize, BootImageError> {
    let size = size as usize;
    if size > MAX_SECTION_BYTES {
        return Err(BootImageError::SectionTooLarge { section });
    }
    let end = start
        .checked_add(size)
        .ok_or(BootImageError::SectionOutOfBounds { section })?;
    if end > total {
        return Err(BootImageError::SectionOutOfBounds { section });
    }
    Ok(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_align(bytes: &mut Vec<u8>) {
        while bytes.len() % BOOT_PAGE_SIZE != 0 {
            bytes.push(0);
        }
    }

    fn boot_image_v4(kernel: &[u8], ramdisk: &[u8], cmdline: &str) -> Vec<u8> {
        let mut image = vec![0u8; BOOT_HEADER_V4_SIZE as usize];
        image[0..8].copy_from_slice(BOOT_MAGIC);
        image[8..12].copy_from_slice(&(kernel.len() as u32).to_le_bytes());
        image[12..16].copy_from_slice(&(ramdisk.len() as u32).to_le_bytes());
        image[20..24].copy_from_slice(&BOOT_HEADER_V4_SIZE.to_le_bytes());
        image[40..44].copy_from_slice(&4u32.to_le_bytes());
        image[44..44 + cmdline.len()].copy_from_slice(cmdline.as_bytes());
        image[1580..1584].copy_from_slice(&0u32.to_le_bytes());
        page_align(&mut image);
        image.extend_from_slice(kernel);
        page_align(&mut image);
        image.extend_from_slice(ramdisk);
        page_align(&mut image);
        image
    }

    fn vendor_boot_v4(vendor_ramdisk: &[u8], bootconfig: &[u8], cmdline: &str) -> Vec<u8> {
        let mut image = vec![0u8; VENDOR_BOOT_HEADER_V4_SIZE as usize];
        image[0..8].copy_from_slice(VENDOR_BOOT_MAGIC);
        image[8..12].copy_from_slice(&4u32.to_le_bytes());
        image[12..16].copy_from_slice(&4096u32.to_le_bytes());
        image[24..28].copy_from_slice(&(vendor_ramdisk.len() as u32).to_le_bytes());
        image[28..28 + cmdline.len()].copy_from_slice(cmdline.as_bytes());
        image[2080..2086].copy_from_slice(b"cvd1\0\0");
        image[2096..2100].copy_from_slice(&VENDOR_BOOT_HEADER_V4_SIZE.to_le_bytes());
        image[2112..2116].copy_from_slice(&0u32.to_le_bytes());
        image[2116..2120].copy_from_slice(&0u32.to_le_bytes());
        image[2120..2124].copy_from_slice(&0u32.to_le_bytes());
        image[2124..2128].copy_from_slice(&(bootconfig.len() as u32).to_le_bytes());
        page_align(&mut image);
        image.extend_from_slice(vendor_ramdisk);
        page_align(&mut image);
        image.extend_from_slice(bootconfig);
        page_align(&mut image);
        image
    }

    #[test]
    fn parses_boot_v4_and_vendor_boot_v4() {
        let boot = boot_image_v4(b"kernel", b"ramdisk", "console=ttyS0");
        let boot = BootImage::parse(&boot).unwrap();
        assert_eq!(boot.version, 4);
        assert_eq!(boot.kernel, b"kernel");
        assert_eq!(boot.ramdisk, b"ramdisk");
        assert_eq!(boot.cmdline, "console=ttyS0");

        let vendor = vendor_boot_v4(b"vendor-ramdisk", b"", "androidboot.slot_suffix=_a");
        let vendor = VendorBootImage::parse(&vendor).unwrap();
        assert_eq!(vendor.vendor_ramdisk, b"vendor-ramdisk");
        assert_eq!(vendor.name, "cvd1");
        assert_eq!(vendor.cmdline, "androidboot.slot_suffix=_a");
        assert!(vendor.bootconfig.is_empty());
    }

    #[test]
    fn assembles_vendor_then_init_boot_then_bootconfig() {
        let boot = BootImage::parse(&boot_image_v4(b"kernel", b"boot-ramdisk", "")).unwrap();
        let init_boot = BootImage::parse(&boot_image_v4(b"", b"init-ramdisk", "")).unwrap();
        let vendor = VendorBootImage::parse(&vendor_boot_v4(
            b"vendor-ramdisk",
            b"",
            "androidboot.slot_suffix=_a",
        ))
        .unwrap();
        let assets = assemble(
            &boot,
            Some(&init_boot),
            &vendor,
            &["androidboot.hw=emulated"],
        )
        .unwrap();
        assert_eq!(assets.kernel, b"kernel");
        assert_eq!(&assets.initrd[..14], b"vendor-ramdisk");
        assert_eq!(&assets.initrd[14..26], b"init-ramdisk");
        assert_eq!(
            &assets.initrd[26..26 + b"androidboot.hw=emulated\n".len()],
            b"androidboot.hw=emulated\n"
        );
        let magic_start = assets.initrd.len() - BOOTCONFIG_TRAILER_BYTES;
        assert_eq!(
            &assets.initrd[magic_start..magic_start + 12],
            BOOTCONFIG_MAGIC
        );
        assert!(assets.kernel_cmdline.ends_with("bootconfig"));
        assert!(
            assets
                .kernel_cmdline
                .starts_with("androidboot.slot_suffix=_a")
        );
    }

    #[test]
    fn bootconfig_trailer_sums_to_zero() {
        let block = bootconfig_block(&["androidboot.a=1", "androidboot.b=22"]).unwrap();
        let size = u32::from_le_bytes(block[block.len() - 8..block.len() - 4].try_into().unwrap());
        assert_eq!(size as usize % 4, 0);
        let data = &block[..size as usize];
        let mut sum = 0u32;
        for chunk in data.chunks_exact(4) {
            sum = sum.wrapping_add(u32::from_le_bytes(chunk.try_into().unwrap()));
        }
        sum = sum.wrapping_add(size);
        let checksum = u32::from_le_bytes(block[block.len() - 4..].try_into().unwrap());
        sum = sum.wrapping_add(checksum);
        assert_eq!(sum, 0);
        let magic_start = block.len() - BOOTCONFIG_TRAILER_BYTES;
        assert_eq!(&block[magic_start..magic_start + 12], BOOTCONFIG_MAGIC);
    }

    #[test]
    fn rejects_malformed_images() {
        assert_eq!(
            BootImage::parse(b"ANDROID!").unwrap_err(),
            BootImageError::TooSmall {
                needed: BOOT_HEADER_V3_SIZE as usize,
                got: 8
            }
        );
        let mut image = boot_image_v4(b"kernel", b"ramdisk", "");
        image[0..8].copy_from_slice(b"NOTMAGIC");
        assert_eq!(
            BootImage::parse(&image).unwrap_err(),
            BootImageError::BadMagic {
                expected: "ANDROID!"
            }
        );
        let mut image = boot_image_v4(b"kernel", b"ramdisk", "");
        image[40..44].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(
            BootImage::parse(&image).unwrap_err(),
            BootImageError::UnsupportedHeaderVersion(2)
        );
        let mut image = boot_image_v4(b"kernel", b"ramdisk", "");
        // Claim a kernel far larger than the buffer.
        image[8..12].copy_from_slice(&(MAX_SECTION_BYTES as u32 + 1).to_le_bytes());
        assert_eq!(
            BootImage::parse(&image).unwrap_err(),
            BootImageError::SectionTooLarge { section: "kernel" }
        );
        let image = boot_image_v4(b"", b"ramdisk", "");
        let boot = BootImage::parse(&image).unwrap();
        let vendor = VendorBootImage::parse(&vendor_boot_v4(b"vendor", b"", "")).unwrap();
        assert_eq!(
            assemble(&boot, None, &vendor, &[]).unwrap_err(),
            BootImageError::EmptyKernel
        );
    }

    #[test]
    fn refuses_bootconfig_parameters_that_cannot_be_written() {
        assert_eq!(
            bootconfig_block(&["androidboot.a=1\nandroidboot.b=2"]).unwrap_err(),
            BootImageError::InvalidBootConfigParameter { index: 0 }
        );
        assert_eq!(
            bootconfig_block(&[""]).unwrap_err(),
            BootImageError::InvalidBootConfigParameter { index: 0 }
        );
    }
}
