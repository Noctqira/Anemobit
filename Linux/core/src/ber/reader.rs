//! Reads BER TLVs sequentially.

use std::ops::Range;

use super::{BerError, tag};

/// Cursor that reads BER TLVs sequentially.
///
/// Child readers share the same bytes as their parent, and positions are always relative to the whole data
/// (SNMPv3 HMAC verification needs to know where content sits in the original packet).
#[derive(Debug, Clone)]
pub(crate) struct BerReader<'a> {
    bytes: &'a [u8],
    offset: usize,
    end: usize,
}

impl<'a> BerReader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        BerReader {
            bytes,
            offset: 0,
            end: bytes.len(),
        }
    }

    pub(crate) fn is_at_end(&self) -> bool {
        self.offset >= self.end
    }

    /// Peeks at the next tag without moving the cursor.
    pub(crate) fn peek_tag(&self) -> Result<u8, BerError> {
        if self.is_at_end() {
            return Err(BerError::Truncated);
        }
        Ok(self.bytes[self.offset])
    }

    /// Reads one TLV and returns the tag and contents.
    pub(crate) fn read_tlv(&mut self) -> Result<(u8, &'a [u8]), BerError> {
        let (tag, range) = self.read_tlv_range()?;
        Ok((tag, &self.bytes[range]))
    }

    /// Reads one TLV and returns the tag and the content range in the original bytes.
    pub(crate) fn read_tlv_range(&mut self) -> Result<(u8, Range<usize>), BerError> {
        let tag = self.next_byte()?;
        let first = self.next_byte()?;
        let length = if first & 0x80 == 0 {
            usize::from(first)
        } else {
            let count = first & 0x7F;
            if count == 0 {
                return Err(BerError::IndefiniteLength);
            }
            if count > 4 {
                return Err(BerError::LengthTooLarge);
            }
            let mut length = 0;
            for _ in 0..count {
                length = (length << 8) | usize::from(self.next_byte()?);
            }
            length
        };
        if self.end - self.offset < length {
            return Err(BerError::Truncated);
        }
        let range = self.offset..self.offset + length;
        self.offset += length;
        Ok((tag, range))
    }

    fn next_byte(&mut self) -> Result<u8, BerError> {
        let byte = self.peek_tag()?;
        self.offset += 1;
        Ok(byte)
    }

    /// Reads a constructed value with the given tag and returns a child reader that sees only its contents.
    pub(crate) fn read_constructed(&mut self, expected: u8) -> Result<BerReader<'a>, BerError> {
        let (tag, range) = self.read_tlv_range()?;
        if tag != expected {
            return Err(BerError::UnexpectedTag {
                expected,
                actual: tag,
            });
        }
        Ok(self.sub_reader(range))
    }

    pub(crate) fn read_sequence(&mut self) -> Result<BerReader<'a>, BerError> {
        self.read_constructed(tag::SEQUENCE)
    }

    /// Creates a child reader over a range of the original bytes (e.g. BER inside an OCTET STRING).
    pub(crate) fn sub_reader(&self, range: Range<usize>) -> BerReader<'a> {
        BerReader {
            bytes: self.bytes,
            offset: range.start,
            end: range.end,
        }
    }

    /// A range of the original bytes.
    pub(crate) fn bytes(&self, range: Range<usize>) -> &'a [u8] {
        &self.bytes[range]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_single(bytes: &[u8]) -> Result<(u8, Vec<u8>), BerError> {
        let mut reader = BerReader::new(bytes);
        let (tag, content) = reader.read_tlv()?;
        assert!(reader.is_at_end());
        Ok((tag, content.to_vec()))
    }

    #[test]
    fn reads_non_minimal_long_form_length() {
        // Some implementations use the long form for short lengths; the decoder must accept it.
        assert_eq!(
            read_single(&[0x04, 0x81, 0x02, 0x61, 0x62]),
            Ok((0x04, vec![0x61, 0x62]))
        );
    }

    #[test]
    fn rejects_indefinite_length() {
        assert_eq!(
            read_single(&[0x04, 0x80, 0x00, 0x00]),
            Err(BerError::IndefiniteLength)
        );
    }

    #[test]
    fn rejects_length_field_longer_than_four_bytes() {
        assert_eq!(
            read_single(&[0x04, 0x85, 0, 0, 0, 0, 1, 0x61]),
            Err(BerError::LengthTooLarge)
        );
    }

    #[test]
    fn rejects_length_beyond_buffer() {
        assert_eq!(read_single(&[0x04, 0x05, 0x61]), Err(BerError::Truncated));
        assert_eq!(read_single(&[0x04, 0x82, 0x01]), Err(BerError::Truncated));
        assert_eq!(read_single(&[0x04]), Err(BerError::Truncated));
        assert_eq!(read_single(&[]), Err(BerError::Truncated));
    }

    #[test]
    fn sequence_gives_sub_reader_over_content() {
        let bytes = [0x30, 0x05, 0x02, 0x01, 0x05, 0x05, 0x00];
        let mut reader = BerReader::new(&bytes);
        let mut sequence = reader.read_sequence().unwrap();
        assert_eq!(sequence.peek_tag(), Ok(0x02));
        assert_eq!(sequence.read_tlv(), Ok((0x02, &[0x05][..])));
        assert_eq!(sequence.read_tlv(), Ok((0x05, &[][..])));
        assert!(sequence.is_at_end());
        assert!(reader.is_at_end());
        // A child reader at its end can't read the outer data.
        assert_eq!(sequence.peek_tag(), Err(BerError::Truncated));
    }

    #[test]
    fn read_sequence_rejects_other_tag() {
        let mut reader = BerReader::new(&[0x04, 0x00]);
        assert_eq!(
            reader.read_sequence().map(|_| ()),
            Err(BerError::UnexpectedTag {
                expected: 0x30,
                actual: 0x04
            })
        );
    }

    #[test]
    fn ranges_are_relative_to_whole_buffer() {
        // The OCTET STRING contains AA 05: length 5 exceeds the child reader's range.
        let bytes = [0x30, 0x04, 0x04, 0x02, 0xAA, 0x05];
        let mut reader = BerReader::new(&bytes);
        let mut sequence = reader.read_sequence().unwrap();
        let (tag, range) = sequence.read_tlv_range().unwrap();
        assert_eq!((tag, range.clone()), (0x04, 4..6));
        assert_eq!(sequence.bytes(range.clone()), [0xAA, 0x05]);
        let mut inner = reader.sub_reader(range);
        assert_eq!(inner.peek_tag(), Ok(0xAA));
        assert_eq!(inner.read_tlv(), Err(BerError::Truncated));
    }
}
