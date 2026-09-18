//! CLI for browsing a `.redb` file.

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

use redb_view::{DatabaseView, DisplayValue, TableKind};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run() -> Result<(), ExitCode> {
    let mut args = env::args().skip(1);
    let path = args.next().ok_or_else(usage)?;
    let cmd = args.next().ok_or_else(usage)?;
    match cmd.as_str() {
        "tables" => {
            if args.next().is_some() {
                return Err(usage());
            }
            cmd_tables(&path)
        }
        "page" => {
            let table = args.next().ok_or_else(usage)?;
            let mut offset = 0_u64;
            let mut limit = 50_usize;
            while let Some(flag) = args.next() {
                match flag.as_str() {
                    "--offset" => {
                        offset = args
                            .next()
                            .ok_or_else(usage)?
                            .parse()
                            .map_err(|_| usage())?;
                    }
                    "--limit" => {
                        limit = args
                            .next()
                            .ok_or_else(usage)?
                            .parse()
                            .map_err(|_| usage())?;
                    }
                    _ => return Err(usage()),
                }
            }
            cmd_page(&path, &table, offset, limit)
        }
        _ => Err(usage()),
    }
}

fn usage() -> ExitCode {
    let _ = writeln!(
        io::stderr(),
        "usage: redb-view <path.redb> tables\n       redb-view <path.redb> page <table> [--offset N] [--limit N]"
    );
    ExitCode::from(1)
}

fn cmd_tables(path: &str) -> Result<(), ExitCode> {
    let view = DatabaseView::open(path).map_err(|err| fail(&err.to_string()))?;
    let tables = view.tables().map_err(|err| fail(&err.to_string()))?;
    let _ = writeln!(io::stdout(), "name\tkind\tlen");
    for t in tables {
        let kind = match t.kind {
            TableKind::Normal => "normal",
            TableKind::Multimap => "multimap",
        };
        let _ = writeln!(io::stdout(), "{}\t{kind}\t{}", t.name, t.len);
    }
    Ok(())
}

fn cmd_page(path: &str, table: &str, offset: u64, limit: usize) -> Result<(), ExitCode> {
    let view = DatabaseView::open(path).map_err(|err| fail(&err.to_string()))?;
    let rows = view
        .page(table, offset, limit)
        .map_err(|err| fail(&err.to_string()))?;
    let _ = writeln!(io::stdout(), "index\tkey\tvalue");
    for row in rows {
        let _ = writeln!(
            io::stdout(),
            "{}\t{}\t{}",
            row.index,
            show(&row.key),
            show(&row.value)
        );
    }
    Ok(())
}

fn show(v: &DisplayValue) -> String {
    match &v.text {
        Some(t) => t.clone(),
        None => format!("hex:{}", v.hex),
    }
}

fn fail(msg: &str) -> ExitCode {
    let _ = writeln!(io::stderr(), "{msg}");
    ExitCode::from(1)
}
