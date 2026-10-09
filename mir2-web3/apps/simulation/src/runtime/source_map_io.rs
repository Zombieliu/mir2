//! Bounded ordinary map I/O, not a Source `Map.Load` success rule or receipt.
//!
//! Crystal Map.cs:436-440 reads the file before decoding; successful `Load`
//! additionally initializes the world (432-513). These preparation limits only
//! make the existing raw/gzip byte providers unavailable on oversized input,
//! I/O/decode errors, or unavailable output allocation. They create no loaded
//! map, population, empty-map authority, fallback, cache entry, or identity.
//! The checked raw result preserves failure origin so ordinary collision callers
//! may retain their existing pack fallback on actual I/O/allocation failure
//! without adding fallback for a raw file that exceeds the decoder byte domain.
//!
//! Raw and decoded bytes share source_map_loading::MAX_TERRAIN_BYTES. Compressed
//! actual reads/prefetch have that limit plus 1 MiB; this is not a limit on the
//! entire gzip file's metadata length. As with the existing read::GzDecoder,
//! only the first member is decoded. Its complete footer (CRC and ISIZE) must
//! validate, but ignored trailing bytes or later members are not scanned.
//!
//! Our output Vec grows fallibly and requests/reports capacity no larger than
//! its limit. Fixed stack buffers bound read/prefetch. The existing flate2
//! implementation still makes finite, non-fallible header/deflate workspace
//! allocations internally; its public API cannot guarantee recovery from all
//! process-wide allocation failures. No new unbounded read_to_end is used.

use super::source_map_loading::MAX_TERRAIN_BYTES;
use flate2::bufread::GzDecoder;
use std::fs::File;
use std::io::{self, BufRead, Read};
use std::path::Path;

const BUFFER_BYTES: usize = 8 * 1024;
// An I/O Candidate bound, not part of original Source Map.Load semantics.
const COMPRESSED_IO_ALLOWANCE: usize = 1024 * 1024;

#[derive(Debug)]
pub(super) enum RawMapReadFailure {
    Io(io::Error),
    Limit,
    AllocationUnavailable,
}

impl RawMapReadFailure {
    /// Existing raw I/O/allocation failure may try the pack. A raw file read
    /// past the already-bounded decoder domain must not add a new fallback.
    /// In particular, actual I/O InvalidData is not classified as Limit.
    pub(super) fn can_try_pack(&self) -> bool {
        matches!(self, Self::Io(_) | Self::AllocationUnavailable)
    }

    fn into_io_error(self) -> io::Error {
        match self {
            Self::Io(error) => error,
            Self::Limit => limit_error(),
            Self::AllocationUnavailable => io::ErrorKind::OutOfMemory.into(),
        }
    }
}

pub(super) fn read_raw_map_file_checked(path: &Path) -> Result<Vec<u8>, RawMapReadFailure> {
    let file = File::open(path).map_err(RawMapReadFailure::Io)?;
    read_raw_reader_checked(file, MAX_TERRAIN_BYTES)
}

pub(super) fn read_raw_map_file(path: &Path) -> Option<Vec<u8>> {
    read_raw_map_file_checked(path).ok()
}

pub(super) fn read_gzip_map_file(path: &Path) -> Option<Vec<u8>> {
    let compressed_limit = MAX_TERRAIN_BYTES.checked_add(COMPRESSED_IO_ALLOWANCE)?;
    read_gzip_reader(File::open(path).ok()?, MAX_TERRAIN_BYTES, compressed_limit).ok()
}

fn limit_error() -> io::Error {
    io::ErrorKind::InvalidData.into()
}

/// Retry actual interrupted reads, but do not accept an invalid Read count and
/// let it cause slice/index arithmetic to panic in the bounded accumulation.
fn read_retry(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    loop {
        match reader.read(buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Ok(read) if read > buffer.len() => return Err(limit_error()),
            result => return result,
        }
    }
}

fn append_bounded(
    output: &mut Vec<u8>,
    bytes: &[u8],
    limit: usize,
) -> Result<(), RawMapReadFailure> {
    let new_len = output
        .len()
        .checked_add(bytes.len())
        .ok_or(RawMapReadFailure::Limit)?;
    if new_len > limit || output.capacity() > limit {
        return Err(RawMapReadFailure::Limit);
    }

    if output.capacity() < new_len {
        let capacity = output
            .capacity()
            .saturating_mul(2)
            .max(BUFFER_BYTES)
            .max(new_len)
            .min(limit);
        output
            .try_reserve_exact(capacity - output.len())
            .map_err(|_| RawMapReadFailure::AllocationUnavailable)?;
        if output.capacity() > limit {
            return Err(RawMapReadFailure::Limit);
        }
    }
    // The fallible reserve above supplies this append's complete capacity.
    output.extend_from_slice(bytes);
    Ok(())
}

/// At exactly limit bytes, read once more with a nonempty one-byte buffer.
/// Real EOF succeeds; limit+1 fails without appending the excess byte. For a
/// gzip decoder this extra read also forces full footer validation at the
/// decoded boundary instead of treating the output cap as successful EOF.
fn read_raw_reader(reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    read_raw_reader_checked(reader, limit).map_err(RawMapReadFailure::into_io_error)
}

fn read_raw_reader_checked(
    mut reader: impl Read,
    limit: usize,
) -> Result<Vec<u8>, RawMapReadFailure> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; BUFFER_BYTES];
    loop {
        let remaining = limit - output.len();
        let request = remaining.saturating_add(1).min(buffer.len());
        let read =
            read_retry(&mut reader, &mut buffer[..request]).map_err(RawMapReadFailure::Io)?;
        if read == 0 {
            return Ok(output);
        }
        append_bounded(&mut output, &buffer[..read], limit)?;
    }
}

fn read_gzip_reader(
    reader: impl Read,
    decoded_limit: usize,
    compressed_limit: usize,
) -> io::Result<Vec<u8>> {
    // bufread::GzDecoder is the same single-member state machine wrapped by
    // read::GzDecoder, without the latter's separate heap prefetch buffer.
    // CRC, ISIZE, optional gzip headers and truncation remain decoder checks.
    let input = InputBudgetReader::new(reader, compressed_limit);
    read_raw_reader(GzDecoder::new(input), decoded_limit)
}

/// Real input budget, never Read::take-style artificial EOF at a limit.
/// If the decoder needs input after its budget is exhausted, probe one byte:
/// actual EOF is returned, while any extra byte latches an error. At most the
/// configured compressed limit plus that one rejection probe is read.
struct InputBudgetReader<R> {
    reader: R,
    buffer: [u8; BUFFER_BYTES],
    position: usize,
    filled: usize,
    remaining: usize,
    over_limit: bool,
}

impl<R> InputBudgetReader<R> {
    fn new(reader: R, limit: usize) -> Self {
        Self {
            reader,
            buffer: [0; BUFFER_BYTES],
            position: 0,
            filled: 0,
            remaining: limit,
            over_limit: false,
        }
    }
}

impl<R: Read> BufRead for InputBudgetReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.over_limit {
            return Err(limit_error());
        }
        if self.position == self.filled {
            if self.remaining == 0 {
                let mut probe = [0_u8; 1];
                if read_retry(&mut self.reader, &mut probe)? != 0 {
                    self.over_limit = true;
                    return Err(limit_error());
                }
                self.position = 0;
                self.filled = 0;
            } else {
                let request = self.remaining.min(self.buffer.len());
                let read = read_retry(&mut self.reader, &mut self.buffer[..request])?;
                self.remaining -= read;
                self.position = 0;
                self.filled = read;
            }
        }
        Ok(&self.buffer[self.position..self.filled])
    }

    fn consume(&mut self, amount: usize) {
        self.position = self.position.saturating_add(amount).min(self.filled);
    }
}

impl<R: Read> Read for InputBudgetReader<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let buffered = self.fill_buf()?;
        let read = buffered.len().min(output.len());
        output[..read].copy_from_slice(&buffered[..read]);
        self.consume(read);
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::{Cursor, Write};

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    struct SegmentedReader<R> {
        reader: R,
        segment: usize,
    }

    impl<R: Read> Read for SegmentedReader<R> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            let request = self.segment.min(output.len());
            self.reader.read(&mut output[..request])
        }
    }

    struct CountedReader<R> {
        reader: R,
        bytes_read: usize,
    }

    impl<R: Read> Read for CountedReader<R> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            let read = self.reader.read(output)?;
            self.bytes_read += read;
            Ok(read)
        }
    }

    struct ErrorAfter<R> {
        reader: R,
        remaining: usize,
    }

    impl<R: Read> Read for ErrorAfter<R> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            let request = self.remaining.min(output.len());
            let read = self.reader.read(&mut output[..request])?;
            self.remaining -= read;
            Ok(read)
        }
    }

    struct InterruptOnce<R> {
        reader: R,
        interrupted: bool,
    }

    impl<R: Read> Read for InterruptOnce<R> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(io::ErrorKind::Interrupted.into());
            }
            self.reader.read(output)
        }
    }

    #[test]
    fn raw_exact_limit_requires_real_eof_and_preserves_every_byte() {
        let bytes = b"\0Map\xff\r\n\0";
        assert_eq!(
            read_raw_reader(bytes.as_slice(), bytes.len()).unwrap(),
            bytes
        );
    }

    #[test]
    fn raw_limit_plus_one_is_rejected_without_unbounded_reading() {
        let bytes = [7_u8; 128];
        let mut input = CountedReader {
            reader: bytes.as_slice(),
            bytes_read: 0,
        };
        assert_eq!(
            read_raw_reader(&mut input, 17).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(input.bytes_read, 18);
    }

    #[test]
    fn raw_zero_limit_accepts_only_actual_empty_input() {
        assert_eq!(read_raw_reader(&[][..], 0).unwrap(), Vec::<u8>::new());
        assert!(read_raw_reader(&[0][..], 0).is_err());
    }

    #[test]
    fn segmented_raw_reads_are_not_mistaken_for_eof() {
        let bytes = b"complete raw map bytes";
        assert_eq!(
            read_raw_reader(
                SegmentedReader {
                    reader: bytes.as_slice(),
                    segment: 2,
                },
                bytes.len(),
            )
            .unwrap(),
            bytes
        );
    }

    #[test]
    fn raw_read_error_never_returns_a_partial_vector() {
        let input = ErrorAfter {
            reader: &b"not a partial success"[..],
            remaining: 3,
        };
        assert_eq!(
            read_raw_reader(input, 64).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn raw_error_on_exact_limit_probe_is_not_successful_eof() {
        let input = ErrorAfter {
            reader: &b"map"[..],
            remaining: 3,
        };
        assert_eq!(
            read_raw_reader(input, 3).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn interrupted_reads_can_resume_without_changing_bytes() {
        let bytes = b"resumed bytes";
        assert_eq!(
            read_raw_reader(
                InterruptOnce {
                    reader: bytes.as_slice(),
                    interrupted: false,
                },
                bytes.len(),
            )
            .unwrap(),
            bytes
        );
    }

    #[test]
    fn output_capacity_growth_is_geometric_and_capped() {
        let mut output = Vec::new();
        let limit = BUFFER_BYTES * 2 + 13;
        append_bounded(&mut output, &[1], limit).unwrap();
        assert_eq!(output.capacity(), BUFFER_BYTES);
        append_bounded(&mut output, &[2; BUFFER_BYTES], limit).unwrap();
        assert_eq!(output.capacity(), BUFFER_BYTES * 2);
        append_bounded(&mut output, &[3; BUFFER_BYTES + 12], limit).unwrap();
        assert_eq!(output.len(), limit);
        assert_eq!(output.capacity(), limit);
        assert!(append_bounded(&mut output, &[4], limit).is_err());
        assert_eq!(output.len(), limit);
        assert_eq!(output.capacity(), limit);
    }

    #[test]
    fn tiny_output_limit_does_not_reserve_an_entire_read_buffer() {
        let mut output = Vec::new();
        append_bounded(&mut output, b"a", 3).unwrap();
        assert_eq!(output.capacity(), 3);
        append_bounded(&mut output, b"bc", 3).unwrap();
        assert_eq!(output, b"abc");
        assert_eq!(output.capacity(), 3);
    }

    #[test]
    fn invalid_reader_count_is_an_error_not_a_slice_panic() {
        struct InvalidCount;
        impl Read for InvalidCount {
            fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
                Ok(output.len() + 1)
            }
        }
        assert_eq!(
            read_raw_reader(InvalidCount, 3).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn complete_gzip_accepts_exact_decoded_and_compressed_limits() {
        let decoded = b"map bytes with exact limits\0\xff";
        let encoded = gzip(decoded);
        assert_eq!(
            read_gzip_reader(encoded.as_slice(), decoded.len(), encoded.len()).unwrap(),
            decoded
        );
    }

    #[test]
    fn gzip_can_validate_across_single_byte_compressed_reads() {
        let decoded = b"segmented gzip body";
        let encoded = gzip(decoded);
        assert_eq!(
            read_gzip_reader(
                SegmentedReader {
                    reader: encoded.as_slice(),
                    segment: 1,
                },
                decoded.len(),
                encoded.len(),
            )
            .unwrap(),
            decoded
        );
    }

    #[test]
    fn valid_empty_member_still_requires_complete_footer_with_zero_output_limit() {
        let encoded = gzip(b"");
        assert_eq!(
            read_gzip_reader(encoded.as_slice(), 0, encoded.len()).unwrap(),
            Vec::<u8>::new()
        );
        assert!(read_gzip_reader(&encoded[..encoded.len() - 1], 0, encoded.len()).is_err());
    }

    #[test]
    fn crc_error_is_rejected_even_when_decoded_bytes_exactly_fill_limit() {
        let decoded = b"must validate footer after output is full";
        let mut encoded = gzip(decoded);
        let crc_offset = encoded.len() - 8;
        encoded[crc_offset] ^= 1;
        assert!(read_gzip_reader(encoded.as_slice(), decoded.len(), encoded.len()).is_err());
    }

    #[test]
    fn invalid_isize_is_rejected_after_a_complete_body() {
        let decoded = b"body alone is not a valid member";
        let mut encoded = gzip(decoded);
        let isize_offset = encoded.len() - 4;
        encoded[isize_offset] ^= 1;
        assert!(read_gzip_reader(encoded.as_slice(), decoded.len(), encoded.len()).is_err());
    }

    #[test]
    fn truncated_header_body_and_footer_never_return_partial_output() {
        let encoded = gzip(b"complete map expected");
        for cut in [
            0,
            1,
            2,
            9,
            encoded.len() - 8,
            encoded.len() - 3,
            encoded.len() - 1,
        ] {
            assert!(
                read_gzip_reader(&encoded[..cut], 128, encoded.len()).is_err(),
                "truncated at {cut}"
            );
        }
    }

    #[test]
    fn small_compressed_bomb_cannot_cross_decoded_limit() {
        let encoded = gzip(&[0_u8; 4096]);
        assert!(encoded.len() < 128);
        assert_eq!(
            read_gzip_reader(encoded.as_slice(), 31, 128)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn compressed_limit_cannot_fake_eof_or_a_valid_completed_member() {
        let encoded = gzip(b"footer crosses the compressed budget");
        let limit = encoded.len() - 1;
        let mut input = CountedReader {
            reader: encoded.as_slice(),
            bytes_read: 0,
        };
        assert_eq!(
            read_gzip_reader(&mut input, 128, limit).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(input.bytes_read, limit + 1);
    }

    #[test]
    fn input_budget_distinguishes_exact_eof_from_extra_input_and_latches_error() {
        let mut exact = InputBudgetReader::new(&b"ab"[..], 2);
        let mut bytes = [0; 2];
        exact.read_exact(&mut bytes).unwrap();
        assert_eq!(bytes, *b"ab");
        assert!(exact.fill_buf().unwrap().is_empty());

        let mut excess = InputBudgetReader::new(&b"abc"[..], 2);
        excess.read_exact(&mut bytes).unwrap();
        assert_eq!(
            excess.fill_buf().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            excess.fill_buf().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn compressed_read_error_does_not_publish_already_decoded_bytes() {
        let encoded = gzip(b"whole map or none");
        let input = ErrorAfter {
            reader: encoded.as_slice(),
            remaining: encoded.len() - 1,
        };
        assert_eq!(
            read_gzip_reader(input, 64, encoded.len())
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn only_first_member_is_decoded_and_a_bad_later_member_is_ignored() {
        let first = gzip(b"first map");
        let mut second = gzip(b"second is not included");
        let crc_offset = second.len() - 8;
        second[crc_offset] ^= 1;
        let mut combined = first.clone();
        combined.extend_from_slice(&second);
        assert_eq!(
            read_gzip_reader(combined.as_slice(), 9, combined.len()).unwrap(),
            b"first map"
        );
        let mut reference = flate2::read::GzDecoder::new(combined.as_slice());
        let mut original_bytes = Vec::new();
        reference.read_to_end(&mut original_bytes).unwrap();
        assert_eq!(original_bytes, b"first map");
    }

    #[test]
    fn bad_first_member_does_not_skip_to_a_valid_later_member() {
        let mut first = gzip(b"bad first map");
        let crc_offset = first.len() - 8;
        first[crc_offset] ^= 1;
        first.extend_from_slice(&gzip(b"valid later map"));
        assert!(read_gzip_reader(first.as_slice(), 128, first.len()).is_err());
    }

    #[test]
    fn ignored_large_trailing_data_is_not_scanned_or_a_whole_file_size_limit() {
        let first = gzip(b"first map");
        let limit = first.len();
        let mut combined = first;
        combined.extend_from_slice(&[0xaa; BUFFER_BYTES * 3]);
        let mut input = CountedReader {
            reader: Cursor::new(combined),
            bytes_read: 0,
        };
        assert_eq!(
            read_gzip_reader(&mut input, 9, limit).unwrap(),
            b"first map"
        );
        assert_eq!(input.bytes_read, limit);
    }

    #[test]
    fn invalid_gzip_header_is_not_raw_fallback_or_partial_success() {
        assert!(read_gzip_reader(&b"not a gzip member"[..], 64, 64).is_err());
    }

    #[test]
    fn checked_raw_limit_cannot_add_pack_fallback_and_uses_only_one_probe() {
        let mut input = CountedReader {
            reader: &b"map with excess bytes"[..],
            bytes_read: 0,
        };
        let failure = read_raw_reader_checked(&mut input, 3).unwrap_err();
        assert!(matches!(&failure, RawMapReadFailure::Limit));
        assert!(!failure.can_try_pack());
        assert_eq!(input.bytes_read, 4);
        assert_eq!(failure.into_io_error().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn actual_invalid_data_read_error_keeps_pack_fallback_at_the_exact_boundary() {
        struct InvalidDataAfter<R> {
            reader: R,
            remaining: usize,
        }
        impl<R: Read> Read for InvalidDataAfter<R> {
            fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
                if self.remaining == 0 {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                let request = self.remaining.min(output.len());
                let read = self.reader.read(&mut output[..request])?;
                self.remaining -= read;
                Ok(read)
            }
        }

        let input = InvalidDataAfter {
            reader: &b"map"[..],
            remaining: 3,
        };
        let failure = read_raw_reader_checked(input, 3).unwrap_err();
        assert!(matches!(
            &failure,
            RawMapReadFailure::Io(error) if error.kind() == io::ErrorKind::InvalidData
        ));
        assert!(failure.can_try_pack());
        // It shares the old public error kind with a budget failure, but its
        // independently preserved origin above selects a different fallback.
        assert_eq!(failure.into_io_error().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn checked_raw_success_and_declared_allocation_failure_keep_their_policy() {
        assert_eq!(read_raw_reader_checked(&b"map"[..], 3).unwrap(), b"map");
        let failure = RawMapReadFailure::AllocationUnavailable;
        assert!(failure.can_try_pack());
        assert_eq!(failure.into_io_error().kind(), io::ErrorKind::OutOfMemory);
    }
}
