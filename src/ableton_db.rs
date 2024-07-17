use std::path::PathBuf;
use rusqlite::{Connection, Result as SqliteResult, params, types::Type};
use crate::errors::DatabaseError;
use crate::custom_types::PluginFormat;

#[derive(Debug)]
pub struct DbPlugin {
    pub plugin_id: i32,
    pub module_id: Option<i32>,
    pub dev_identifier: String,
    pub name: String,
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub sdk_version: Option<String>,
    pub flags: Option<i32>,
    pub scanstate: Option<i32>,
    pub enabled: Option<i32>,
}

pub struct AbletonDatabase {
    conn: Connection,
}

impl AbletonDatabase {
    pub fn new(db_path: PathBuf) -> Result<Self, DatabaseError> {
        let conn = Connection::open(db_path)
            .map_err(|e| DatabaseError::ConnectionError(e.to_string()))?;
        Ok(Self { conn })
    }
    
    pub fn get_installed_plugins(&self) -> Result<Vec<(String, PluginFormat)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT name, dev_identifier FROM plugins WHERE scanstate = 1 AND enabled = 1"
        )?;
        let plugin_iter = stmt.query_map(params![], |row| {
            let name: String = row.get(0)?;
            let dev_identifier: String = row.get(1)?;
            let format = parse_plugin_format(&dev_identifier)
                .ok_or_else(|| rusqlite::Error::InvalidColumnType(
                    1, "dev_identifier".to_string(), Type::Text
                ))?;
            Ok((name, format))
        })?;

        plugin_iter
            .collect::<SqliteResult<Vec<_>>>()
            .map_err(DatabaseError::from)
    }

    pub fn get_plugin_by_dev_identifier(
        &self, 
        dev_identifier: &str
    ) -> Result<Option<DbPlugin>, DatabaseError> 
    {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM plugins WHERE dev_identifier = ?"
        )?;
        let result: SqliteResult<DbPlugin> = stmt.query_row(params![dev_identifier], |row| {
            Ok(DbPlugin {
                plugin_id: row.get(0)?,
                module_id: row.get(1)?,
                dev_identifier: row.get(2)?,
                name: row.get(3)?,
                vendor: row.get(4)?,
                version: row.get(5)?,
                sdk_version: row.get(6)?,
                flags: row.get(7)?,
                scanstate: row.get(8)?,
                enabled: row.get(9)?,
            })
        });

        match result {
            Ok(plugin) => Ok(Some(plugin)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::QueryError(e.to_string())),
        }
    }
}

fn parse_plugin_format(dev_identifier: &str) -> Option<PluginFormat> {
    if dev_identifier.starts_with("device:vst3:instr:") {
        Some(PluginFormat::VST3Instrument)
    } else if dev_identifier.starts_with("device:vst3:audiofx:") {
        Some(PluginFormat::VST3AudioFx)
    } else if dev_identifier.starts_with("device:vst:instr:") {
        Some(PluginFormat::VST2Instrument)
    } else if dev_identifier.starts_with("device:vst:audiofx:") {
        Some(PluginFormat::VST2AudioFx)
    } else {
        None
    }
}