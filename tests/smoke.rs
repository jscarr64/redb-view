use redb::{Database, TableDefinition};
use redb_view::DatabaseView;

#[test]
fn external_smoke() {
    let dir = std::env::temp_dir().join(format!("redb-view-ext-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.redb");
    {
        let db = Database::create(&path).unwrap();
        let txn = db.begin_write().unwrap();
        {
            const T: TableDefinition<'_, &str, &[u8]> = TableDefinition::new("blob");
            let mut table = txn.open_table(T).unwrap();
            table.insert("a", b"hello".as_slice()).unwrap();
        }
        txn.commit().unwrap();
    }
    let view = DatabaseView::open(&path).unwrap();
    let rows = view.page("blob", 0, 5).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key.text.as_deref(), Some("a"));
    assert_eq!(rows[0].value.text.as_deref(), Some("hello"));
    let _ = std::fs::remove_dir_all(&dir);
}
