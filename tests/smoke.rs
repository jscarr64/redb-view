use std::path::{Path, PathBuf};
use std::process::Command;

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

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("redb-view-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_text_number_db(path: &Path) {
    let db = Database::create(path).unwrap();
    let txn = db.begin_write().unwrap();
    {
        const T: TableDefinition<'_, &str, u64> = TableDefinition::new("str_u64");
        let mut table = txn.open_table(T).unwrap();
        table.insert("alpha", &10_u64).unwrap();
        table.insert("beta", &20_u64).unwrap();
    }
    {
        const T: TableDefinition<'_, u64, &str> = TableDefinition::new("u64_str");
        let mut table = txn.open_table(T).unwrap();
        table.insert(&1_u64, "one").unwrap();
        table.insert(&2_u64, "two").unwrap();
    }
    {
        const T: TableDefinition<'_, &str, u8> = TableDefinition::new("str_u8");
        txn.open_table(T).unwrap().insert("n", &7_u8).unwrap();
    }
    {
        const T: TableDefinition<'_, &str, u16> = TableDefinition::new("str_u16");
        txn.open_table(T).unwrap().insert("n", &700_u16).unwrap();
    }
    {
        const T: TableDefinition<'_, &str, u32> = TableDefinition::new("str_u32");
        txn.open_table(T).unwrap().insert("n", &70_000_u32).unwrap();
    }
    {
        const T: TableDefinition<'_, &str, u128> = TableDefinition::new("str_u128");
        txn.open_table(T).unwrap().insert("n", &9_u128).unwrap();
    }
    {
        const T: TableDefinition<'_, &str, i8> = TableDefinition::new("str_i8");
        txn.open_table(T).unwrap().insert("n", &-3_i8).unwrap();
    }
    {
        const T: TableDefinition<'_, &str, i16> = TableDefinition::new("str_i16");
        txn.open_table(T).unwrap().insert("n", &-300_i16).unwrap();
    }
    {
        const T: TableDefinition<'_, &str, i32> = TableDefinition::new("str_i32");
        txn.open_table(T)
            .unwrap()
            .insert("n", &-30_000_i32)
            .unwrap();
    }
    {
        const T: TableDefinition<'_, &str, i64> = TableDefinition::new("str_i64");
        txn.open_table(T)
            .unwrap()
            .insert("n", &-3_000_000_i64)
            .unwrap();
    }
    {
        const T: TableDefinition<'_, &str, i128> = TableDefinition::new("str_i128");
        txn.open_table(T).unwrap().insert("n", &-9_i128).unwrap();
    }
    {
        const T: TableDefinition<'_, u8, &str> = TableDefinition::new("u8_str");
        txn.open_table(T).unwrap().insert(&7_u8, "seven").unwrap();
    }
    {
        const T: TableDefinition<'_, u16, &str> = TableDefinition::new("u16_str");
        txn.open_table(T).unwrap().insert(&700_u16, "wide").unwrap();
    }
    {
        const T: TableDefinition<'_, u32, &str> = TableDefinition::new("u32_str");
        txn.open_table(T)
            .unwrap()
            .insert(&70_000_u32, "wider")
            .unwrap();
    }
    {
        const T: TableDefinition<'_, u128, &str> = TableDefinition::new("u128_str");
        txn.open_table(T).unwrap().insert(&9_u128, "huge").unwrap();
    }
    {
        const T: TableDefinition<'_, i8, &str> = TableDefinition::new("i8_str");
        txn.open_table(T).unwrap().insert(&-3_i8, "neg").unwrap();
    }
    {
        const T: TableDefinition<'_, i16, &str> = TableDefinition::new("i16_str");
        txn.open_table(T).unwrap().insert(&-300_i16, "neg").unwrap();
    }
    {
        const T: TableDefinition<'_, i32, &str> = TableDefinition::new("i32_str");
        txn.open_table(T)
            .unwrap()
            .insert(&-30_000_i32, "neg")
            .unwrap();
    }
    {
        const T: TableDefinition<'_, i64, &str> = TableDefinition::new("i64_str");
        txn.open_table(T)
            .unwrap()
            .insert(&-3_000_000_i64, "neg")
            .unwrap();
    }
    {
        const T: TableDefinition<'_, i128, &str> = TableDefinition::new("i128_str");
        txn.open_table(T).unwrap().insert(&-9_i128, "neg").unwrap();
    }
    txn.commit().unwrap();
}

#[test]
fn page_str_u64_and_u64_str() {
    let dir = temp_dir("text-num");
    let path = dir.join("t.redb");
    write_text_number_db(&path);

    let view = DatabaseView::open(&path).unwrap();

    let rows = view.page("str_u64", 0, 10).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].index, 0);
    assert_eq!(rows[0].key.text.as_deref(), Some("alpha"));
    assert_eq!(rows[0].value.text.as_deref(), Some("10"));
    assert_eq!(rows[1].key.text.as_deref(), Some("beta"));
    assert_eq!(rows[1].value.text.as_deref(), Some("20"));

    let page2 = view.page("str_u64", 1, 1).unwrap();
    assert_eq!(page2.len(), 1);
    assert_eq!(page2[0].index, 1);
    assert_eq!(page2[0].value.text.as_deref(), Some("20"));

    let rows = view.page("u64_str", 0, 10).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].key.text.as_deref(), Some("1"));
    assert_eq!(rows[0].value.text.as_deref(), Some("one"));
    assert_eq!(rows[1].key.text.as_deref(), Some("2"));
    assert_eq!(rows[1].value.text.as_deref(), Some("two"));

    let page2 = view.page("u64_str", 1, 1).unwrap();
    assert_eq!(page2.len(), 1);
    assert_eq!(page2[0].index, 1);
    assert_eq!(page2[0].value.text.as_deref(), Some("two"));

    let expect = [
        ("str_u8", "n", "7"),
        ("str_u16", "n", "700"),
        ("str_u32", "n", "70000"),
        ("str_u128", "n", "9"),
        ("str_i8", "n", "-3"),
        ("str_i16", "n", "-300"),
        ("str_i32", "n", "-30000"),
        ("str_i64", "n", "-3000000"),
        ("str_i128", "n", "-9"),
        ("u8_str", "7", "seven"),
        ("u16_str", "700", "wide"),
        ("u32_str", "70000", "wider"),
        ("u128_str", "9", "huge"),
        ("i8_str", "-3", "neg"),
        ("i16_str", "-300", "neg"),
        ("i32_str", "-30000", "neg"),
        ("i64_str", "-3000000", "neg"),
        ("i128_str", "-9", "neg"),
    ];
    for (table, key, value) in expect {
        let rows = view.page(table, 0, 5).unwrap();
        assert_eq!(rows.len(), 1, "{table}");
        assert_eq!(rows[0].key.text.as_deref(), Some(key), "{table}");
        assert_eq!(rows[0].value.text.as_deref(), Some(value), "{table}");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cli_pages_str_u64_and_u64_str() {
    let dir = temp_dir("text-num-cli");
    let path = dir.join("t.redb");
    write_text_number_db(&path);
    let bin = env!("CARGO_BIN_EXE_redb-view");
    let path = path.display().to_string();

    let str_u64 = Command::new(bin)
        .args([&path, "page", "str_u64"])
        .output()
        .unwrap();
    assert!(str_u64.status.success());
    assert_eq!(
        String::from_utf8(str_u64.stdout).unwrap(),
        "index\tkey\tvalue\n0\talpha\t10\n1\tbeta\t20\n"
    );

    let u64_str = Command::new(bin)
        .args([&path, "page", "u64_str", "--offset", "1", "--limit", "1"])
        .output()
        .unwrap();
    assert!(u64_str.status.success());
    assert_eq!(
        String::from_utf8(u64_str.stdout).unwrap(),
        "index\tkey\tvalue\n1\t2\ttwo\n"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
