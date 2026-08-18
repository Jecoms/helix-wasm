//! Packing several files into one, so a host can save them in a single go.
//!
//! [`crate::download`] hands the host one file at a time, which is all a
//! browser download is; a session that wants its whole [`crate::vfs`] out
//! therefore needs the many files to *become* one first. Zip is what that
//! one file has to be: it is the only container a reader can open with
//! nothing installed on any of the three desktop platforms, and the only
//! one a browser's own compression primitives cannot build (a
//! `CompressionStream` produces a gzip *stream*, not an archive).
//!
//! Entries are **stored**, not deflated. Nothing in this tree links a
//! compressor, and pulling one in to shrink an archive of a few text
//! buffers would cost more bundle than it ever saves on the wire — the
//! sizes involved are a hand-typed session's, not a repository's. Stored
//! entries also keep this module to the part of the format that is fixed
//! and checkable: local headers, a central directory, and an end-of-
//! central-directory record, all of them 32-bit, with a CRC over each
//! file's bytes.
//!
//! No zip64. Every size and offset the format writes here is a `u32`, and
//! the archive is built in wasm32 linear memory, which tops out at 4 GiB —
//! so a file or an archive large enough to need the 64-bit records cannot
//! be assembled in the first place.
//!
//! Gated to wasm32 and `test` exactly as [`crate::vfs`] and
//! [`crate::download`] are: only wasm32 code paths have anything to pack,
//! and the `test` arm is there so the unit tests below run on the host.

use std::time::{SystemTime, UNIX_EPOCH};

/// The `version needed to extract` (and `version made by`) written into
/// every header: 2.0, which is the floor for the stored method and the
/// directory layout below. The upper byte of `version made by` stays 0
/// (MS-DOS/FAT), which is what leaving the external attributes empty means.
const VERSION: u16 = 20;

/// General-purpose bit 11: the file name is UTF-8. Store keys are Rust
/// strings, so this is the truth, and without it an extractor is entitled
/// to read a non-ASCII name as CP437.
const UTF8_NAMES: u16 = 0x0800;

/// The stored (uncompressed) compression method.
const STORED: u16 = 0;

/// MS-DOS dates count years from 1980 and pack them into 7 bits, so this is
/// the whole range the format can express. A timestamp outside it is
/// clamped rather than wrapped: a wrapped date is a lie, and a clamped one
/// is visibly the edge of what the container holds.
const DOS_YEAR_MIN: u64 = 1980;
const DOS_YEAR_MAX: u64 = 2107;

/// A zip archive under construction.
///
/// Built in one pass: [`add`](Zip::add) appends a file's local header and
/// its bytes to the output while recording the matching central-directory
/// entry, and [`finish`](Zip::finish) appends that directory and the record
/// pointing at it.
pub struct Zip {
    /// The archive so far: local headers and file data, and after
    /// [`finish`](Zip::finish) the central directory too.
    out: Vec<u8>,
    /// The central directory, accumulated alongside `out` because each of
    /// its entries needs the offset its local header went to.
    central: Vec<u8>,
    entries: u16,
}

impl Default for Zip {
    fn default() -> Self {
        Self::new()
    }
}

impl Zip {
    pub fn new() -> Self {
        Zip {
            out: Vec::new(),
            central: Vec::new(),
            entries: 0,
        }
    }

    /// Adds one stored file: `name` is its path *inside* the archive (`/`
    /// separated and relative, which is what every extractor expects — a
    /// leading slash or a `..` is what makes an archive refuse to unpack),
    /// `modified` the time the extracted file is stamped with.
    ///
    /// Names are not deduplicated. Two entries under one name make an
    /// archive whose extractors disagree with each other, so callers pass
    /// keys from a set — the virtual file system's, in this tree.
    pub fn add(&mut self, name: &str, contents: &[u8], modified: SystemTime) {
        let (time, date) = dos_timestamp(modified);
        let crc = crc32(contents);
        let offset = self.out.len() as u32;
        let size = contents.len() as u32;
        let name = name.as_bytes();
        let name_len = name.len() as u16;

        // Local file header, then the bytes themselves. Sizes and CRC go in
        // the header up front rather than in a trailing data descriptor:
        // everything is already in memory, so there is nothing to stream.
        self.out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        self.out.extend_from_slice(&VERSION.to_le_bytes());
        self.out.extend_from_slice(&UTF8_NAMES.to_le_bytes());
        self.out.extend_from_slice(&STORED.to_le_bytes());
        self.out.extend_from_slice(&time.to_le_bytes());
        self.out.extend_from_slice(&date.to_le_bytes());
        self.out.extend_from_slice(&crc.to_le_bytes());
        self.out.extend_from_slice(&size.to_le_bytes()); // compressed
        self.out.extend_from_slice(&size.to_le_bytes()); // uncompressed
        self.out.extend_from_slice(&name_len.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes()); // extra field length
        self.out.extend_from_slice(name);
        self.out.extend_from_slice(contents);

        // The central-directory entry: the same fields, plus where the
        // local header just went. This is the copy extractors actually
        // read, which is why the archive is unusable until `finish` writes
        // the directory out.
        self.central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        self.central.extend_from_slice(&VERSION.to_le_bytes()); // made by
        self.central.extend_from_slice(&VERSION.to_le_bytes()); // needed
        self.central.extend_from_slice(&UTF8_NAMES.to_le_bytes());
        self.central.extend_from_slice(&STORED.to_le_bytes());
        self.central.extend_from_slice(&time.to_le_bytes());
        self.central.extend_from_slice(&date.to_le_bytes());
        self.central.extend_from_slice(&crc.to_le_bytes());
        self.central.extend_from_slice(&size.to_le_bytes()); // compressed
        self.central.extend_from_slice(&size.to_le_bytes()); // uncompressed
        self.central.extend_from_slice(&name_len.to_le_bytes());
        self.central.extend_from_slice(&0u16.to_le_bytes()); // extra
        self.central.extend_from_slice(&0u16.to_le_bytes()); // comment
        self.central.extend_from_slice(&0u16.to_le_bytes()); // disk number
        self.central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        self.central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        self.central.extend_from_slice(&offset.to_le_bytes());
        self.central.extend_from_slice(name);

        self.entries += 1;
    }

    /// The finished archive.
    ///
    /// An archive with no entries is still a valid one — an empty zip is
    /// its end-of-central-directory record and nothing else — so callers
    /// that would rather say "nothing to export" have to decide that for
    /// themselves.
    pub fn finish(mut self) -> Vec<u8> {
        let offset = self.out.len() as u32;
        let size = self.central.len() as u32;
        self.out.extend_from_slice(&self.central);

        // End of central directory: one disk, this many entries, and where
        // to find them. No archive comment.
        self.out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes()); // this disk
        self.out.extend_from_slice(&0u16.to_le_bytes()); // disk with the cd
        self.out.extend_from_slice(&self.entries.to_le_bytes()); // on this disk
        self.out.extend_from_slice(&self.entries.to_le_bytes()); // total
        self.out.extend_from_slice(&size.to_le_bytes());
        self.out.extend_from_slice(&offset.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes()); // comment length
        self.out
    }
}

/// CRC-32/ISO-HDLC — the check zip entries carry — computed a bit at a
/// time.
///
/// Bitwise rather than table-driven on purpose: a 1 KiB table is a bigger
/// addition to a wasm bundle than the eight shifts per byte are to the time
/// this runs, and what it runs over is a session's hand-typed text.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            // Branchless conditional xor: `0u32.wrapping_sub(crc & 1)` is
            // all ones when the low bit is set and zero when it is not.
            crc = (crc >> 1) ^ (0xEDB8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

/// `time` as the (dos time, dos date) pair every zip header carries: a
/// local time with two-second resolution, years counted from 1980.
///
/// "Local" is the format's word for it, not a conversion — there is no zone
/// here and none in a wasm32 session, so this writes UTC and an extractor
/// reads it as whatever its own rules say. The alternative is the extended
/// timestamp extra field, which is optional, unevenly supported, and buys
/// an hour of accuracy on files that exist to be re-opened, not audited.
fn dos_timestamp(time: SystemTime) -> (u16, u16) {
    let secs = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (year, month, day) = civil_from_days(secs / 86_400);
    // Clamped, not wrapped: see DOS_YEAR_MIN/MAX. Below the floor the date
    // becomes 1980-01-01, which is the zero of this field's calendar.
    if year < DOS_YEAR_MIN {
        return (0, 1 << 5 | 1);
    }
    let year = year.min(DOS_YEAR_MAX);
    let seconds_of_day = secs % 86_400;
    let time = (seconds_of_day / 3600) << 11
        | (seconds_of_day % 3600 / 60) << 5
        // Two-second resolution is the format's, not a rounding choice.
        | (seconds_of_day % 60) / 2;
    let date = (year - DOS_YEAR_MIN) << 9 | u64::from(month) << 5 | u64::from(day);
    (time as u16, date as u16)
}

/// The (year, month, day) `days` days after 1970-01-01, in the proleptic
/// Gregorian calendar.
///
/// Howard Hinnant's `civil_from_days`
/// (<https://howardhinnant.github.io/date_algorithms.html#civil_from_days>),
/// restricted to non-negative days — the only ones a `SystemTime` measured
/// from the unix epoch can produce. It shifts the year to start in March so
/// the leap day lands at the end of it, which is what makes the month
/// lengths a single linear formula.
fn civil_from_days(days: u64) -> (u64, u32, u32) {
    // Re-base onto 0000-03-01, the start of a 400-year era.
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z % 146_097; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year =
        day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100); // [0, 365]
    let march_month = (5 * day_of_year + 2) / 153; // [0, 11], 0 = March
    let day = (day_of_year - (153 * march_month + 2) / 5 + 1) as u32; // [1, 31]
    let month = if march_month < 10 {
        march_month + 3
    } else {
        march_month - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// The bytes at `offset`, as a `u32`.
    fn u32_at(zip: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(zip[offset..offset + 4].try_into().unwrap())
    }

    fn u16_at(zip: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes(zip[offset..offset + 2].try_into().unwrap())
    }

    fn at(year: u64, month: u64, day: u64, hour: u64, minute: u64, second: u64) -> SystemTime {
        // Days since the epoch the long way round, so the fixture does not
        // lean on the algorithm it is checking.
        let days = (1970..year)
            .map(|y| {
                if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                    366
                } else {
                    365
                }
            })
            .sum::<u64>()
            + (1..month)
                .map(|m| match m {
                    1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                    4 | 6 | 9 | 11 => 30,
                    _ if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
                    _ => 28,
                })
                .sum::<u64>()
            + (day - 1);
        UNIX_EPOCH + Duration::from_secs(days * 86_400 + hour * 3600 + minute * 60 + second)
    }

    #[test]
    fn crc32_matches_the_known_check_values() {
        // The two vectors every CRC catalogue lists for CRC-32/ISO-HDLC.
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"The quick brown fox jumps over the lazy dog"), 0x414F_A339);
    }

    #[test]
    fn civil_from_days_walks_the_calendar() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(59), (1970, 3, 1));
        // 1972 is a leap year, so it has a 29 February to land on.
        assert_eq!(civil_from_days(789), (1972, 2, 29));
        // 2000 is a leap year (divisible by 400) where 1900 was not.
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(20_315), (2025, 8, 15));
    }

    #[test]
    fn dos_timestamps_pack_the_calendar_fields() {
        let (time, date) = dos_timestamp(at(2025, 8, 15, 13, 45, 31));
        assert_eq!(date, ((2025 - 1980) << 9) | (8 << 5) | 15);
        // Seconds have two-second resolution: 31 stores as 15.
        assert_eq!(time, (13 << 11) | (45 << 5) | 15);
    }

    #[test]
    fn dos_timestamps_clamp_to_the_range_the_field_holds() {
        // The unix epoch predates the DOS one by a decade; it becomes the
        // floor rather than wrapping to some year in the 2000s.
        assert_eq!(dos_timestamp(UNIX_EPOCH), (0, (1 << 5) | 1));
        let (_, date) = dos_timestamp(at(2200, 6, 3, 0, 0, 0));
        assert_eq!(date >> 9, (2107 - 1980));
    }

    #[test]
    fn an_empty_archive_is_just_the_end_record() {
        let zip = Zip::new().finish();
        assert_eq!(zip.len(), 22);
        assert_eq!(u32_at(&zip, 0), 0x0605_4b50);
        assert_eq!(u16_at(&zip, 10), 0); // total entries
    }

    #[test]
    fn an_entry_is_stored_whole_with_its_crc() {
        let mut zip = Zip::new();
        zip.add("notes.txt", b"hello", at(2025, 8, 15, 13, 45, 30));
        let zip = zip.finish();

        // Local header, then the name, then the bytes verbatim — stored,
        // so the file is findable in the archive as-is.
        assert_eq!(u32_at(&zip, 0), 0x0403_4b50);
        assert_eq!(u16_at(&zip, 8), STORED);
        assert_eq!(u16_at(&zip, 6), UTF8_NAMES);
        assert_eq!(u32_at(&zip, 14), crc32(b"hello"));
        assert_eq!(u32_at(&zip, 18), 5); // compressed size
        assert_eq!(u32_at(&zip, 22), 5); // uncompressed size
        assert_eq!(&zip[30..39], b"notes.txt");
        assert_eq!(&zip[39..44], b"hello");

        // The central directory follows the data and the end record points
        // at it; the local header it names is at the top of the archive.
        assert_eq!(u32_at(&zip, 44), 0x0201_4b50);
        let end = zip.len() - 22;
        assert_eq!(u32_at(&zip, end), 0x0605_4b50);
        assert_eq!(u16_at(&zip, end + 10), 1);
        assert_eq!(u32_at(&zip, end + 12), (end - 44) as u32); // directory size
        assert_eq!(u32_at(&zip, end + 16), 44); // directory offset
        assert_eq!(u32_at(&zip, 44 + 42), 0); // this entry's local header
    }

    #[test]
    fn every_entry_gets_a_directory_record_pointing_at_its_own_header() {
        let mut zip = Zip::new();
        zip.add("a.txt", b"first", UNIX_EPOCH);
        zip.add("dir/b.txt", b"second", UNIX_EPOCH);
        let zip = zip.finish();

        let end = zip.len() - 22;
        assert_eq!(u16_at(&zip, end + 8), 2); // entries on this disk
        assert_eq!(u16_at(&zip, end + 10), 2); // entries in total

        // Walk the directory the way an extractor does and check each
        // record's offset really lands on a local header for that name.
        let mut offset = u32_at(&zip, end + 16) as usize;
        for name in ["a.txt", "dir/b.txt"] {
            assert_eq!(u32_at(&zip, offset), 0x0201_4b50);
            let name_len = u16_at(&zip, offset + 28) as usize;
            assert_eq!(&zip[offset + 46..offset + 46 + name_len], name.as_bytes());
            let local = u32_at(&zip, offset + 42) as usize;
            assert_eq!(u32_at(&zip, local), 0x0403_4b50);
            assert_eq!(&zip[local + 30..local + 30 + name_len], name.as_bytes());
            offset += 46 + name_len;
        }
        assert_eq!(offset, end);
    }
}
