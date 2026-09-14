use std::env;
use std::fs::File;
use std::io::{self, Read};

// https://www.w3.org/TR/png-3/#3PNGsignature
pub const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

// https://www.w3.org/TR/png-3/#5Chunk-layout
// struct Chuck {
//     length: u32, // The length counts only the data field, not itself, the chunk type, or the CRC.
//     chunk_type: u32,
//     data: [u8; length],
//     crc: u32,  // including the chunk type field and chunk data fields, but not including the length field
// }
pub struct Chunk<'a> {
    pub length: u32,
    pub chunk_type: [u8; 4],
    pub data: &'a [u8],
    pub crc: u32,
}
impl Chunk<'_> {
    pub fn verify_crc(&self) -> bool {
        let mut crc = 0xffffffff;
        for &b in &self.chunk_type {
            let idx = ((crc ^ b as u32) & 0xff) as usize;
            crc = TABLE[idx] ^ (crc >> 8);
        }
        for &b in self.data {
            let idx = ((crc ^ b as u32) & 0xff) as usize;
            crc = TABLE[idx] ^ (crc >> 8);
        }
        crc ^= 0xffffffff;
        crc == self.crc
    }
    pub fn chunk_type(&self) -> &str {
        str::from_utf8(&self.chunk_type).unwrap_or("<invalid>")
    }
    // https://www.w3.org/TR/png-3/#5Chunk-naming-conventions
    pub fn is_critical(&self) -> bool {
        self.chunk_type[0] & 0b00100000 == 0
    }
    pub fn is_ancillary(&self) -> bool {
        self.chunk_type[0] & 0b00100000 == 1
    }
    pub fn is_public(&self) -> bool {
        self.chunk_type[1] & 0b00100000 == 0
    }
    pub fn is_private(&self) -> bool {
        self.chunk_type[1] & 0b00100000 == 1
    }
    pub fn is_conform(&self) -> bool {
        self.chunk_type[2] & 0b00100000 == 0
    }
    pub fn not_conform(&self) -> bool {
        self.chunk_type[2] & 0b00100000 == 1
    }
    pub fn unsafe_to_copy(&self) -> bool {
        self.chunk_type[3] & 0b00100000 == 0
    }
    pub fn safe_to_copy(&self) -> bool {
        self.chunk_type[3] & 0b00100000 == 1
    }
    pub fn property(&self) -> String {
        std::format!(
            "property: Ancillary({}) Private({}) Reserved({}) Safe-to-copy({})",
            self.is_critical(),
            self.is_private(),
            self.is_conform(),
            self.safe_to_copy()
        )
    }
}

// https://www.w3.org/TR/png-3/#5CRC-algorithm
const TABLE: [u32; 256] = make_crc_table();
pub fn crc32(buf: &[u8]) -> u32 {
    let mut crc = 0xffffffff;
    for &b in buf {
        let idx = ((crc ^ b as u32) & 0xff) as usize;
        crc = TABLE[idx] ^ (crc >> 8);
    }
    crc ^ 0xffffffff
}
const fn make_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];

    let mut n = 0;

    while n < 256 {
        let mut c = n as u32;

        let mut i = 0;
        while i < 8 {
            if c & 1 != 0 {
                c = 0xEDB88320 ^ (c >> 1);
            } else {
                c >>= 1;
            }

            i += 1;
        }

        table[n] = c;
        n += 1;
    }
    table
}

// https://www.w3.org/TR/png-3/#5ChunkOrdering
pub fn check_chunk_ordering(chunks: &Vec<Chunk<'_>>) {
    let ihdr_pos = chunks.iter().position(|c| c.chunk_type == *b"IHDR");
    let plte_pos = chunks.iter().position(|c| c.chunk_type == *b"PlTE");
    let idat_pos = chunks.iter().position(|c| c.chunk_type == *b"IDAT");
    let iend_pos = chunks.iter().position(|c| c.chunk_type == *b"IEND");
    let mut have_idat = false;
    let mut last_idat = false;
    for chunk in chunks {
        if chunk.chunk_type == *b"IDAT" {
            assert_eq!(last_idat, false, "IDAT chunks must be consecutive");
            have_idat = true;
        } else if have_idat {
            last_idat = true;
        }
    }
    if let Some(p) = plte_pos {
        assert!(p < idat_pos.unwrap(), "PLTE Chunk must Before first IDAT");
    }
    assert_eq!(ihdr_pos, Some(0), "IHDR Chunk Shall be first");
    let pos = chunks.len() - 1;
    assert_eq!(iend_pos, Some(pos), "IEND Chunk Shall be last");
}
pub fn check_chunk_count(chunks: &Vec<Chunk<'_>>) {
    let get_count = |chunk_type: [u8; 4]| -> usize {
        chunks
            .iter()
            .filter(|chunk| chunk.chunk_type == chunk_type)
            .count()
    };
    let ihdr_count = get_count(*b"IHDR");
    let plte_count = get_count(*b"PLTE");
    let idat_count = get_count(*b"IDAT");
    let iend_count = get_count(*b"IEND");
    assert_eq!(ihdr_count, 1, "IHDR Chunk: Multiple allowed = No");
    assert!(plte_count <= 1, "PLTE Chunk: Multiple allowed = No");
    assert!(idat_count >= 1, "PLTE Chunk: Multiple allowed = Yes");
    assert_eq!(iend_count, 1, "IEND Chunk: Multiple allowed = No");
}

// https://www.w3.org/TR/png-3/#sec-field-value-extensibility
// Values greater than or equal to 128 in the following fields are
// private field values:
// bit depth
// color type
// compression method
// interlace method
// filter method

// https://www.w3.org/TR/png-3/#6Colour-values
const GREY: u8 = 0;
const PALETTE_USED: u8 = 1;
const TRUECOLOR_USED: u8 = 2;
const ALPHA_USED: u8 = 4;
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageType {
    Greyscale = GREY,
    Truecolor = TRUECOLOR_USED,
    IndexedColor = TRUECOLOR_USED | PALETTE_USED,
    GreyscaleWithAlpha = ALPHA_USED,
    TruecolorWithAlpha = TRUECOLOR_USED | ALPHA_USED,
}
impl ImageType {
    // https://www.w3.org/TR/png-3/#4Concepts.PNGImage
    pub fn image_type(color_type: u8) -> ImageType {
        match color_type {
            0 => ImageType::Greyscale,
            2 => ImageType::Truecolor,
            3 => ImageType::IndexedColor,
            4 => ImageType::GreyscaleWithAlpha,
            6 => ImageType::TruecolorWithAlpha,
            _ => unreachable!("There are only five types of PNG images."),
        }
    }
    pub fn channal(self) -> usize {
        match self {
            ImageType::Greyscale => 1,
            ImageType::Truecolor => 3,
            ImageType::IndexedColor => 1,
            ImageType::GreyscaleWithAlpha => 2,
            ImageType::TruecolorWithAlpha => 4,
        }
    }
}

// https://www.w3.org/TR/png-3/#7Integers-and-byte-order
// the most significant byte comes first,
// then the less significant bytes in descending order of significance

// https://www.w3.org/TR/png-3/#8Interlace
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterlaceMethod {
    None = 0,
    Adam7 = 1,
}
impl InterlaceMethod {
    pub fn method(val: u8) -> InterlaceMethod {
        match val {
            0 => InterlaceMethod::None,
            1 => InterlaceMethod::Adam7,
            _ => unreachable!(
                "Only interlace  methods 0(None) and 1(Adam7) are defined. Other filter methods are reserved"
            ),
        }
    }
}

// https://www.w3.org/TR/png-3/#9Filter-types
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterMethod {
    Zero = 0,
}
impl FilterMethod {
    pub fn method(val: u8) -> FilterMethod {
        match val {
            0 => FilterMethod::Zero,
            _ => unreachable!("Only filter method 0 is defined. Other filter methods are reserved"),
        }
    }
}
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterType {
    None = 0,
    Sub = 1,
    Up = 2,
    Average = 3,
    Paeth = 4,
}
impl FilterType {
    pub fn filter_type(val: u8) -> FilterType {
        match val {
            0 => FilterType::None,
            1 => FilterType::Sub,
            2 => FilterType::Up,
            3 => FilterType::Average,
            4 => FilterType::Paeth,
            _ => unreachable!("Only provides a set of five filter types"),
        }
    }
}
type Filter = FilterType;
impl Filter {
    // https://www.w3.org/TR/png-3/#9Filter-type-4-Paeth
    fn paeth(a: u8, b: u8, c: u8) -> u8 {
        let p = a as i32 + b as i32 - c as i32;

        let pa = (p - a as i32).abs();
        let pb = (p - b as i32).abs();
        let pc = (p - c as i32).abs();

        if pa <= pb && pa <= pc {
            a
        } else if pb <= pc {
            b
        } else {
            c
        }
    }

    pub fn apply(&self, scanline: &mut [u8], prev_scanline: &[u8], bpp: usize) {
        match self {
            FilterType::None => {}

            FilterType::Sub => {
                for i in bpp..scanline.len() {
                    scanline[i] = scanline[i].wrapping_add(scanline[i - bpp]);
                }
            }

            FilterType::Up => {
                for i in 0..scanline.len() {
                    scanline[i] = scanline[i].wrapping_add(prev_scanline[i]);
                }
            }

            FilterType::Average => {
                for i in 0..scanline.len() {
                    let a = if i >= bpp { scanline[i - bpp] } else { 0 };

                    let b = prev_scanline[i];

                    let average = ((a as u16 + b as u16) / 2) as u8;

                    scanline[i] = scanline[i].wrapping_add(average);
                }
            }

            FilterType::Paeth => {
                for i in 0..scanline.len() {
                    let a = if i >= bpp { scanline[i - bpp] } else { 0 };

                    let b = prev_scanline[i];

                    let c = if i >= bpp { prev_scanline[i - bpp] } else { 0 };

                    scanline[i] = scanline[i].wrapping_add(Self::paeth(a, b, c));
                }
            }
        }
    }
}

// https://www.w3.org/TR/png-3/#10CompressionCM0
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionMethod {
    Deflate = 0,
}
impl CompressionMethod {
    pub fn method(val: u8) -> CompressionMethod {
        match val {
            0 => CompressionMethod::Deflate,
            _ => unreachable!(
                "Only PNG compression method 0 is defined. Other values of compression method are reserved"
            ),
        }
    }
}

// https://datatracker.ietf.org/doc/html/rfc1950 (zlib)
pub fn zlib(data: &[u8], out: &mut Vec<u8>) {
    let cmf = data[0];
    let compression_method = cmf & 0x0f;
    let compression_info = cmf >> 4;
    assert_eq!(8, compression_method);
    assert!(
        7 >= compression_info,
        "Values of CINFO above 7 are not allowed"
    );
    let flg = data[1];
    let check = flg & 0b0001_1111;
    let preset_dictionary = flg & 0b0010_0000 != 0;
    let compression_level = flg >> 6;
    let check_check = 31 - ((cmf as u16 * 256 + (0b1110_0000 & flg as u16)) % 31);
    assert_eq!(check_check as u8, check);
    let level = match compression_level {
        0 => "fastest algorithm",
        1 => "fast  algorithm",
        2 => "default algorithm",
        3 => "maximum compression,slowest algorithm",
        _ => unreachable!(),
    };
    let len = data.len();
    let check = u32::from_be_bytes(data[len - 4..].try_into().unwrap());
    println!(
        "zlib info:\n\tpreset dictionary({preset_dictionary}),compression level:{level}({compression_level}),adler32(0X{check:08X})"
    );
    inflate(&data[2..len - 4], out);
    assert_eq!(adler32(&out), check, "Incorrect adler32");
}
pub fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    const NMAX: usize = 5_552;

    let mut a: u32 = 1;
    let mut b: u32 = 0;

    for chunk in data.chunks(NMAX) {
        for &byte in chunk {
            a += byte as u32;
            b += a;
        }

        a %= MOD;
        b %= MOD;
    }

    (b << 16) | a
}

// https://datatracker.ietf.org/doc/html/rfc1951 (deflate)
pub fn inflate(data: &[u8], out: &mut Vec<u8>) {
    let mut bfinal = false;
    let mut num = 1;
    let mut bit = BitReader::new(data);
    println!("deflate info:");
    loop {
        if bfinal {
            break;
        }
        bfinal = bit.read_bits(1).unwrap() == 1;

        let btype = bit.read_bits(2).unwrap();
        let compression = match btype {
            0 => "no compression",
            1 => "compressed with fixed Huffman codes",
            2 => "compressed with dynamic Huffman codes",
            3 | _ => unreachable!("reserved(error)"),
        };
        println!("\tblock {num:<3} {compression}({btype})");
        num += 1;
        match btype {
            0 => {
                stored(&mut bit, out);
            }
            1 => {
                fixed(&mut bit, out);
            }
            2 => {
                dynamic(&mut bit, out);
            }
            _ => unreachable!("reserved(error)"),
        }
    }
}

pub struct Huffman<'a> {
    pub count: &'a mut [usize],
    pub symbol: &'a mut [usize],
}
pub fn decode(bit: &mut BitReader, huffman: &Huffman) -> usize {
    let mut code: isize = 0;
    let mut first: isize = 0;
    let mut index: usize = 0;
    for len in 1..=15 {
        code |= bit.read_bits(1).unwrap() as isize;
        let count = huffman.count[len] as isize;
        if code - count < first {
            let symbol_index = index + (code - first) as usize;
            return huffman.symbol[symbol_index];
        }
        index += count as usize;
        first += count;
        first <<= 1;
        code <<= 1;
    }
    usize::MAX
}
pub fn construct(huffman: &mut Huffman, data: &[usize], n: usize) {
    for len in 0..=15 {
        huffman.count[len] = 0;
    }
    for symbol in 0..n {
        huffman.count[data[symbol]] += 1;
    }
    let mut offs = [0; 16];
    offs[1] = 0;
    for i in 1..15 {
        offs[i + 1] = offs[i] + huffman.count[i];
    }
    for symbol in 0..n {
        let len = data[symbol];

        if len != 0 {
            huffman.symbol[offs[len]] = symbol;
            offs[len] += 1;
        }
    }
}

pub struct BitReader<'a> {
    data: &'a [u8],
    index: usize,
    bits: u64,
    count: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            index: 0,
            bits: 0,
            count: 0,
        }
    }

    #[inline]
    fn fill(&mut self, n: usize) -> bool {
        while self.count < n {
            let Some(&byte) = self.data.get(self.index) else {
                return false;
            };

            self.bits |= (byte as u64) << self.count;
            self.count += 8;
            self.index += 1;
        }
        true
    }

    #[inline]
    pub fn read_bits(&mut self, n: usize) -> Option<u64> {
        debug_assert!(n <= 56);

        if !self.fill(n) {
            return None;
        }

        let value = self.bits & ((1u64 << n) - 1);

        self.bits >>= n;
        self.count -= n;

        Some(value)
    }

    #[inline]
    pub fn peek_bits(&mut self, n: usize) -> Option<u64> {
        debug_assert!(n <= 56);

        if !self.fill(n) {
            return None;
        }

        Some(self.bits & ((1u64 << n) - 1))
    }

    #[inline]
    pub fn skip_bits(&mut self, n: usize) -> bool {
        if !self.fill(n) {
            return false;
        }
        self.bits >>= n;
        self.count -= n;
        true
    }
    #[inline]
    pub fn align_byte(&mut self) {
        let n = self.count & 7;
        self.bits >>= n;
        self.count -= n;
    }
    #[inline]
    pub fn byte_position(&self) -> usize {
        self.index - self.count / 8
    }
    #[inline]
    pub fn take_bytes(&mut self, len: usize) -> Option<&'a [u8]> {
        self.align_byte();
        let pos = self.index - self.count / 8;

        let end = pos.checked_add(len)?;

        if end > self.data.len() {
            return None;
        }

        self.index = end;

        self.bits = 0;
        self.count = 0;

        Some(&self.data[pos..end])
    }
}

pub fn codes(
    data: &mut BitReader,
    lencode: &mut Huffman,
    distcode: &mut Huffman,
    out: &mut Vec<u8>,
) {
    let lens = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    let lext = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    let dists = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    let dext = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];
    let mut len;
    let mut dist;
    let mut symbol = 0;
    while symbol != 256 {
        symbol = decode(data, lencode);
        if symbol < 256 {
            out.push(symbol as u8);
        } else if symbol > 256 {
            symbol -= 257;
            assert!(symbol <= 29);
            len = lens[symbol] + data.read_bits(lext[symbol]).unwrap();
            symbol = decode(data, distcode);
            dist = dists[symbol] + data.read_bits(dext[symbol]).unwrap();
            while len > 0 {
                let value = out[out.len() - dist as usize];
                len -= 1;
                out.push(value);
            }
        }
    }
}
pub fn stored(data: &mut BitReader, out: &mut Vec<u8>) {
    data.align_byte();
    let len = data.read_bits(16).unwrap() as u16;
    let nlen = data.read_bits(16).unwrap() as u16;
    assert_eq!(!len, nlen);
    out.extend_from_slice(data.take_bytes(len as usize).unwrap());
}
pub fn fixed(data: &mut BitReader, out: &mut Vec<u8>) {
    let mut lencount = [0usize; 16];
    let mut lensym = [0usize; 288];
    let mut distcount = [0usize; 16];
    let mut distsym = [0usize; 30];
    let mut lencode = Huffman {
        count: &mut lencount,
        symbol: &mut lensym,
    };
    let mut distcode = Huffman {
        count: &mut distcount,
        symbol: &mut distsym,
    };
    let mut arr = [0usize; 288];
    let mut i = 0;
    while i <= 143 {
        arr[i] = 8;
        i += 1;
    }
    while i <= 255 {
        arr[i] = 9;
        i += 1;
    }
    while i <= 279 {
        arr[i] = 7;
        i += 1;
    }
    while i <= 287 {
        arr[i] = 8;
        i += 1;
    }
    construct(&mut lencode, &arr, 288);
    i = 0;
    while i < 30 {
        arr[i] = 5;
        i += 1;
    }
    construct(&mut distcode, &arr, 30);
    codes(data, &mut lencode, &mut distcode, out);
}
pub fn dynamic(data: &mut BitReader, out: &mut Vec<u8>) {
    let order = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let mut lengths = [0; 286 + 30];
    let mut lencount = [0usize; 16];
    let mut lensym = [0usize; 286];
    let mut distcount = [0usize; 16];
    let mut distsym = [0usize; 30];
    let mut lencode = Huffman {
        count: &mut lencount,
        symbol: &mut lensym,
    };
    let mut distcode = Huffman {
        count: &mut distcount,
        symbol: &mut distsym,
    };
    let nlen = data.read_bits(5).unwrap() + 257;
    let ndist = data.read_bits(5).unwrap() + 1;
    let ncode = data.read_bits(4).unwrap() + 4;

    for i in 0..ncode as usize {
        lengths[order[i]] = data.read_bits(3).unwrap() as usize;
    }
    construct(&mut lencode, &lengths, 19);
    let mut index = 0;
    while index < nlen + ndist {
        let mut symbol;
        let mut len;
        symbol = decode(data, &lencode);
        if symbol < 16 {
            lengths[index as usize] = symbol;
            index += 1;
        } else {
            len = 0;
            if symbol == 16 {
                len = lengths[index as usize - 1];
                symbol = 3 + data.read_bits(2).unwrap() as usize;
            } else if symbol == 17 {
                symbol = 3 + data.read_bits(3).unwrap() as usize;
            } else {
                symbol = 11 + data.read_bits(7).unwrap() as usize;
            }
            while symbol > 0 {
                lengths[index as usize] = len;
                index += 1;
                symbol -= 1;
            }
        }
    }

    construct(&mut lencode, &lengths, nlen as usize);
    construct(&mut distcode, &lengths[nlen as usize..], ndist as usize);

    codes(data, &mut lencode, &mut distcode, out);
}

// https://www.w3.org/TR/png-3/#11IHDR
// struct IHDR {
//     length: u32,
//     chunk_type: [u8; 4],
//     width: u32,
//     height: u32,
//     bit_width: u8,
//     color_type: u8,
//     compression_method: u8,
//     filter_method: u8,
//     interlace_method: u8,
//     crc: u32,
// }
pub struct IHDRChunkData {
    pub width: u32,
    pub height: u32,
    pub bit_width: u8,
    pub color_type: u8,
    pub compression_method: u8,
    pub filter_method: u8,
    pub interlace_method: u8,
}
impl IHDRChunkData {
    pub fn parse(buf: &[u8]) -> IHDRChunkData {
        let width = u32::from_be_bytes(buf[0..4].try_into().unwrap());
        let height = u32::from_be_bytes(buf[4..8].try_into().unwrap());
        let bit_width = buf[8];
        let color_type = buf[9];
        let compression_method = buf[10];
        let filter_method = buf[11];
        let interlace_method = buf[12];
        IHDRChunkData {
            width,
            height,
            bit_width,
            color_type,
            compression_method,
            filter_method,
            interlace_method,
        }
    }
}

// https://www.w3.org/TR/png-3/#12Encoder-gamma-handling
pub fn gamma_handling() {}

// https://www.w3.org/TR/png-3/#12Filter-selection
pub fn choose_filter(row: &[u8], _pre_row: &[u8], _bit_depth: u8) {
    // Filters are applied to bytes
    let abs_byte = |byte: u8| -> u64 { (byte as i8).unsigned_abs() as u64 };
    let _none: u64 = row.iter().map(|&byte| abs_byte(byte)).sum();
}

// https://www.w3.org/TR/png-3/#13Decoders.Errors

// https://www.w3.org/TR/png-3/#13Error-checking
// only for unrecognized chunk types.
pub fn error_checking(chunk: Chunk<'_>) {
    let chunk_type = chunk.chunk_type;
    for c in chunk_type {
        assert!((0x41..=0x5A).contains(&c) || (0x61..=0x7A).contains(&c));
    }
}

// https://www.w3.org/TR/png-3/#14Additional-chunk-types
// https://w3c.github.io/png/extensions/Overview.html
pub fn read_chunk(buf: &[u8]) -> Vec<Chunk<'_>> {
    let len = buf.len();
    let mut pos = 0;
    let mut chunk;
    let mut chunks = Vec::new();
    let mut length;
    let mut chunk_type;
    let mut data;
    let mut crc;
    loop {
        if pos >= len {
            break;
        }
        length = u32::from_be_bytes(buf[pos..pos + 4].try_into().unwrap());
        chunk_type = buf[pos + 4..pos + 8].try_into().unwrap();
        pos += 8;
        data = &buf[pos..pos + length as usize];
        pos += length as usize;
        crc = u32::from_be_bytes(buf[pos..pos + 4].try_into().unwrap());
        pos += 4;
        chunk = Chunk {
            length,
            chunk_type,
            data,
            crc,
        };
        chunks.push(chunk);
    }
    chunks
}

pub fn parse_chunk(chunks: &Vec<Chunk<'_>>) {
    check_chunk_count(chunks);
    check_chunk_ordering(chunks);
    let IHDRChunkData {
        width,
        height,
        bit_width,
        color_type,
        compression_method,
        filter_method,
        interlace_method,
    } = IHDRChunkData::parse(chunks[0].data);
    let image_type = ImageType::image_type(color_type);
    let compression = CompressionMethod::method(compression_method);
    let filter = FilterMethod::method(filter_method);
    let interlace = InterlaceMethod::method(interlace_method);
    println!(
        "{width} x {height} image,{bit_width}bit/channal,\
        {image_type:?}({color_type}),compression({compression:?}),\
        filter({filter:?}),interlace({interlace:?})"
    );

    // https://www.w3.org/TR/png-3/#11PLTE
    let mut plte: Option<&[u8]> = None;
    for chunk in chunks {
        if &chunk.chunk_type == b"PLTE" {
            plte = Some(chunk.data);
        }
    }
    if plte.is_some() {
        let plte = plte.unwrap();
        assert_eq!(
            plte.len() % 3,
            0,
            "The length of the PLTE chunk must be divisible by 3."
        );
    }

    // https://www.w3.org/TR/png-3/#11IDAT
    let idat_chunks: Vec<&Chunk<'_>> = chunks
        .iter()
        .filter(|chunk| chunk.chunk_type == *b"IDAT")
        .collect();
    let mut len = 0;
    for idat in &idat_chunks {
        len += idat.length;
    }
    let mut idat_data = Vec::with_capacity(len as usize);
    for idat in idat_chunks {
        idat_data.extend_from_slice(idat.data);
    }

    let mut out: Vec<u8> = Vec::with_capacity(4 * 1920 * 1080 + 1080);
    zlib(&idat_data, &mut out);

    //https://www.w3.org/TR/png-3/#7Scanline
    let len = out.len();
    let channal = image_type.channal();
    let height = height as usize;

    let scan = len / height;
    let mut prev_scanline = vec![0; scan - 1];

    for i in 0..height {
        let start = scan * i;
        let end = scan * (i + 1);
        let filter = FilterType::filter_type(out[start]);
        filter.apply(
            &mut out[start + 1..end],
            &prev_scanline,
            channal * bit_width as usize / 8,
        );
        prev_scanline.copy_from_slice(&out[start + 1..end]);
    }
    // let mut rgb = Vec::with_capacity(height * (scan - 1));
    // for i in 0..height {
    //     let start = scan * i;
    //     let end = scan * (i + 1);
    //     rgb.extend_from_slice(&out[start + 1..end]);
    // }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut file = File::open(&args[1])?;
    println!("{args:?}");
    let file_size = file.metadata()?.len() as usize;
    let mut buf = Vec::with_capacity(file_size);
    file.read_to_end(&mut buf)?;
    assert_eq!(buf[..8], PNG_SIGNATURE, "Incorrect PNG format signature.");
    let chunks = read_chunk(&buf[8..]);
    parse_chunk(&chunks);
    for chunk in chunks {
        chunk.verify_crc();
        println!("chunk(\"{}\"),length {}", chunk.chunk_type(), chunk.length);
        let len = chunk.length;
        let data = chunk.data;
        match &chunk.chunk_type {
            // https://www.w3.org/TR/png-3/#11tEXt
            // struct tEXt<'a> {
            //     length: u32,
            //     chunk_type: [u8; 4],
            //     keyword: &'a [u8],
            //     null: u8,
            //     text_string: &'a [u8],
            //     crc: u32,
            // }
            b"tEXt" => {
                let null_pos = data.iter().position(|&b| b == 0).unwrap();

                let keyword = str::from_utf8(&data[..null_pos]).unwrap();
                let text_string = str::from_utf8(&data[null_pos + 1..]).unwrap();

                println!("\tkeyword:{keyword},text_string:{text_string}");
            }
            // https://www.w3.org/TR/png-3/#11iTXt
            // struct iTXt<'a> {
            //     length: u32,
            //     chunk_type: [u8; 4],
            //     keyword: &'a [u8],
            //     null: u8,
            //     compression_flag: u8,
            //     compression_method: u8,
            //     language_tag: &'a [u8],
            //     null2: u8,
            //     translated_keyword: &'a [u8],
            //     null3: u8,
            //     text: &'a [u8],
            //     crc: u32,
            // }
            b"iTXt" => {
                let null_pos = data.iter().position(|&b| b == 0).unwrap();

                let keyword = str::from_utf8(&data[..null_pos]).unwrap();

                let compression_flag = data[null_pos + 1];
                let compression_method = data[null_pos + 2];

                // language_tag
                let language_tag_start = null_pos + 3;

                let null_pos2 = data[language_tag_start..]
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap();

                let language_tag_end = language_tag_start + null_pos2;

                let language_tag =
                    str::from_utf8(&data[language_tag_start..language_tag_end]).unwrap();

                // translated_keyword
                let translated_keyword_start = language_tag_end + 1;

                let null_pos3 = data[translated_keyword_start..]
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap();

                let translated_keyword_end = translated_keyword_start + null_pos3;

                let translated_keyword =
                    str::from_utf8(&data[translated_keyword_start..translated_keyword_end])
                        .unwrap();

                // text
                let text = str::from_utf8(&data[translated_keyword_end + 1..]).unwrap();

                println!(
                    "\tkeyword:{keyword}, \
     compression_flag:{compression_flag}, \
     compression_method:{compression_method}, \
     language_tag:{language_tag}, \
     translated_keyword:{translated_keyword}, \
     text:{text}"
                );
            }
            // https://www.w3.org/TR/png-3/#11pHYs
            // struct pHYs{
            //     length: u32,
            //     chunk_type: [u8; 4],
            //     pixels_per_unit_x_axis:u32,
            //     pixels_per_unit_y_axis:u32,
            //     unit_specifier:u8, (0) unit is unknown  (1)unit is the metre
            //     crc: u32,
            // }
            b"pHYs" => {
                // Physical pixel dimensions
                assert_eq!(9, len);
                let pixels_per_unit_x_axis = u32::from_be_bytes(data[..4].try_into().unwrap());
                let pixels_per_unit_y_axis = u32::from_be_bytes(data[4..8].try_into().unwrap());
                let unit_specifier = match data[8] {
                    0 => "unit is unknown",
                    1 => "unit is the metre",
                    _ => "Error",
                };
                println!(
                    "\tx:{pixels_per_unit_x_axis},y:{pixels_per_unit_y_axis},{unit_specifier}"
                );
            }
            // https://www.w3.org/TR/png-3/#srgb-standard-colour-space
            // struct sRGB {
            //     length: u32,
            //     chunk_type: [u8; 4],
            //     rendering_intent: u8,
            //     crc: u32,
            // }
            b"sRGB" => {
                // Standard RGB color space
                assert_eq!(1, len);
                let rendering_intent = match data[0] {
                    0 => "Perceptual",
                    1 => "Relative colorimetric",
                    2 => "Saturation",
                    3 => "Absolute colorimetric",
                    _ => "Error",
                };
                println!("\trendering_intent:{rendering_intent}");
            }
            // https://www.w3.org/TR/png-3/#11cHRM
            // struct cHRM {
            //     length: u32,
            //     chunk_type: [u8; 4],
            //     white_point_x: u32, representing the x or y value times 100000.
            //     white_point_y: u32,
            //     red_x: u32,
            //     red_y: u32,
            //     green_x: u32,
            //     green_y: u32,
            //     blue_x: u32,
            //     blue_y: u32,
            //     crc: u32,
            // }
            b"cHRM" => {
                // Primary chromaticities and white point
                assert_eq!(32, len);
                let white_point_x =
                    u32::from_be_bytes(data[..4].try_into().unwrap()) as f64 / 100000.0;
                let white_point_y =
                    u32::from_be_bytes(data[4..8].try_into().unwrap()) as f64 / 100000.0;
                let red_x = u32::from_be_bytes(data[8..12].try_into().unwrap()) as f64 / 100000.0;
                let red_y = u32::from_be_bytes(data[12..16].try_into().unwrap()) as f64 / 100000.0;
                let green_x =
                    u32::from_be_bytes(data[16..20].try_into().unwrap()) as f64 / 100000.0;
                let green_y =
                    u32::from_be_bytes(data[20..24].try_into().unwrap()) as f64 / 100000.0;
                let blue_x = u32::from_be_bytes(data[24..28].try_into().unwrap()) as f64 / 100000.0;
                let blue_y = u32::from_be_bytes(data[28..32].try_into().unwrap()) as f64 / 100000.0;
                println!(
                    "\twhite_point_x:{white_point_x},white_point_y:{white_point_y},red_x:{red_x},red_y:{red_y},green_x:{green_x},green_y:{green_y},blue_x:{blue_x},blue_y:{blue_y}"
                );
            }
            // https://www.w3.org/TR/png-3/#11gAMA
            // struct acTL {
            //     length: u32,
            //     chunk_type: [u8; 4],
            //     gamma：u32,
            //     crc: u32,
            // }
            b"gAMA" => {
                // Image gamma
                assert_eq!(4, len);
                let gamma = u32::from_be_bytes(data[..4].try_into().unwrap()) as f64 / 100000.0;
                println!("\tgamma:{gamma}");
            }
            // https://www.w3.org/TR/png-3/#acTL-chunk
            // struct acTL {
            //     length: u32,
            //     chunk_type: [u8; 4],
            //     num_frames: u32,
            //     num_plays: u32,
            //     crc: u32,
            // }
            b"acTL" => {
                // Animation Control Chunk
                assert_eq!(8, len);

                let num_frames = u32::from_be_bytes(data[..4].try_into().unwrap());
                let num_plays = u32::from_be_bytes(data[4..].try_into().unwrap());
                println!("\tnum_frames:{num_frames},num_plays:{num_plays}");
            }
            // https://www.w3.org/TR/png-3/#fcTL-chunk
            // struct fcTL {
            //     length: u32,
            //     chunk_type: [u8; 4],
            //     sequence_number: u32,
            //     width: u32,
            //     height: u32,
            //     x_offset: u32,
            //     y_offset: u32,
            //     delay_num: u16,
            //     delay_den: u16, If the denominator is 0, it is to be treated as if it were 100
            //     dispose_op: u8,
            //     blend_op: u8,
            //     crc: u32,
            // }
            b"fcTL" => {
                // Frame Control Chunk
                assert_eq!(26, len);
                let sequence_number = u32::from_be_bytes(data[..4].try_into().unwrap());
                let width = u32::from_be_bytes(data[4..8].try_into().unwrap());
                let height = u32::from_be_bytes(data[8..12].try_into().unwrap());
                let x_offset = u32::from_be_bytes(data[12..16].try_into().unwrap());
                let y_offset = u32::from_be_bytes(data[16..20].try_into().unwrap());
                let delay_num = u16::from_be_bytes(data[20..22].try_into().unwrap());
                let delay_den = u16::from_be_bytes(data[22..24].try_into().unwrap());
                let dispose_op = data[24];
                let blend_op = data[25];
                println!(
                    "\tsequence_number:{sequence_number},width:{width},height:{height},x_offset:{x_offset},y_offset:{y_offset},delay_num:{delay_num},delay_den:{delay_den},dispose_op:{dispose_op},blend_op:{blend_op}"
                );
            }
            // https://www.w3.org/TR/png-3/#fdAT-chunk
            // struct fdAT {
            //     length: u32,
            //     chunk_type: [u8; 4],
            //     sequence_number: u32,
            //     frame_data: [u8; length - 4],
            //     crc: u32,
            // }
            b"fdAT" => {
                // Frame Data Chunk
                let sequence_number = u32::from_be_bytes(data[..4].try_into().unwrap());
                println!("\tsequence_number:{sequence_number:<3}");
                // let mut out: Vec<u8> = Vec::with_capacity(1920 * 1080);
                // zlib(&data[4..], &mut out);
            }
            // https://www.w3.org/TR/png-3/#11IEND
            // pub struct IEND {
            //     pub length: u32,
            //     pub chunk_type: [u8; 4],
            //     pub crc: u32,
            // }
            b"IEND" => { // Image trailer
            }
            _ => {}
        };
    }

    Ok(())
}
