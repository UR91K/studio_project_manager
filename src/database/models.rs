use chrono::{DateTime, Local, Utc};
use rusqlite::types::ToSql;

pub(crate) struct SqlDateTime(DateTime<Local>);

impl From<DateTime<Local>> for SqlDateTime {
    fn from(dt: DateTime<Local>) -> Self {
        SqlDateTime(dt)
    }
}

impl SqlDateTime {
    pub fn from_utc(dt: DateTime<Utc>) -> Self {
        SqlDateTime(dt.with_timezone(&Local))
    }
}

impl ToSql for SqlDateTime {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        Ok(rusqlite::types::ToSqlOutput::from(self.0.timestamp()))
    }
}
