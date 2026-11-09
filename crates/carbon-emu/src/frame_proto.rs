//! The frame/input socket wire format between QEMU's `android` UI backend and the `:vm` service.
//!
//! One `SOCK_SEQPACKET` socketpair carries display control messages from QEMU to the app, and
//! back-pressure (`RELEASE`) plus resize (`RESIZE`) from the app to QEMU. The `AHardwareBuffer`
//! handle that follows a `FRAME` is *not* part of this byte format: it is sent with
//! `AHardwareBuffer_sendHandleToUnixSocket`, and an optional fence fd follows as `SCM_RIGHTS`.
//!
//! The format is fixed-size, little-endian and versioned. Anything that does not decode exactly
//! is dropped by the reader, never guessed at: QEMU is treated as untrusted input to the service.

use std::fmt;

/// Bumped only for an incompatible change; QEMU's `HELLO` must carry the same value.
pub const PROTOCOL_VERSION: u16 = 1;
/// `b"CHVF"`, little-endian.
const MAGIC: u32 = 0x4348_5646;
const HEADER_BYTES: usize = 12;
/// No display control message needs a larger payload.
pub const MAX_PAYLOAD_BYTES: usize = 4096;
/// Display coordinates are bounded by the largest mode the engine accepts.
pub const MAX_DIMENSION: u32 = 16_384;
/// AHardwareBuffer formats are small enum values.
const MAX_FORMAT: u32 = 1024;

pub const KIND_HELLO: u16 = 1;
pub const KIND_MODE: u16 = 2;
pub const KIND_FRAME: u16 = 3;
pub const KIND_CURSOR: u16 = 4;
pub const KIND_RELEASE: u16 = 0x81;
pub const KIND_RESIZE: u16 = 0x82;

/// `FRAME.flags` bit: an acquire fence fd follows the buffer handle.
pub const FRAME_FLAG_HAS_FENCE: u32 = 1 << 0;
const KNOWN_FRAME_FLAGS: u32 = FRAME_FLAG_HAS_FENCE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameDescriptor {
    /// Monotonic, non-zero. A `RELEASE` echoes it back so QEMU can reuse that buffer.
    pub seq: u32,
    pub width: u32,
    pub height: u32,
    /// `AHardwareBuffer_Format` value.
    pub format: u32,
    /// `AHardwareBuffer_UsageFlags` bits.
    pub usage: u64,
    pub has_fence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// QEMU → app, first message on the frame socket.
    Hello {
        version: u16,
    },
    /// QEMU → app, a new display mode. Clears the frame ring.
    Mode {
        width: u32,
        height: u32,
        format: u32,
    },
    /// QEMU → app; the buffer handle follows out of band.
    Frame(FrameDescriptor),
    Cursor {
        x: u32,
        y: u32,
        visible: bool,
    },
    /// App → QEMU: the buffer for `seq` may be reused.
    Release {
        seq: u32,
    },
    /// App → QEMU: the surface changed size.
    Resize {
        width: u32,
        height: u32,
        dpi: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    TooShort {
        needed: usize,
        got: usize,
    },
    PayloadTooLarge(usize),
    BadMagic(u32),
    UnsupportedVersion(u16),
    UnknownKind(u16),
    WrongLength {
        kind: u16,
        expected: usize,
        found: usize,
    },
    ReservedNotZero(u16),
    InvalidField(&'static str),
    /// A message arrived on the socket for the wrong direction.
    WrongDirection(u16),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Message {
    pub fn kind(&self) -> u16 {
        match self {
            Message::Hello { .. } => KIND_HELLO,
            Message::Mode { .. } => KIND_MODE,
            Message::Frame(_) => KIND_FRAME,
            Message::Cursor { .. } => KIND_CURSOR,
            Message::Release { .. } => KIND_RELEASE,
            Message::Resize { .. } => KIND_RESIZE,
        }
    }

    /// QEMU is the only writer of the display stream and the only reader of `RELEASE`/`RESIZE`.
    pub fn is_from_qemu(&self) -> bool {
        matches!(
            self,
            Message::Hello { .. }
                | Message::Mode { .. }
                | Message::Frame(_)
                | Message::Cursor { .. }
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        let payload = self.payload();
        let mut out = Vec::with_capacity(HEADER_BYTES + payload.len());
        out.extend_from_slice(&MAGIC.to_le_bytes());
        out.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
        out.extend_from_slice(&self.kind().to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    /// Decodes one datagram. `from_qemu` enforces the stream direction.
    pub fn decode(bytes: &[u8], from_qemu: bool) -> Result<Self, DecodeError> {
        if bytes.len() < HEADER_BYTES {
            return Err(DecodeError::TooShort {
                needed: HEADER_BYTES,
                got: bytes.len(),
            });
        }
        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != MAGIC {
            return Err(DecodeError::BadMagic(magic));
        }
        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        if version != PROTOCOL_VERSION {
            return Err(DecodeError::UnsupportedVersion(version));
        }
        let kind = u16::from_le_bytes(bytes[6..8].try_into().unwrap());
        let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        if length > MAX_PAYLOAD_BYTES {
            return Err(DecodeError::PayloadTooLarge(length));
        }
        if bytes.len() != HEADER_BYTES + length {
            return Err(DecodeError::TooShort {
                needed: HEADER_BYTES + length,
                got: bytes.len(),
            });
        }
        let payload = &bytes[HEADER_BYTES..];
        let message = match kind {
            KIND_HELLO => {
                expect(kind, payload, 4)?;
                let version = u16::from_le_bytes(payload[0..2].try_into().unwrap());
                let reserved = u16::from_le_bytes(payload[2..4].try_into().unwrap());
                if reserved != 0 {
                    return Err(DecodeError::ReservedNotZero(kind));
                }
                if version != PROTOCOL_VERSION {
                    return Err(DecodeError::UnsupportedVersion(version));
                }
                Message::Hello { version }
            }
            KIND_MODE => {
                expect(kind, payload, 12)?;
                let width = u32(payload, 0);
                let height = u32(payload, 4);
                let format = u32(payload, 8);
                check_dimensions(width, height)?;
                check_format(format)?;
                Message::Mode {
                    width,
                    height,
                    format,
                }
            }
            KIND_FRAME => {
                expect(kind, payload, 28)?;
                let descriptor = FrameDescriptor {
                    seq: u32(payload, 0),
                    width: u32(payload, 4),
                    height: u32(payload, 8),
                    format: u32(payload, 12),
                    usage: u64(payload, 16),
                    has_fence: {
                        let flags = u32(payload, 24);
                        if flags & !KNOWN_FRAME_FLAGS != 0 {
                            return Err(DecodeError::ReservedNotZero(kind));
                        }
                        flags & FRAME_FLAG_HAS_FENCE != 0
                    },
                };
                if descriptor.seq == 0 {
                    return Err(DecodeError::InvalidField("frame.seq"));
                }
                check_dimensions(descriptor.width, descriptor.height)?;
                check_format(descriptor.format)?;
                Message::Frame(descriptor)
            }
            KIND_CURSOR => {
                expect(kind, payload, 12)?;
                let x = u32(payload, 0);
                let y = u32(payload, 4);
                let visible = u32(payload, 8);
                if visible > 1 {
                    return Err(DecodeError::InvalidField("cursor.visible"));
                }
                Message::Cursor {
                    x,
                    y,
                    visible: visible == 1,
                }
            }
            KIND_RELEASE => {
                expect(kind, payload, 4)?;
                let seq = u32(payload, 0);
                if seq == 0 {
                    return Err(DecodeError::InvalidField("release.seq"));
                }
                Message::Release { seq }
            }
            KIND_RESIZE => {
                expect(kind, payload, 12)?;
                let width = u32(payload, 0);
                let height = u32(payload, 4);
                let dpi = u32(payload, 8);
                check_dimensions(width, height)?;
                if dpi == 0 || dpi > 10_000 {
                    return Err(DecodeError::InvalidField("resize.dpi"));
                }
                Message::Resize { width, height, dpi }
            }
            other => return Err(DecodeError::UnknownKind(other)),
        };
        if from_qemu != message.is_from_qemu() {
            return Err(DecodeError::WrongDirection(kind));
        }
        Ok(message)
    }

    fn payload(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Message::Hello { version } => {
                out.extend_from_slice(&version.to_le_bytes());
                out.extend_from_slice(&0u16.to_le_bytes());
            }
            Message::Mode {
                width,
                height,
                format,
            } => {
                out.extend_from_slice(&width.to_le_bytes());
                out.extend_from_slice(&height.to_le_bytes());
                out.extend_from_slice(&format.to_le_bytes());
            }
            Message::Frame(frame) => {
                out.extend_from_slice(&frame.seq.to_le_bytes());
                out.extend_from_slice(&frame.width.to_le_bytes());
                out.extend_from_slice(&frame.height.to_le_bytes());
                out.extend_from_slice(&frame.format.to_le_bytes());
                out.extend_from_slice(&frame.usage.to_le_bytes());
                out.extend_from_slice(
                    &(if frame.has_fence {
                        FRAME_FLAG_HAS_FENCE
                    } else {
                        0
                    })
                    .to_le_bytes(),
                );
            }
            Message::Cursor { x, y, visible } => {
                out.extend_from_slice(&x.to_le_bytes());
                out.extend_from_slice(&y.to_le_bytes());
                out.extend_from_slice(&u32::from(*visible).to_le_bytes());
            }
            Message::Release { seq } => out.extend_from_slice(&seq.to_le_bytes()),
            Message::Resize { width, height, dpi } => {
                out.extend_from_slice(&width.to_le_bytes());
                out.extend_from_slice(&height.to_le_bytes());
                out.extend_from_slice(&dpi.to_le_bytes());
            }
        }
        out
    }
}

fn expect(kind: u16, payload: &[u8], expected: usize) -> Result<(), DecodeError> {
    if payload.len() != expected {
        return Err(DecodeError::WrongLength {
            kind,
            expected,
            found: payload.len(),
        });
    }
    Ok(())
}

fn u32(payload: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(payload[offset..offset + 4].try_into().unwrap())
}

fn u64(payload: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(payload[offset..offset + 8].try_into().unwrap())
}

fn check_dimensions(width: u32, height: u32) -> Result<(), DecodeError> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(DecodeError::InvalidField("dimensions"));
    }
    Ok(())
}

fn check_format(format: u32) -> Result<(), DecodeError> {
    if format == 0 || format > MAX_FORMAT {
        return Err(DecodeError::InvalidField("format"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Message {
        Message::Frame(FrameDescriptor {
            seq: 7,
            width: 1080,
            height: 2392,
            format: 1,
            usage: 0x0000_0200,
            has_fence: true,
        })
    }

    #[test]
    fn golden_bytes_for_each_message() {
        let hello = Message::Hello { version: 1 };
        assert_eq!(
            hello.encode(),
            vec![
                0x46, 0x56, 0x48, 0x43, // "CHVF"
                0x01, 0x00, // version
                0x01, 0x00, // kind
                0x04, 0x00, 0x00, 0x00, // length
                0x01, 0x00, 0x00, 0x00, // payload
            ]
        );
        assert_eq!(
            Message::Hello { version: 1 }.encode().len(),
            HEADER_BYTES + 4
        );
        for message in [
            Message::Mode {
                width: 1080,
                height: 2392,
                format: 1,
            },
            frame(),
            Message::Cursor {
                x: 10,
                y: 20,
                visible: true,
            },
            Message::Release { seq: 7 },
            Message::Resize {
                width: 720,
                height: 1280,
                dpi: 320,
            },
        ] {
            let encoded = message.encode();
            let decoded = Message::decode(&encoded, message.is_from_qemu()).unwrap();
            assert_eq!(decoded, message);
        }
    }

    #[test]
    fn frame_descriptor_round_trips_with_fence_flag() {
        let Message::Frame(frame) = frame() else {
            unreachable!()
        };
        assert!(frame.has_fence);
        assert_eq!(frame.usage, 0x200);
    }

    #[test]
    fn truncated_messages_never_decode() {
        for message in [
            frame(),
            Message::Release { seq: 3 },
            Message::Mode {
                width: 16,
                height: 16,
                format: 1,
            },
        ] {
            let encoded = message.encode();
            for length in 0..encoded.len() {
                assert!(
                    Message::decode(&encoded[..length], message.is_from_qemu()).is_err(),
                    "prefix {length} of {message:?} decoded"
                );
            }
        }
    }

    #[test]
    fn rejects_bad_magic_version_length_and_direction() {
        let mut encoded = frame().encode();
        encoded[0] = 0;
        assert!(matches!(
            Message::decode(&encoded, true),
            Err(DecodeError::BadMagic(_))
        ));
        let mut encoded = frame().encode();
        encoded[4] = 2;
        assert_eq!(
            Message::decode(&encoded, true),
            Err(DecodeError::UnsupportedVersion(2))
        );
        let mut encoded = frame().encode();
        encoded[11] = 0xff;
        assert!(matches!(
            Message::decode(&encoded, true),
            Err(DecodeError::TooShort { .. }) | Err(DecodeError::PayloadTooLarge(_))
        ));
        assert_eq!(
            Message::decode(&frame().encode(), false),
            Err(DecodeError::WrongDirection(KIND_FRAME))
        );
        assert_eq!(
            Message::decode(&Message::Release { seq: 1 }.encode(), true),
            Err(DecodeError::WrongDirection(KIND_RELEASE))
        );
    }

    #[test]
    fn rejects_oversize_geometry_and_unknown_flags() {
        let mut encoded = frame().encode();
        // width = 0
        encoded[HEADER_BYTES + 4..HEADER_BYTES + 8].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(
            Message::decode(&encoded, true),
            Err(DecodeError::InvalidField("dimensions"))
        );
        let mut encoded = frame().encode();
        let flags_offset = HEADER_BYTES + 24;
        encoded[flags_offset..flags_offset + 4].copy_from_slice(&0b10u32.to_le_bytes());
        assert_eq!(
            Message::decode(&encoded, true),
            Err(DecodeError::ReservedNotZero(KIND_FRAME))
        );
        let mut encoded = Message::Mode {
            width: MAX_DIMENSION + 1,
            height: 10,
            format: 1,
        }
        .encode();
        assert_eq!(
            Message::decode(&encoded, true),
            Err(DecodeError::InvalidField("dimensions"))
        );
        encoded = Message::Mode {
            width: 10,
            height: 10,
            format: 0,
        }
        .encode();
        assert_eq!(
            Message::decode(&encoded, true),
            Err(DecodeError::InvalidField("format"))
        );
    }

    #[test]
    fn frame_sequence_zero_is_refused() {
        let mut encoded = frame().encode();
        encoded[HEADER_BYTES..HEADER_BYTES + 4].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(
            Message::decode(&encoded, true),
            Err(DecodeError::InvalidField("frame.seq"))
        );
    }

    /// The bytes QEMU's `-display android` backend emits for the same messages, pinned by
    /// `qualification/emu/frame-proto/golden-messages.txt`. The C test in that directory proves
    /// the producer writes these bytes; this test proves the reader understands them and agrees
    /// about every field. A change to either side fails here, in one place.
    #[test]
    fn agrees_with_the_qemu_golden_messages() {
        let file = include_str!("../../../qualification/emu/frame-proto/golden-messages.txt");
        let mut seen = 0;

        for line in file.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let (name, hex) = line
                .split_once(' ')
                .unwrap_or_else(|| panic!("malformed golden line: {line}"));
            let bytes: Vec<u8> = (0..hex.len() / 2)
                .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("hex"))
                .collect();
            let expected = match name {
                "hello" => Message::Hello { version: 1 },
                "mode_640x480" => Message::Mode {
                    width: 640,
                    height: 480,
                    format: 1,
                },
                "mode_1080x2400" => Message::Mode {
                    width: 1080,
                    height: 2400,
                    format: 1,
                },
                "frame_seq1" => Message::Frame(FrameDescriptor {
                    seq: 1,
                    width: 640,
                    height: 480,
                    format: 1,
                    usage: 0x130,
                    has_fence: false,
                }),
                "frame_seq3_with_fence" => Message::Frame(FrameDescriptor {
                    seq: 3,
                    width: 1080,
                    height: 2400,
                    format: 1,
                    usage: 0x130,
                    has_fence: true,
                }),
                "cursor_visible" => Message::Cursor {
                    x: 12,
                    y: 34,
                    visible: true,
                },
                "cursor_hidden" => Message::Cursor {
                    x: 0,
                    y: 0,
                    visible: false,
                },
                other => panic!("golden message {other} has no expectation in this test"),
            };

            assert_eq!(bytes.len(), hex.len() / 2, "{name}: odd-length hex");
            assert_eq!(
                Message::decode(&bytes, true),
                Ok(expected.clone()),
                "{name}: decoder disagrees with the QEMU producer"
            );
            assert_eq!(
                expected.encode(),
                bytes,
                "{name}: re-encoding does not reproduce the golden bytes"
            );
            seen += 1;
        }

        assert_eq!(seen, 7, "one or more golden messages were skipped");
    }
}
