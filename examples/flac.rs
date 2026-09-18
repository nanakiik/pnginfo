use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::sync::OnceLock;

static SAMPLE_RATE: OnceLock<u32> = OnceLock::new();
static BIT_DEPTH: OnceLock<u8> = OnceLock::new();

// https://datatracker.ietf.org/doc/html/rfc9639/#section-5
// #name-format-principles
// All numbers used in a FLAC bitstream are integers
// All numbers are big-endian coded
// All samples encoded to and decoded from the FLAC format MUST be in a signed representation.
// Unary coding in a FLAC bitstream is done with zero bits terminated with a one bit
//

// https://datatracker.ietf.org/doc/html/rfc9639/#section-6
// name-format-layout-overview
pub const SIGNATURE: &[u8; 4] = b"fLaC";

// https://datatracker.ietf.org/doc/html/rfc9639/#section-7
// name-streamable-subset

// https://datatracker.ietf.org/doc/html/rfc9639/#section-8
// name-file-level-metadata
// The first metadata block MUST be a streaminfo metadata block.

// https://datatracker.ietf.org/doc/html/rfc9639/#section-8.1
// name-metadata-block-header
pub struct MetadataBlock<'a> {
    // u(1)
    pub is_lat: bool,
    // u(7)
    pub metadata_block_type: MetadataBlockType,
    // u(24) excluding the 4 header bytes, as an unsigned number coded big-endian.
    pub metadata_block_size: u32,
    // u(8 * n)
    pub metadata_block_data: &'a [u8],
}
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataBlockType {
    Streaminfo,
    Padding,
    Application,
    Seektable,
    Vorbiscomment,
    Cuesheet,
    Picture,
    Reserved,        // 7 - 126
    Forbidden = 127, // Forbidden (to avoid confusion with a frame sync code)
}
impl MetadataBlockType {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Streaminfo,
            1 => Self::Padding,
            2 => Self::Application,
            3 => Self::Seektable,
            4 => Self::Vorbiscomment,
            5 => Self::Cuesheet,
            6 => Self::Picture,
            7..=126 => Self::Reserved,
            _ => unreachable!(),
        }
    }
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-8.2
// name-streaminfo
pub struct StreamInfo {
    // u(16) The minimum block size (in samples) used in the stream, excluding the last block.
    pub minimum_block_size: u16,
    // u(16) The maximum block size (in samples) used in the stream.
    pub maximum_block_size: u16,
    // u(24) The minimum frame size (in bytes) used in the stream.
    // A value of 0 signifies that the value is not known.
    pub minimum_frame_size: u32,
    // u(24) The maximum frame size (in bytes) used in the stream.
    // A value of 0 signifies that the value is not known.
    pub maximum_frame_size: u32,
    // u(20) Sample rate in Hz.
    pub sample_rate: u32,
    // u(3) (number of channels)-1. FLAC supports from 1 to 8 channels.
    pub channels: u8,
    // u(5) (bits per sample)-1. FLAC supports from 4 to 32 bits per sample.
    pub bits_per_sample: u8,
    // u(36) Total number of interchannel samples in the stream.
    // A value of 0 here means the number of len samples is unknown.
    pub total_number_samples: u64,
    // u(128) MD5 checksum of the unencoded audio data.
    // A value of 0 signifies that the value is not known.
    pub checksum: [u8; 16],
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-8.4
// name-padding
pub struct Padding<'a> {
    // u(n) n is 8 times the size described in the metadata block header.
    pub padding: &'a [u8],
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-8.4
// name-application
// The only mandatory field is a 32-bit application identifier (application ID).
pub struct Application<'a> {
    // u(32) Registered application ID.
    pub id: u32,
    // u(n)  n MUST be a multiple of 8, i.e., a whole number of bytes
    pub data: &'a [u8],
}
// https://datatracker.ietf.org/doc/html/rfc9639/#section-12.2
// name-flac-application-metadata-b
#[allow(non_upper_case_globals)]
pub mod application_id {
    // FlacFile
    pub const ATCH: u32 = 0x4154_4348;
    // beSolo
    pub const BSOL: u32 = 0x4253_4F4C;
    // Bugs Player
    pub const BUGS: u32 = 0x4255_4753;
    // GoldWave cue points
    pub const Cues: u32 = 0x4375_6573;
    // CUE Splitter
    pub const Fica: u32 = 0x4669_6361;
    // flac-tools
    pub const Ftol: u32 = 0x4674_6F6C;
    // MOTB MetaCzar
    pub const MOTB: u32 = 0x4D4F_5442;
    // MP3 Stream Editor
    pub const MPSE: u32 = 0x4D50_5345;
    // MusicML: Music Metadata Language
    pub const MuML: u32 = 0x4D75_4D4C;
    // Sound Devices RIFF chunk storage
    pub const RIFF: u32 = 0x5249_4646;
    // Sound Font FLAC
    pub const SFFL: u32 = 0x5346_464C;
    // Sony Creative Software
    pub const SONY: u32 = 0x534F_4E59;
    // flacsqueeze
    pub const SQEZ: u32 = 0x5351_455A;
    // TwistedWave
    pub const TtWv: u32 = 0x5474_5776;
    // UITS Embedding tools
    pub const UITS: u32 = 0x5549_5453;
    // FLAC AIFF chunk storage
    pub const aiff: u32 = 0x6169_6666;
    // flac-image
    pub const imag: u32 = 0x696D_6167;
    // Parseable Embedded Extensible Metadata
    pub const peem: u32 = 0x7065_656D;
    // QFLAC Studio
    pub const qfst: u32 = 0x7166_7374;
    // FLAC RIFF chunk storage
    pub const riff: u32 = 0x7269_6666;
    // TagTuner
    pub const tune: u32 = 0x7475_6E65;
    // FLAC Wave64 chunk storage
    pub const w64: u32 = 0x7736_3420;
    // XBAT
    pub const xbat: u32 = 0x7862_6174;
    // xmcd
    pub const xmcd: u32 = 0x786D_6364;
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-8.5
// name-seek-table
pub struct SeekTable<'a> {
    pub seek_points: &'a [Point],
}
// https://datatracker.ietf.org/doc/html/rfc9639/#section-8.5.1
// name-seek-point
pub struct Point {
    // u(64) Sample number of the first sample in the target frame or 0xFFFFFFFFFFFFFFFF for a placeholder point.
    pub sample: u64,
    // Offset (in bytes) from the first byte of the first frame header to the first byte of the target frame's header.
    pub offset: u64,
    // Number of samples in the target frame.
    pub number: u16,
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-8.6
// name-vorbis-
// A FLAC file MUST NOT contain more than one Vorbis comment metadata block.
pub struct VorbisComment<'a> {
    pub length: u32,
    pub vendor_string: &'a [u8],
    pub field_number: u32,
    pub fields: &'a [Field<'a>],
}
pub struct Field<'a> {
    pub number: u32,
    pub field: &'a [u8],
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-8.6.2
// name-channel-mask
pub mod name_channel_mask {
    pub const WAVEFORMATEXTENSIBLE_CHANNEL_MASK: u16 = 0;
    pub const FRONT_LEFT: u32 = 1 << 0;
    pub const FRONT_RIGHT: u32 = 1 << 1;
    pub const FRONT_CENTER: u32 = 1 << 2;
    pub const LFE: u32 = 1 << 3;
    pub const BACK_LEFT: u32 = 1 << 4;
    pub const BACK_RIGHT: u32 = 1 << 5;
    pub const FRONT_LEFT_OF_CENTER: u32 = 1 << 6;
    pub const FRONT_RIGHT_OF_CENTER: u32 = 1 << 7;
    pub const BACK_CENTER: u32 = 1 << 8;
    pub const SIDE_LEFT: u32 = 1 << 9;
    pub const SIDE_RIGHT: u32 = 1 << 10;
    pub const TOP_CENTER: u32 = 1 << 11;
    pub const TOP_FRONT_LEFT: u32 = 1 << 12;
    pub const TOP_FRONT_CENTER: u32 = 1 << 13;
    pub const TOP_FRONT_RIGHT: u32 = 1 << 14;
    pub const TOP_REAR_LEFT: u32 = 1 << 15;
    pub const TOP_REAR_CENTER: u32 = 1 << 16;
    pub const TOP_REAR_RIGHT: u32 = 1 << 17;
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-8.7
// name-cuesheet
// Compact Disc Digital Audio (CD-DA)
pub struct Cuesheet {
    // u(128*8) Media catalog number in ASCII printable characters 0x20-0x7E.
    pub media_catalog_number: [u8; 128],
    // u(64) Number of lead-in samples.
    pub number_of_samples: u64,
    // 1 if the cuesheet corresponds to a CD-DA; else 0.
    pub is_cd_da: bool,
    // u(7+258*8) Reserved. All bits MUST be set to zero.
    pub reserved: (),
    // u(8) Number of tracks in this cuesheet.
    pub number_of_tracks: u8,
    // Cuesheet tracks
    pub number_of_structures: u8,
}
// https://datatracker.ietf.org/doc/html/rfc9639/#section-8.7.1
// name-cuesheet-track
pub struct CuesheetTrack {
    // u(64) Track offset of the first index point in samples, relative to the beginning of the FLAC audio stream.
    pub track_offset: u64,
    // u(8) Track number.
    pub track_number: u8,
    // u(12*8) Track ISRC.
    pub track_isrc: u8,
    // u(1) The track type: 0 for audio, 1 for non-audio.
    // This corresponds to the CD-DA Q-channel control bit 3.
    pub track_type: bool,
    // u(1) The pre-emphasis flag: 0 for no pre-emphasis, 1 for pre-emphasis.
    // This corresponds to the CD-DA Q-channel control bit 5.
    pub pre_emphasis_flag: bool,
    // u(6+13*8) Reserved. All bits MUST be set to zero.
    pub reserved: (),
    // u(8) The number of track index points.
    pub number_of_track_index_points: u8,
}
// https://datatracker.ietf.org/doc/html/rfc9639/#section-8.7.1.1
// name-cuesheet-track-index-point
pub struct CuesheetTrackIndexPoint {
    // u(64) Offset in samples, relative to the track offset, of the index point.
    pub offset: u64,
    // u(8) The track index point number.
    pub index: u8,
    // u(3*8) Reserved. All bits MUST be set to zero.
    pub reserved: (), //u24
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-8.8
// name-picture
#[derive(Debug)]
pub struct Picture<'a> {
    // u(32) Picture type according to Table 13.
    pub picture_type: PictureType,
    // u(32) The length of the media type string in bytes.
    pub media_length: u32,
    // u(32) MIME type, e.g. "image/jpeg", "image/png", or "-->" for URI.
    pub media_type: &'a str,
    // u(32) The length of the description string in bytes.
    pub description_length: u32,
    // u(n*8) Picture description in UTF-8.
    pub description: &'a str,
    // u(32) Width in pixels.
    pub width: u32,
    // u(n*8) Height in pixels.
    pub height: u32,
    // u(32) Color depth in bits per pixel.
    pub color_depth: u32,
    // u(32) Number of colors used for indexed-color pictures.
    // 0 for non-indexed pictures.
    pub colors: u32,
    // u(32) The length of the picture data in bytes.
    pub data_length: u32,
    // u(n*8)	The binary picture data.
    pub data: &'a [u8],
}
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureType {
    Other = 0,
    /// PNG file icon of 32x32 pixels.
    PngFileIcon32x32 = 1,
    /// General file icon.
    GeneralFileIcon = 2,
    /// Front cover.
    FrontCover = 3,
    /// Back cover.
    BackCover = 4,
    /// Liner notes page.
    LinerNotesPage = 5,
    /// Media label (e.g., CD, Vinyl or Cassette label).
    MediaLabel = 6,
    /// Lead artist, lead performer, or soloist.
    LeadArtist = 7,
    /// Artist or performer.
    ArtistOrPerformer = 8,
    /// Conductor.
    Conductor = 9,
    /// Band or orchestra.
    BandOrOrchestra = 10,
    /// Composer.
    Composer = 11,
    /// Lyricist or text writer.
    LyricistOrTextWriter = 12,
    /// Recording location.
    RecordingLocation = 13,
    /// During recording.
    DuringRecording = 14,
    /// During performance.
    DuringPerformance = 15,
    /// Movie or video screen capture.
    MovieOrVideoScreenCapture = 16,
    /// A bright colored fish.
    ABrightColoredFish = 17,
    /// Illustration.
    Illustration = 18,
    /// Band or artist logotype.
    BandOrArtistLogotype = 19,
    /// Publisher or studio logotype.
    PublisherOrStudioLogotype = 20,
}
impl From<u32> for PictureType {
    fn from(value: u32) -> Self {
        match value {
            0 => PictureType::Other,
            1 => PictureType::PngFileIcon32x32,
            2 => PictureType::GeneralFileIcon,
            3 => PictureType::FrontCover,
            4 => PictureType::BackCover,
            5 => PictureType::LinerNotesPage,
            6 => PictureType::MediaLabel,
            7 => PictureType::LeadArtist,
            8 => PictureType::ArtistOrPerformer,
            9 => PictureType::Conductor,
            10 => PictureType::BandOrOrchestra,
            11 => PictureType::Composer,
            12 => PictureType::LyricistOrTextWriter,
            13 => PictureType::RecordingLocation,
            14 => PictureType::DuringRecording,
            15 => PictureType::DuringPerformance,
            16 => PictureType::MovieOrVideoScreenCapture,
            17 => PictureType::ABrightColoredFish,
            18 => PictureType::Illustration,
            19 => PictureType::BandOrArtistLogotype,
            20 => PictureType::PublisherOrStudioLogotype,
            _ => PictureType::Other,
        }
    }
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-9
// name-frame-structure
// One or more frames follow directly after the last metadata block.
// Each frame consists of a frame header, one or more subframes, padding zero bits to achieve byte alignment, and a frame footer.
// The number of subframes in each frame is equal to the number of audio channels.

// https://datatracker.ietf.org/doc/html/rfc9639/#section-9.1
// name-frame-header
pub struct FrameHeader {
    // u(15) Each frame MUST start on a byte boundary and start with the 15-bit frame sync code 0b111111111111100.
    pub frame_sync_code: u16,
    // u(1) The blocking strategy bit is 0 for a fixed block size stream or 1 for a variable block size stream.
    pub blocking_strategy_bit: bool,
    // u(4) the first 4 bits of the third byte of each frame referred to as the block size bits.
    pub block_size_bits: u8,
    // u(4) the last 4 bits of the third byte of each frame referred to as the sample rate bits.
    pub sample_rate_bits: u8,
    // u(4) the first 4 bits of the fourth byte of each frame referred to as the channels bits.
    pub channals_bits: u8,
    // u(3) bits 5, 6, and 7 of each fourth byte of each frame contain the bit depth The next bit is reserved and MUST be zero.
    pub bit_depth_bits: u8,
    // starting at the fifth byte of the frame
    pub code_number: u64,
    // https://datatracker.ietf.org/doc/html/rfc9639/#section-9.1.6
    // name-uncommon-block-size
    pub uncommon_block_size: u16,
    // https://datatracker.ietf.org/doc/html/rfc9639/#section-9.1.7
    // name-uncommon-sample-rate
    pub uncommon_sample_rate: u32,
    // (u8) This CRC covers the whole frame header before the CRC, including the sync code.
    pub crc: u8,
    // Auxiliary data
    pub pos: usize,
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-4.2
// name-interchannel-decorrelation
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Independent,
    Leftside,
    Midside,
    Sideright,
}

impl FrameHeader {
    pub fn new(buf: &[u8]) -> FrameHeader {
        let temp = u16::from_be_bytes([buf[0], buf[1]]);
        let frame_sync_code = temp & (!1);
        assert_eq!(frame_sync_code, 0xFFF8);
        let blocking_strategy_bit = temp & 1 == 1;
        let block_size_bits = buf[2] >> 4;
        let sample_rate_bits = buf[2] & 0x0F;
        let channals_bits = buf[3] >> 4;
        let bit_depth_bits = (buf[3] >> 1) & 0x07;
        let (code_number, len) = FrameHeader::decode_number(&buf[4..]);
        let mut pos = 4 + len;
        let uncommon_block_size = if block_size_bits == 6 {
            pos += 1;
            buf[pos - 1] as u16 + 1
        } else if block_size_bits == 7 {
            pos += 2;
            u16::from_be_bytes([buf[pos - 2], buf[pos - 1]]) + 1
        } else {
            0
        };
        let uncommon_sample_rate = if sample_rate_bits == 6 {
            pos += 1;
            buf[pos - 1] as u32
        } else if sample_rate_bits == 7 {
            pos += 2;
            u16::from_be_bytes([buf[pos - 2], buf[pos - 1]]) as u32
        } else if sample_rate_bits == 8 {
            pos += 2;
            10 * u16::from_be_bytes([buf[pos - 2], buf[pos - 1]]) as u32
        } else {
            0
        };
        let crc = buf[pos];
        pos += 1;
        FrameHeader {
            frame_sync_code,
            blocking_strategy_bit,
            block_size_bits,
            sample_rate_bits,
            channals_bits,
            bit_depth_bits,
            code_number,
            uncommon_block_size,
            uncommon_sample_rate,
            crc,
            pos,
        }
    }
    // https://datatracker.ietf.org/doc/html/rfc9639/#section-9.1.1
    // name-block-size-bits
    pub fn block_size(&self) -> u16 {
        let block_size: u16 = match self.block_size_bits {
            // Reserved
            0b0000 => unreachable!(),
            // 192
            0b0001 => 192,
            // 144 * (2v), i.e., 576, 1152, 2304, or 4608
            0b0010..=0b0101 => 144 * (1 << (self.block_size_bits - 2)),
            // Uncommon block size minus 1, stored as an 8-bit number
            0b0110 => self.uncommon_block_size,
            // Uncommon block size minus 1, stored as a 16-bit number
            0b0111 => self.uncommon_block_size,
            // 2v, i.e., 256, 512, 1024, 2048, 4096, 8192, 16384, or 32768
            0b1000..=0b1111 => 1 << self.block_size_bits,
            _ => unreachable!("overflow"),
        };
        block_size
    }

    // https://datatracker.ietf.org/doc/html/rfc9639/#section-9.1.2
    // name-sample-rate-bits
    pub fn sample_rate(&self) -> u32 {
        match self.sample_rate_bits {
            0b0000 => *SAMPLE_RATE.get().unwrap(),
            0b0001 => 88_200,
            0b0010 => 176_400,
            0b0011 => 192_000,
            0b0100 => 8_000,
            0b0101 => 16_000,
            0b0110 => 22_050,
            0b0111 => 24_000,
            0b1000 => 32_000,
            0b1001 => 44_100,
            0b1010 => 48_000,
            0b1011 => 96_000,
            // Uncommon sample rate in kHz, stored as an 8-bit number
            0b1100 => self.uncommon_sample_rate,
            // Uncommon sample rate in Hz, stored as a 16-bit number
            0b1101 => self.uncommon_sample_rate,
            // Uncommon sample rate in Hz divided by 10, stored as a 16-bit number
            0b1110 => 10 * self.uncommon_sample_rate,
            _ => unreachable!("Forbidden"),
        }
    }

    // https://datatracker.ietf.org/doc/html/rfc9639/#section-9.1.3
    // name-channels-bits
    pub fn channel(&self) -> u8 {
        match self.channals_bits {
            c @ 0..8 => c + 1,
            8..=10 => 2,
            _ => unreachable!("Reserved"),
        }
    }

    pub fn channel_assignment(&self) -> Channel {
        match self.channals_bits {
            0..8 => Channel::Independent,
            8 => Channel::Leftside,
            9 => Channel::Sideright,
            10 => Channel::Midside,
            _ => unreachable!("Reserved"),
        }
    }

    // https://datatracker.ietf.org/doc/html/rfc9639/#section-9.1.4
    // name-bit-depth-bits
    pub fn bit_depth(&self) -> u8 {
        match self.bit_depth_bits {
            0 => *BIT_DEPTH.get().unwrap(),
            1 => 8,
            2 => 12,
            3 => unreachable!("Reserved"),
            b @ 4..7 => 4 * b,
            7 => 32,
            _ => unreachable!("overflow"),
        }
    }
    // https://datatracker.ietf.org/doc/html/rfc9639/#section-9.1.5
    // name-coded-number
    pub fn decode_number(bytes: &[u8]) -> (u64, usize) {
        let first = bytes[0];
        let (len, prefix_bits) = match first {
            b if b & 0b1000_0000 == 0 => (1, 7),
            b if b & 0b1110_0000 == 0b1100_0000 => (2, 5),
            b if b & 0b1111_0000 == 0b1110_0000 => (3, 4),
            b if b & 0b1111_1000 == 0b1111_0000 => (4, 3),
            b if b & 0b1111_1100 == 0b1111_1000 => (5, 2),
            b if b & 0b1111_1110 == 0b1111_1100 => (6, 1),
            0b1111_1110 => (7, 0),
            _ => unreachable!(),
        };
        let mut value = (first & ((1 << prefix_bits) - 1)) as u64;
        for &b in &bytes[1..len] {
            debug_assert_eq!(b & 0xc0, 0x80);
            value = (value << 6) | (b & 0x3f) as u64;
        }
        (value, len)
    }

    // https://datatracker.ietf.org/doc/html/rfc9639/#section-9.1.8
    // name-frame-header-crc
    pub fn crc8(data: &[u8]) -> u8 {
        const POLY: u8 = 0x07; // CRC-8  (x^8 + x^2 + x + 1)
        let mut crc: u8 = 0;
        for &byte in data {
            crc ^= byte;
            for _ in 0..8 {
                if (crc & 0x80) != 0 {
                    crc = (crc << 1) ^ POLY;
                } else {
                    crc <<= 1;
                }
            }
        }
        crc
    }
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-9.2
// name-subframes
// Following the frame header are a number of subframes equal to the number of audio channels.

// https://datatracker.ietf.org/doc/html/rfc9639/#section-9.2.1
// name-subframe-header
// The first bit of the header MUST be 0
pub struct Subframe {
    pub subframe_type: SubframeType,
    pub order: Option<u8>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubframeType {
    // Constant subframe.
    Constant,
    // Verbatim subframe.
    Verbatim,
    // Reserved
    Reserved,
    // Subframe with a fixed predictor of order v-8; i.e., 0, 1, 2, 3 or 4
    FixedPredictor,
    // Subframe with a linear predictor of order v-31; i.e., 1 through 32 (inclusive)
    LinearPredictor,
}

// https://datatracker.ietf.org/doc/html/rfc9639/#name-frame-footer
pub fn crc16(data: &[u8]) -> u16 {
    let poly: u16 = 0x8005;
    let mut crc: u16 = 0x0000;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ poly;
            } else {
                crc = crc << 1;
            }
        }
    }
    crc
}

pub struct Block<'a> {
    pub block_type: u8,
    pub block_data: &'a [u8],
}
pub fn read_block(buf: &[u8]) -> (Vec<Block<'_>>, usize) {
    let mut is_last;
    let mut pos = 0;
    let mut block_type;
    let mut block_size;
    let mut blocks = Vec::new();
    loop {
        is_last = buf[pos] >> 7 == 1;
        block_type = buf[pos] & 0x7F;
        block_size = u32::from_be_bytes([0, buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;
        pos += 4;
        let block_data = &buf[pos..pos + block_size];
        pos += block_size;
        let block = Block {
            block_type,
            block_data,
        };
        blocks.push(block);
        if is_last {
            break;
        }
    }
    (blocks, pos)
}
pub fn parse_block(blocks: Vec<Block<'_>>) {
    for block in blocks {
        let block_type = MetadataBlockType::from_u8(block.block_type);
        let data = block.block_data;
        let length = data.len();
        println!("block({block_type:?}) length:{length:}");
        match block_type {
            MetadataBlockType::Streaminfo => {
                assert_eq!(length, 34);
                let min_block_size = u16::from_be_bytes([data[0], data[1]]);
                let max_block_size = u16::from_be_bytes([data[2], data[3]]);
                let min_frame_size = u32::from_be_bytes([0, data[4], data[5], data[6]]);
                let max_frame_size = u32::from_be_bytes([0, data[7], data[8], data[9]]);
                let value = u32::from_be_bytes([0, data[10], data[11], data[12]]);
                let sample_rate = value >> 4;
                SAMPLE_RATE.set(sample_rate).unwrap();
                let channels = ((value >> 1) & 0x7) + 1;
                let temp = data[13];
                let bits_per_sample = (((value & 1) << 4) as u8 | (temp >> 4)) + 1;
                BIT_DEPTH.set(bits_per_sample).unwrap();
                let total_samples = u64::from_be_bytes([
                    0,
                    0,
                    0,
                    data[13] & 0x0F,
                    data[14],
                    data[15],
                    data[16],
                    data[17],
                ]);
                let md5 = &data[18..34];
                println!("  {:<15}: {}", "Min block size", min_block_size);
                println!("  {:<15}: {}", "Max block size", max_block_size);
                println!("  {:<15}: {}", "Min frame size", min_frame_size);
                println!("  {:<15}: {}", "Max frame size", max_frame_size);
                println!("  {:<15}: {} Hz", "Sample rate", sample_rate);
                println!("  {:<15}: {}", "Channels", channels);
                println!("  {:<15}: {}", "Bits per sample", bits_per_sample);
                println!("  {:<15}: {}", "Total samples", total_samples);
                print!("MD5: ");
                for byte in md5 {
                    print!("{:02x}", byte);
                }
                println!();
            }
            MetadataBlockType::Padding => {}
            MetadataBlockType::Application => {
                // https://datatracker.ietf.org/doc/html/rfc9639/#application-id-registry
                let id = str::from_utf8(&data[..4]).unwrap();
                println!("id:{id}");
            }
            MetadataBlockType::Seektable => {}
            MetadataBlockType::Vorbiscomment => {
                let length = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let mut pos = 4 + length as usize;
                let vendor_string = str::from_utf8(&data[4..pos]).unwrap();
                let comment_len =
                    u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
                println!("  {vendor_string}");
                pos += 4;
                for _ in 0..comment_len {
                    let len = u32::from_le_bytes([
                        data[pos],
                        data[pos + 1],
                        data[pos + 2],
                        data[pos + 3],
                    ]) as usize;
                    pos += 4;
                    let str = str::from_utf8(&data[pos..pos + len]).unwrap();
                    pos += len;
                    println!("  {str}");
                }
            }
            MetadataBlockType::Cuesheet => {}
            MetadataBlockType::Picture => {
                let mut pos = 0;
                let picture_type =
                    u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
                pos += 4;
                let media_type_length =
                    u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let media_type = str::from_utf8(&data[pos..pos + media_type_length]).unwrap();
                pos += media_type_length;
                let description_length =
                    u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let description = str::from_utf8(&data[pos..pos + description_length]).unwrap();
                pos += description_length;
                let width =
                    u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
                pos += 4;
                let height =
                    u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
                pos += 4;
                let color_depth =
                    u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
                pos += 4;
                let colors =
                    u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
                let str = if colors == 0 {
                    std::format!("non-indexed")
                } else {
                    std::format!("indexed-color({})", colors)
                };
                pos += 4;
                let data_length =
                    u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                pos += 4;
                let _picture_data = &data[pos..pos + data_length];
                let picture_type2: PictureType = picture_type.into();
                println!("  picture type: {picture_type}({picture_type2:?})");
                println!("  MIME type: {media_type}");
                println!("  description: {description}");
                println!("  width: {width}");
                println!("  height: {height}");
                println!("  color depth: {color_depth}");
                println!("  colors: {str}");
                println!("  picture data length: {data_length}");
            }
            _ => {}
        }
    }
}
pub fn read_frame(buf: &[u8]) {
    let mut pos = 0;
    while pos < buf.len() {
        let start = pos;
        let frame_header = FrameHeader::new(&buf[pos..]);
        let is_variable = frame_header.blocking_strategy_bit;
        let block_size = frame_header.block_size();
        let sample_rate = frame_header.sample_rate();
        let channel = frame_header.channel();
        let bit_depth = frame_header.bit_depth();
        let number = frame_header.code_number;
        pos += frame_header.pos;
        assert_eq!(frame_header.crc, FrameHeader::crc8(&buf[start..pos - 1]));
        let assignment = frame_header.channel_assignment();
        println!(
            "frame{number} variable({is_variable}) blocksize={block_size} sample_rate={sample_rate} channels={channel}({assignment:?}) bit_depth={bit_depth}"
        );
        let mut stream = BitReader::new(&buf[pos..]);
        for i in 0..channel {
            let mut precision = bit_depth as usize;
            // https://datatracker.ietf.org/doc/html/rfc9639/#section-4.2
            // name-interchannel-decorrelation
            match assignment {
                Channel::Independent => {}
                Channel::Leftside => {
                    if i == 1 {
                        precision += 1;
                    }
                }
                Channel::Sideright => {
                    if i == 0 {
                        precision += 1;
                    }
                }
                Channel::Midside => {
                    if i == 1 {
                        precision += 1;
                    }
                }
            }
            read_subframe(&mut stream, precision, block_size as u32);
        }
        // https://datatracker.ietf.org/doc/html/rfc9639/#section-9.3
        // name-frame-footer
        let temp = if stream.bit_pos % 8 == 0 { 0 } else { 1 };
        pos += stream.bit_pos / 8 + temp + 2;
        // This CRC covers the whole frame, excluding the 16-bit CRC but including the sync code.
        let _crc_16 = u16::from_le_bytes([buf[pos - 2], buf[pos - 1]]);
    }
}
// https://datatracker.ietf.org/doc/html/rfc9639/#section-9.2
// name-subframes
pub fn read_subframe(stream: &mut BitReader, mut precision: usize, block_size: u32) {
    assert_eq!(stream.read_bits(1), 0);
    let subframe_type = stream.read_bits(6) as u8;
    if stream.read_bits(1) != 0 {
        while stream.read_bits(1) == 1 {}
        let w = stream.position() + 1;
        precision -= w;
    }
    let subframe;
    let _samples = match subframe_type {
        0 => {
            subframe = Subframe {
                subframe_type: SubframeType::Constant,
                order: None,
            };
            read_subframe_constant(stream, block_size, precision as u32)
        }
        1 => {
            subframe = Subframe {
                subframe_type: SubframeType::Verbatim,
                order: None,
            };
            read_subframe_verbatim(stream, block_size, precision as u32)
        }
        2..8 => unreachable!("Reserved"),
        8..=12 => {
            let order = subframe_type & 7;
            subframe = Subframe {
                subframe_type: SubframeType::FixedPredictor,
                order: Some(order),
            };
            read_subframe_fixed(stream, block_size, precision, order as usize)
        }
        13..=31 => unreachable!("Reserved"),
        32..64 => {
            let order = (subframe_type & 0b11111) + 1;
            subframe = Subframe {
                subframe_type: SubframeType::LinearPredictor,
                order: Some(order),
            };
            read_subframe_lpc(stream, block_size, precision, order as usize)
        }
        _ => unreachable!("overflow"),
    };
    let str = if let Some(order) = subframe.order {
        std::format!("order={order}")
    } else {
        std::format!("")
    };
    println!("  subframe type={:?} {str}", subframe.subframe_type);
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-9.2.3
// name-constant-subframe
pub fn read_subframe_constant(stream: &mut BitReader, block_size: u32, bps: u32) -> Vec<i32> {
    let sample = stream.read_signed(bps as usize).unwrap() as i32;
    vec![sample; block_size as usize]
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-9.2.4
// Verbatim Subframe
pub fn read_subframe_verbatim(stream: &mut BitReader, block_size: u32, bps: u32) -> Vec<i32> {
    let mut samples = Vec::with_capacity(block_size as usize);
    for _ in 0..block_size {
        samples.push(stream.read_signed(bps as usize).unwrap() as i32);
    }
    samples
}
// https://datatracker.ietf.org/doc/html/rfc9639/#section-9.2.5
// name-fixed-predictor-subframe
pub fn read_subframe_fixed(
    stream: &mut BitReader,
    block_size: u32,
    bps: usize,
    order: usize,
) -> Vec<i32> {
    let mut samples = Vec::with_capacity(block_size as usize);
    for _ in 0..order {
        samples.push(stream.read_signed(bps).unwrap() as i32);
    }
    read_residual(stream, &mut samples, block_size, order);
    fixed_restore(&mut samples, order);
    samples
}

// https://datatracker.ietf.org/doc/html/rfc9639/#section-9.2.6
// name-linear-predictor-subframe
pub fn read_subframe_lpc(
    stream: &mut BitReader,
    block_size: u32,
    bps: usize,
    order: usize,
) -> Vec<i32> {
    let mut samples = Vec::with_capacity(block_size as usize);
    for _ in 0..order {
        let warm_up = stream.read_signed(bps).unwrap();
        samples.push(warm_up as i32);
    }
    let predictor_coefficient_precision = stream.read_bits(4) as usize + 1;
    let prediction_right_shift_bits = stream.read_signed(5).unwrap();

    let mut qlp_coefficients = Vec::with_capacity(order as usize);
    for _ in 0..order {
        let predictor_coefficient = stream.read_signed(predictor_coefficient_precision).unwrap();
        qlp_coefficients.push(predictor_coefficient as i32);
    }
    read_residual(stream, &mut samples, block_size, order);
    lpc_restore(
        &mut samples,
        &qlp_coefficients,
        prediction_right_shift_bits as u32,
    );
    samples
}
// https://datatracker.ietf.org/doc/html/rfc9639/#section-9.2.7
// name-coded-residual
pub fn read_residual(
    stream: &mut BitReader,
    samples: &mut Vec<i32>,
    block_size: u32,
    order: usize,
) {
    let residual_indicate = stream.read_bits(2);
    let (parameter_bit, val) = if residual_indicate == 0 {
        (4, 0b1111)
    } else if residual_indicate == 1 {
        (5, 0b11111)
    } else {
        unreachable!()
    };
    let partition_order = stream.read_bits(4);
    let partitions = 1u32 << partition_order;
    let partition_size = (block_size / partitions) as usize;
    for i in 0..partitions {
        let number = if i == 0 {
            partition_size - order
        } else {
            partition_size
        };
        // https://datatracker.ietf.org/doc/html/rfc9639/#section-9.2.7.2
        // name-rice-code
        let parameter = stream.read_bits(parameter_bit);
        // https://datatracker.ietf.org/doc/html/rfc9639/#section-9.2.7.1
        // name-escaped-partition
        if parameter == val {
            let bits = stream.read_bits(5);
            for _ in 0..number {
                let sample = stream.read_signed(bits as usize).unwrap();
                samples.push(sample as i32);
            }
        } else {
            for _ in 0..number {
                let q = stream.unary();
                let r = stream.read_bits(parameter as usize);
                let u = ((q as u64) << parameter) | r;
                let signed: i64 = if u & 1 == 0 {
                    (u >> 1) as i64
                } else {
                    -((u >> 1) as i64) - 1
                };
                samples.push(signed as i32);
            }
        }
    }
}

//
pub fn fixed_restore(samples: &mut [i32], order: usize) {
    let len: usize = samples.len();
    match order {
        0 => {}
        1 => {
            for t in order..len {
                let p = samples[t - 1] as i64;
                samples[t] = (p + samples[t] as i64) as i32;
            }
        }
        2 => {
            for t in order..len {
                let p = 2 * samples[t - 1] as i64 - samples[t - 2] as i64;
                samples[t] = (p + samples[t] as i64) as i32;
            }
        }
        3 => {
            for t in order..len {
                let p =
                    3 * samples[t - 1] as i64 - 3 * samples[t - 2] as i64 + samples[t - 3] as i64;
                samples[t] = (p + samples[t] as i64) as i32;
            }
        }
        4 => {
            for t in order..len {
                let p = 4 * samples[t - 1] as i64 - 6 * samples[t - 2] as i64
                    + 4 * samples[t - 3] as i64
                    - samples[t - 4] as i64;
                samples[t] = (p + samples[t] as i64) as i32;
            }
        }
        _ => {
            unreachable!();
        }
    }
}

pub fn lpc_restore(
    samples: &mut [i32],
    qlp_coefficients: &[i32],
    prediction_right_shift_bits: u32,
) {
    let order = qlp_coefficients.len();
    let len = samples.len();
    let mut rc_stack = [0i32; 32];
    let n = order.min(32);
    for (j, &c) in qlp_coefficients.iter().rev().take(n).enumerate() {
        rc_stack[j] = c;
    }
    let rc = &rc_stack[..n];
    for t in order..len {
        let window = &samples[t - order..t];
        let mut pred: i64 = 0;
        for (&s, &c) in window.iter().zip(rc) {
            pred += (c as i64) * (s as i64);
        }
        let predicted = (pred >> prediction_right_shift_bits) as i32;
        samples[t] = predicted.wrapping_add(samples[t]);
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut file = File::open(&args[1])?;
    let file_size = file.metadata()?.len() as usize;
    let mut buf = Vec::with_capacity(file_size);
    file.read_to_end(&mut buf)?;
    assert_eq!(SIGNATURE, b"fLaC");
    let (blocks, pos) = read_block(&buf[4..]);
    parse_block(blocks);
    let frames_data = &buf[pos + 4..];

    read_frame(frames_data);
    Ok(())
}

pub struct BitReader<'a> {
    pub data: &'a [u8],
    pub bit_pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, bit_pos: 0 }
    }
    pub fn read_bits(&mut self, n: usize) -> u64 {
        let mut result = 0u64;
        for _ in 0..n {
            let byte_pos = self.bit_pos / 8;
            let bit_offset = self.bit_pos % 8;
            let bit = (self.data[byte_pos] >> (7 - bit_offset)) & 1;
            result = (result << 1) | bit as u64;
            self.bit_pos += 1;
        }
        result
    }
    pub fn read_signed(&mut self, n: usize) -> Option<i64> {
        if n == 0 {
            return Some(0);
        }
        let value = self.read_bits(n);
        if value & (1u64 << (n - 1)) != 0 {
            let value = value | (!0u64 << n);
            Some(value as i64)
        } else {
            Some(value as i64)
        }
    }
    pub fn position(&self) -> usize {
        self.bit_pos
    }
    pub fn remaining(&self) -> usize {
        self.data.len() * 8 - self.bit_pos
    }
    pub fn skip(&mut self, n: usize) {
        self.bit_pos += n;
    }
    pub fn unary(&mut self) -> u32 {
        let mut quotient = 0;
        while self.read_bits(1) == 0 {
            quotient += 1;
        }
        quotient
    }
}
