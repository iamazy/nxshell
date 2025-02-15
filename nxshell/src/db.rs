use crate::errors::NxError;
use chrono::Local;
use indexmap::IndexMap;
use rusqlite::{Connection, Result};

#[derive(Clone, Default)]
pub struct Session {
    pub id: u64,
    pub group: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub auth_type: u16,
    pub username: String,
    pub secret_data: Vec<u8>,
    pub secret_key: Vec<u8>,
    pub create_time: u64,
}

pub struct DbConn {
    db: Connection,
}

impl DbConn {
    pub fn open() -> Result<Self> {
        let db = Connection::open("db.sqlite")?;
        db.execute(
            "CREATE TABLE IF NOT EXISTS session
                (
                    id             INTEGER PRIMARY KEY AUTOINCREMENT,
                    group_name     TEXT NOT NULL,
                    name           TEXT NOT NULL,
                    host           TEXT NOT NULL,
                    port           INTEGER CHECK (port BETWEEN 1 AND 65535),
                    auth_type      INTEGER CHECK (auth_type BETWEEN 0 AND 9),
                    username       TEXT NOT NULL,
                    secret_data    BLOB NOT NULL,
                    secret_key     BLOB NOT NULL,
                    create_time    DATETIME DEFAULT CURRENT_TIMESTAMP,

                    UNIQUE (group_name, name)
                );",
            (),
        )?;
        Ok(Self { db })
    }

    pub fn find_all_sessions(&self) -> Result<IndexMap<String, Vec<Session>>> {
        let mut stmt = self
            .db
            .prepare("SELECT id, group_name, name, host, port, username FROM session")?;
        let mut rows = stmt.query(())?;
        let mut sessions = vec![];
        while let Some(row) = rows.next()? {
            sessions.push(Session {
                id: row.get(0)?,
                group: row.get(1)?,
                name: row.get(2)?,
                host: row.get(3)?,
                port: row.get(4)?,
                username: row.get(5)?,
                ..Default::default()
            });
        }
        let mut session_groups: IndexMap<String, Vec<Session>> =
            IndexMap::with_capacity(sessions.len());
        for session in sessions {
            session_groups
                .entry(session.group.clone())
                .or_default()
                .push(session);
        }
        Ok(session_groups)
    }

    pub fn find_sessions(&self, key: &str) -> Result<IndexMap<String, Vec<Session>>> {
        if key.is_empty() {
            return self.find_all_sessions();
        }
        let mut stmt = self
            .db
            .prepare("SELECT id, group_name, name, host, port, username FROM session where group_name like ?1 or name like ?1")?;
        let mut rows = stmt.query((format!("%{key}%"),))?;
        let mut sessions = vec![];
        while let Some(row) = rows.next()? {
            sessions.push(Session {
                id: row.get(0)?,
                group: row.get(1)?,
                name: row.get(2)?,
                host: row.get(3)?,
                port: row.get(4)?,
                username: row.get(5)?,
                ..Default::default()
            });
        }
        let mut session_groups: IndexMap<String, Vec<Session>> =
            IndexMap::with_capacity(sessions.len());
        for session in sessions {
            session_groups
                .entry(session.group.clone())
                .or_default()
                .push(session);
        }
        Ok(session_groups)
    }

    pub fn insert_session(&self, session: Session) -> Result<(), NxError> {
        let time = Local::now().timestamp_millis() as u64;
        self.db.execute(
            "INSERT INTO session(group_name, name, host, port, auth_type, \
                                     username, secret_data, secret_key, create_time) \
                                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (
                &session.group,
                &session.name,
                &session.host,
                session.port,
                &session.auth_type,
                &session.username,
                &session.secret_data,
                &session.secret_key,
                time,
            ),
        )?;
        Ok(())
    }

    pub fn find_session(&self, group_name: &str, name: &str) -> Result<Option<Session>> {
        let mut stmt = self.db.prepare(
            "SELECT id, group_name, name, host, port, auth_type, \
                        username, secret_data, secret_key, create_time FROM session \
                        WHERE group_name = ?1 AND name = ?2",
        )?;
        let mut rows = stmt.query((group_name, name))?;
        if let Some(row) = rows.next()? {
            return Ok(Some(Session {
                id: row.get(0)?,
                group: row.get(1)?,
                name: row.get(2)?,
                host: row.get(3)?,
                port: row.get(4)?,
                auth_type: row.get(5)?,
                username: row.get(6)?,
                secret_data: row.get(7)?,
                secret_key: row.get(8)?,
                create_time: row.get(9)?,
            }));
        }
        Ok(None)
    }

    pub fn delete_session(&self, group_name: &str, name: &str) -> Result<()> {
        self.db.execute(
            "DELETE FROM session WHERE group_name = ?1 AND name = ?2",
            (group_name, name),
        )?;
        Ok(())
    }
}
