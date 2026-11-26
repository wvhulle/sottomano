use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

mod binary_keyvalues_parser;

use self::binary_keyvalues_parser::{
    BinaryEntry, BinaryValue, app_entries, appinfo_header, string_table,
};
use super::steam_log_lines::SteamAppId;

const APPINFO_MAGIC_V29: u32 = 0x0756_4429;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SteamAppKind {
    Game,
    Demo,
    Beta,
    Application,
    Tool,
    Config,
    Dlc,
    Music,
    Video,
    Untyped,
    Unlisted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamLibraryRole {
    Playable,
    Tooling,
}

impl SteamAppKind {
    #[must_use]
    pub const fn library_role(self) -> SteamLibraryRole {
        match self {
            SteamAppKind::Game
            | SteamAppKind::Demo
            | SteamAppKind::Beta
            | SteamAppKind::Untyped
            | SteamAppKind::Unlisted => SteamLibraryRole::Playable,
            SteamAppKind::Application
            | SteamAppKind::Tool
            | SteamAppKind::Config
            | SteamAppKind::Dlc
            | SteamAppKind::Music
            | SteamAppKind::Video => SteamLibraryRole::Tooling,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SteamAppKinds {
    kinds_by_app: HashMap<SteamAppId, SteamAppKind>,
}

impl SteamAppKinds {
    #[must_use]
    pub fn installed_app_kind(&self, app_id: SteamAppId) -> SteamAppKind {
        self.kinds_by_app
            .get(&app_id)
            .copied()
            .unwrap_or(SteamAppKind::Unlisted)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppInfoReadFailure {
    #[error("Steam app info {path} unreadable: {read_failure}")]
    Read {
        path: PathBuf,
        read_failure: io::Error,
    },
    #[error("Steam app info has unsupported format {magic:#010x}")]
    UnsupportedFormat { magic: u32 },
    #[error("Steam app info malformed near byte {offset}")]
    Malformed { offset: usize },
    #[error("Steam app info key index {key_index} lies outside its string table")]
    KeyOutsideStringTable { key_index: u32 },
    #[error("Steam app {app_id:?} has an app type this shell does not know")]
    UnknownAppType { app_id: SteamAppId },
}

fn malformed_at(file_bytes: &[u8], remaining: &[u8]) -> AppInfoReadFailure {
    AppInfoReadFailure::Malformed {
        offset: file_bytes.len() - remaining.len(),
    }
}

struct KeyNames<'file> {
    strings: Vec<&'file [u8]>,
}

impl<'file> KeyNames<'file> {
    fn named<'tree>(
        &self,
        entries: &'tree [BinaryEntry<'file>],
        name: &str,
    ) -> Result<Option<&'tree BinaryValue<'file>>, AppInfoReadFailure> {
        entries
            .iter()
            .map(
                |entry| -> Result<Option<&'tree BinaryValue<'file>>, AppInfoReadFailure> {
                    let key = self.strings.get(entry.key_index as usize).ok_or(
                        AppInfoReadFailure::KeyOutsideStringTable {
                            key_index: entry.key_index,
                        },
                    )?;
                    Ok(key
                        .eq_ignore_ascii_case(name.as_bytes())
                        .then_some(&entry.value))
                },
            )
            .find_map(Result::transpose)
            .transpose()
    }

    fn app_type<'tree>(
        &self,
        root_entries: &'tree [BinaryEntry<'file>],
    ) -> Result<Option<&'file [u8]>, AppInfoReadFailure> {
        let Some(BinaryValue::Map(appinfo)) = self.named(root_entries, "appinfo")? else {
            return Ok(None);
        };
        let Some(BinaryValue::Map(common)) = self.named(appinfo, "common")? else {
            return Ok(None);
        };
        match self.named(common, "type")? {
            Some(BinaryValue::Text(type_text)) => Ok(Some(type_text)),
            Some(BinaryValue::Map(_) | BinaryValue::Scalar) | None => Ok(None),
        }
    }
}

fn app_kind(app_id: SteamAppId, type_text: &[u8]) -> Result<SteamAppKind, AppInfoReadFailure> {
    match type_text.to_ascii_lowercase().as_slice() {
        b"game" => Ok(SteamAppKind::Game),
        b"demo" => Ok(SteamAppKind::Demo),
        b"beta" => Ok(SteamAppKind::Beta),
        b"application" => Ok(SteamAppKind::Application),
        b"tool" => Ok(SteamAppKind::Tool),
        b"config" => Ok(SteamAppKind::Config),
        b"dlc" => Ok(SteamAppKind::Dlc),
        b"music" => Ok(SteamAppKind::Music),
        b"video" => Ok(SteamAppKind::Video),
        _ => Err(AppInfoReadFailure::UnknownAppType { app_id }),
    }
}

pub fn app_kinds_in(
    file_bytes: &[u8],
    installed_apps: &HashSet<SteamAppId>,
) -> Result<SteamAppKinds, AppInfoReadFailure> {
    let mut remaining = file_bytes;
    let header = appinfo_header(&mut remaining).map_err(|_| malformed_at(file_bytes, remaining))?;
    if header.magic != APPINFO_MAGIC_V29 {
        return Err(AppInfoReadFailure::UnsupportedFormat {
            magic: header.magic,
        });
    }
    let Some(mut table_bytes) = usize::try_from(header.string_table_offset)
        .map_or(None, |table_start| file_bytes.get(table_start..))
    else {
        return Err(AppInfoReadFailure::Malformed { offset: 8 });
    };
    let key_names = KeyNames {
        strings: string_table(&mut table_bytes)
            .map_err(|_| malformed_at(file_bytes, table_bytes))?,
    };
    let entries = app_entries(&mut remaining).map_err(|_| malformed_at(file_bytes, remaining))?;
    let kinds_by_app = entries
        .iter()
        .filter(|entry| installed_apps.contains(&entry.app_id))
        .map(|entry| {
            let kind = match key_names.app_type(&entry.root_entries)? {
                Some(type_text) => app_kind(entry.app_id, type_text)?,
                None => SteamAppKind::Untyped,
            };
            Ok((entry.app_id, kind))
        })
        .collect::<Result<HashMap<_, _>, AppInfoReadFailure>>()?;
    Ok(SteamAppKinds { kinds_by_app })
}

pub fn read_app_kinds(
    appinfo_path: &Path,
    installed_apps: &HashSet<SteamAppId>,
) -> Result<SteamAppKinds, AppInfoReadFailure> {
    let file_bytes = match fs::read(appinfo_path) {
        Err(read_failure) if read_failure.kind() == io::ErrorKind::NotFound => {
            return Ok(SteamAppKinds::default());
        }
        read => read.map_err(|read_failure| AppInfoReadFailure::Read {
            path: appinfo_path.to_owned(),
            read_failure,
        })?,
    };
    app_kinds_in(&file_bytes, installed_apps)
}

#[cfg(test)]
mod tests {
    use super::{
        binary_keyvalues_parser::{APP_ENTRY_HEADER_BYTES, MAP_END_TAG},
        *,
    };

    fn string_entry(key_index: u32, text: &str) -> Vec<u8> {
        [
            vec![0x01],
            key_index.to_le_bytes().to_vec(),
            text.as_bytes().to_vec(),
            vec![0],
        ]
        .concat()
    }

    fn map_entry(key_index: u32, inner: Vec<u8>) -> Vec<u8> {
        [
            vec![0x00],
            key_index.to_le_bytes().to_vec(),
            inner,
            vec![MAP_END_TAG],
        ]
        .concat()
    }

    fn app_record(number: u32, type_text: &str) -> Vec<u8> {
        let common = map_entry(
            1,
            [
                string_entry(2, type_text),
                [
                    vec![0x02],
                    3u32.to_le_bytes().to_vec(),
                    7i32.to_le_bytes().to_vec(),
                ]
                .concat(),
            ]
            .concat(),
        );
        let blob = [map_entry(0, common), vec![MAP_END_TAG]].concat();
        let body = [vec![0u8; APP_ENTRY_HEADER_BYTES], blob].concat();
        [
            number.to_le_bytes().to_vec(),
            (body.len() as u32).to_le_bytes().to_vec(),
            body,
        ]
        .concat()
    }

    fn appinfo_file(records: &[Vec<u8>]) -> Vec<u8> {
        let apps = [records.concat(), 0u32.to_le_bytes().to_vec()].concat();
        let table_offset = i64::try_from(16 + apps.len()).unwrap();
        let strings: Vec<u8> = ["appinfo", "common", "type", "steam_deck_compat"]
            .iter()
            .flat_map(|name| [name.as_bytes(), &[0]].concat())
            .collect();
        [
            APPINFO_MAGIC_V29.to_le_bytes().to_vec(),
            1u32.to_le_bytes().to_vec(),
            table_offset.to_le_bytes().to_vec(),
            apps,
            4u32.to_le_bytes().to_vec(),
            strings,
        ]
        .concat()
    }

    fn installed(numbers: &[u32]) -> HashSet<SteamAppId> {
        numbers
            .iter()
            .map(|number| SteamAppId { number: *number })
            .collect()
    }

    #[test]
    fn installed_apps_are_classified_by_their_common_type() {
        let file_bytes =
            appinfo_file(&[app_record(1_145_360, "Game"), app_record(1_493_710, "Tool")]);
        let kinds = app_kinds_in(&file_bytes, &installed(&[1_145_360, 1_493_710])).unwrap();
        assert_eq!(
            kinds.installed_app_kind(SteamAppId { number: 1_145_360 }),
            SteamAppKind::Game
        );
        assert_eq!(
            kinds.installed_app_kind(SteamAppId { number: 1_493_710 }),
            SteamAppKind::Tool
        );
        assert_eq!(
            kinds.installed_app_kind(SteamAppId { number: 42 }),
            SteamAppKind::Unlisted
        );
    }

    #[test]
    fn unknown_types_of_apps_that_are_not_installed_are_ignored() {
        let file_bytes = appinfo_file(&[app_record(10, "Series"), app_record(20, "game")]);
        let kinds = app_kinds_in(&file_bytes, &installed(&[20])).unwrap();
        assert_eq!(
            kinds.installed_app_kind(SteamAppId { number: 20 }),
            SteamAppKind::Game
        );
    }

    #[test]
    fn an_unknown_type_of_an_installed_app_is_an_error() {
        let file_bytes = appinfo_file(&[app_record(10, "Series")]);
        assert!(matches!(
            app_kinds_in(&file_bytes, &installed(&[10])),
            Err(AppInfoReadFailure::UnknownAppType { .. })
        ));
    }

    #[test]
    fn older_formats_are_refused() {
        let mut file_bytes = appinfo_file(&[]);
        file_bytes[0] = 0x28;
        assert!(matches!(
            app_kinds_in(&file_bytes, &installed(&[])),
            Err(AppInfoReadFailure::UnsupportedFormat { magic: 0x0756_4428 })
        ));
    }

    #[test]
    fn tools_and_configs_are_tooling_while_games_and_unlisted_apps_are_playable() {
        assert_eq!(SteamAppKind::Tool.library_role(), SteamLibraryRole::Tooling);
        assert_eq!(
            SteamAppKind::Config.library_role(),
            SteamLibraryRole::Tooling
        );
        assert_eq!(
            SteamAppKind::Game.library_role(),
            SteamLibraryRole::Playable
        );
        assert_eq!(
            SteamAppKind::Unlisted.library_role(),
            SteamLibraryRole::Playable
        );
    }
}
