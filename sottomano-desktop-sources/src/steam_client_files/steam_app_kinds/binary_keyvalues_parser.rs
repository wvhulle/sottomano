use winnow::{
    ModalResult, Parser,
    binary::{le_i64, le_u32, u8 as tag_byte},
    combinator::{fail, repeat, repeat_till, terminated},
    token::{literal, take, take_till},
};

use super::super::steam_log_lines::SteamAppId;

pub(super) const APP_ENTRY_HEADER_BYTES: usize = 60;
pub(super) const MAP_END_TAG: u8 = 0x08;

pub(super) enum BinaryValue<'file> {
    Map(Vec<BinaryEntry<'file>>),
    Text(&'file [u8]),
    Scalar,
}

pub(super) struct BinaryEntry<'file> {
    pub(super) key_index: u32,
    pub(super) value: BinaryValue<'file>,
}

pub(super) struct AppInfoHeader {
    pub(super) magic: u32,
    pub(super) string_table_offset: i64,
}

pub(super) struct AppInfoEntry<'file> {
    pub(super) app_id: SteamAppId,
    pub(super) root_entries: Vec<BinaryEntry<'file>>,
}

fn null_terminated<'file>(input: &mut &'file [u8]) -> ModalResult<&'file [u8]> {
    terminated(take_till(0.., 0u8), literal([0u8])).parse_next(input)
}

pub(super) fn binary_map<'file>(input: &mut &'file [u8]) -> ModalResult<Vec<BinaryEntry<'file>>> {
    repeat_till(0.., binary_entry, literal([MAP_END_TAG]))
        .map(|(entries, _): (Vec<BinaryEntry<'file>>, &[u8])| entries)
        .parse_next(input)
}

fn binary_entry<'file>(input: &mut &'file [u8]) -> ModalResult<BinaryEntry<'file>> {
    let value_tag = tag_byte.parse_next(input)?;
    let key_index = le_u32.parse_next(input)?;
    let value = match value_tag {
        0x00 => BinaryValue::Map(binary_map(input)?),
        0x01 => BinaryValue::Text(null_terminated(input)?),
        0x02 | 0x03 | 0x04 | 0x06 => {
            take(4usize).void().parse_next(input)?;
            BinaryValue::Scalar
        }
        0x07 | 0x0a => {
            take(8usize).void().parse_next(input)?;
            BinaryValue::Scalar
        }
        _ => return fail.parse_next(input),
    };
    Ok(BinaryEntry { key_index, value })
}

pub(super) fn appinfo_header(input: &mut &[u8]) -> ModalResult<AppInfoHeader> {
    let magic = le_u32.parse_next(input)?;
    let _universe = le_u32.parse_next(input)?;
    let string_table_offset = le_i64.parse_next(input)?;
    Ok(AppInfoHeader {
        magic,
        string_table_offset,
    })
}

fn app_entry<'file>(input: &mut &'file [u8]) -> ModalResult<AppInfoEntry<'file>> {
    let number = le_u32.verify(|number| *number != 0).parse_next(input)?;
    let entry_size = le_u32.parse_next(input)?;
    let mut entry_bytes: &'file [u8] = take(entry_size as usize).parse_next(input)?;
    take(APP_ENTRY_HEADER_BYTES)
        .void()
        .parse_next(&mut entry_bytes)?;
    let root_entries = binary_map(&mut entry_bytes)?;
    Ok(AppInfoEntry {
        app_id: SteamAppId { number },
        root_entries,
    })
}

pub(super) fn app_entries<'file>(input: &mut &'file [u8]) -> ModalResult<Vec<AppInfoEntry<'file>>> {
    repeat_till(0.., app_entry, le_u32.verify(|number| *number == 0))
        .map(|(entries, _): (Vec<AppInfoEntry<'file>>, u32)| entries)
        .parse_next(input)
}

pub(super) fn string_table<'file>(input: &mut &'file [u8]) -> ModalResult<Vec<&'file [u8]>> {
    let string_count = le_u32.parse_next(input)?;
    repeat(string_count as usize, null_terminated).parse_next(input)
}
