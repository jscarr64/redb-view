//! Browse redb databases: list tables and page key/value rows.
//!
//! Generic over table schemas. Key/value types are probed from a small set of
//! common layouts because redb requires typed `TableDefinition` to iterate.

use std::error::Error;
use std::fmt;
use std::path::Path;

use redb::{
    Database, MultimapTableHandle, ReadableDatabase, ReadableTable, ReadableTableMetadata,
    TableDefinition, TableHandle,
};

/// Open handle on a `.redb` file for read browsing.
pub struct DatabaseView {
    db: Database,
}

/// Kind of user table in the database.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableKind {
    /// Ordinary key → value table.
    Normal,
    /// Multimap key → many values (paging not in 0.1).
    Multimap,
}

/// Summary of one user table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableInfo {
    pub name: String,
    pub kind: TableKind,
    pub len: u64,
}

/// Display form of a key or value (UTF-8 text when possible, always hex).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayValue {
    pub text: Option<String>,
    pub hex: String,
    pub raw_len: usize,
}

/// One row from a paged table scan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KvRow {
    pub index: u64,
    pub key: DisplayValue,
    pub value: DisplayValue,
}

/// Browse / open failure.
#[derive(Debug)]
pub struct ViewError(String);

impl ViewError {
    #[must_use]
    pub fn msg(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ViewError {}

impl DatabaseView {
    /// Open an existing redb file for reads.
    ///
    /// # Errors
    ///
    /// Returns [`ViewError`] when the path cannot be opened as a redb database.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ViewError> {
        let path = path.as_ref();
        let db = Database::open(path)
            .map_err(|err| ViewError::msg(format!("open {}: {err}", path.display())))?;
        Ok(Self { db })
    }

    /// List user tables (normal and multimap) with lengths.
    ///
    /// # Errors
    ///
    /// Returns [`ViewError`] on read or metadata failures.
    pub fn tables(&self) -> Result<Vec<TableInfo>, ViewError> {
        let txn = self
            .db
            .begin_read()
            .map_err(|err| ViewError::msg(format!("begin_read: {err}")))?;
        let mut out = Vec::new();
        for handle in txn
            .list_tables()
            .map_err(|err| ViewError::msg(format!("list_tables: {err}")))?
        {
            let name = handle.name().to_owned();
            let table = txn
                .open_untyped_table(handle)
                .map_err(|err| ViewError::msg(format!("open_untyped {name}: {err}")))?;
            let len = table
                .len()
                .map_err(|err| ViewError::msg(format!("len {name}: {err}")))?;
            out.push(TableInfo {
                name,
                kind: TableKind::Normal,
                len,
            });
        }
        for handle in txn
            .list_multimap_tables()
            .map_err(|err| ViewError::msg(format!("list_multimap_tables: {err}")))?
        {
            let name = handle.name().to_owned();
            let table = txn
                .open_untyped_multimap_table(handle)
                .map_err(|err| ViewError::msg(format!("open_untyped_multimap {name}: {err}")))?;
            let len = table
                .len()
                .map_err(|err| ViewError::msg(format!("len {name}: {err}")))?;
            out.push(TableInfo {
                name,
                kind: TableKind::Multimap,
                len,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Page `limit` rows starting at `offset` (0-based) from a normal table.
    ///
    /// Key/value types are detected by probing common `TableDefinition` layouts.
    ///
    /// # Errors
    ///
    /// Returns [`ViewError`] when the table is missing, is multimap, has an
    /// unsupported key/value type pair, or a read fails.
    pub fn page(&self, table: &str, offset: u64, limit: usize) -> Result<Vec<KvRow>, ViewError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        for info in self.tables()? {
            if info.name == table && info.kind == TableKind::Multimap {
                return Err(ViewError::msg(format!(
                    "table {table}: multimap paging is not supported in 0.1"
                )));
            }
        }

        let txn = self
            .db
            .begin_read()
            .map_err(|err| ViewError::msg(format!("begin_read: {err}")))?;

        if let Ok(rows) =
            page_typed::<&[u8], &[u8], _, _>(&txn, table, offset, limit, disp_bytes, disp_bytes)
        {
            return Ok(rows);
        }
        if let Ok(rows) =
            page_typed::<u64, &[u8], _, _>(&txn, table, offset, limit, disp_u64, disp_bytes)
        {
            return Ok(rows);
        }
        if let Ok(rows) =
            page_typed::<u64, &str, _, _>(&txn, table, offset, limit, disp_u64, disp_str)
        {
            return Ok(rows);
        }
        if let Ok(rows) =
            page_typed::<&str, &[u8], _, _>(&txn, table, offset, limit, disp_str, disp_bytes)
        {
            return Ok(rows);
        }
        if let Ok(rows) =
            page_typed::<&str, &str, _, _>(&txn, table, offset, limit, disp_str, disp_str)
        {
            return Ok(rows);
        }
        if let Ok(rows) =
            page_typed::<u32, &[u8], _, _>(&txn, table, offset, limit, disp_u32, disp_bytes)
        {
            return Ok(rows);
        }
        if let Ok(rows) =
            page_typed::<u128, &[u8], _, _>(&txn, table, offset, limit, disp_u128, disp_bytes)
        {
            return Ok(rows);
        }
        if let Ok(rows) =
            page_typed::<u64, u64, _, _>(&txn, table, offset, limit, disp_u64, disp_u64)
        {
            return Ok(rows);
        }

        Err(ViewError::msg(format!(
            "table {table}: unsupported or missing key/value types (tried common layouts)"
        )))
    }
}

fn page_typed<K, V, FK, FV>(
    txn: &redb::ReadTransaction,
    table: &str,
    offset: u64,
    limit: usize,
    map_key: FK,
    map_value: FV,
) -> Result<Vec<KvRow>, ViewError>
where
    K: redb::Key + 'static,
    V: redb::Value + 'static,
    FK: Fn(K::SelfType<'_>) -> DisplayValue,
    FV: Fn(V::SelfType<'_>) -> DisplayValue,
{
    let def: TableDefinition<'_, K, V> = TableDefinition::new(table);
    let opened = txn
        .open_table(def)
        .map_err(|err| ViewError::msg(format!("open_table {table}: {err}")))?;
    let iter = opened
        .iter()
        .map_err(|err| ViewError::msg(format!("iter {table}: {err}")))?;
    let mut rows = Vec::with_capacity(limit);
    let mut index = 0_u64;
    for item in iter {
        let (key, value) = item.map_err(|err| ViewError::msg(format!("row {table}: {err}")))?;
        if index >= offset {
            rows.push(KvRow {
                index,
                key: map_key(key.value()),
                value: map_value(value.value()),
            });
            if rows.len() == limit {
                break;
            }
        }
        index = index.saturating_add(1);
    }
    Ok(rows)
}

fn disp_bytes(value: &[u8]) -> DisplayValue {
    from_bytes(value)
}

fn disp_str(value: &str) -> DisplayValue {
    let bytes = value.as_bytes();
    DisplayValue {
        text: Some(value.to_owned()),
        hex: to_hex(bytes),
        raw_len: bytes.len(),
    }
}

fn disp_u64(value: u64) -> DisplayValue {
    let bytes = value.to_le_bytes();
    DisplayValue {
        text: Some(value.to_string()),
        hex: to_hex(&bytes),
        raw_len: bytes.len(),
    }
}

fn disp_u32(value: u32) -> DisplayValue {
    let bytes = value.to_le_bytes();
    DisplayValue {
        text: Some(value.to_string()),
        hex: to_hex(&bytes),
        raw_len: bytes.len(),
    }
}

fn disp_u128(value: u128) -> DisplayValue {
    let bytes = value.to_le_bytes();
    DisplayValue {
        text: Some(value.to_string()),
        hex: to_hex(&bytes),
        raw_len: bytes.len(),
    }
}

fn from_bytes(bytes: &[u8]) -> DisplayValue {
    DisplayValue {
        text: std::str::from_utf8(bytes).ok().map(str::to_owned),
        hex: to_hex(bytes),
        raw_len: bytes.len(),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{to_hex, DatabaseView, TableKind};
    use redb::{Database, TableDefinition};

    #[test]
    fn hex_encoding() {
        assert_eq!(to_hex(&[0x00, 0xff]), "00ff");
    }

    #[test]
    fn smoke_create_list_page() {
        let dir = std::env::temp_dir().join(format!("redb-view-smoke-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tmpdir");
        let path = dir.join("t.redb");

        {
            let db = Database::create(&path).expect("create");
            let txn = db.begin_write().expect("write");
            {
                const T: TableDefinition<'_, u64, &str> = TableDefinition::new("demo");
                let mut table = txn.open_table(T).expect("open");
                table.insert(&1_u64, "alpha").expect("ins");
                table.insert(&2_u64, "beta").expect("ins");
            }
            txn.commit().expect("commit");
        }

        let view = DatabaseView::open(&path).expect("open view");
        let tables = view.tables().expect("tables");
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "demo");
        assert_eq!(tables[0].kind, TableKind::Normal);
        assert_eq!(tables[0].len, 2);

        let rows = view.page("demo", 0, 10).expect("page");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].index, 0);
        assert_eq!(rows[0].key.text.as_deref(), Some("1"));
        assert_eq!(rows[0].value.text.as_deref(), Some("alpha"));
        assert_eq!(rows[1].value.text.as_deref(), Some("beta"));

        let page2 = view.page("demo", 1, 1).expect("page offset");
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].index, 1);
        assert_eq!(page2[0].value.text.as_deref(), Some("beta"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
