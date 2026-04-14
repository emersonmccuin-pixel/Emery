// Phase 0 spike: verify sqlite-vec loads on Windows and can roundtrip a vector.
use rusqlite::{ffi::sqlite3_auto_extension, Connection};
use sqlite_vec::sqlite3_vec_init;

#[test]
fn sqlite_vec_loads_and_roundtrips_vector() {
    unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
    }

    let conn = Connection::open_in_memory().expect("open in-memory sqlite");

    let version: String = conn
        .query_row("SELECT vec_version()", [], |row| row.get(0))
        .expect("vec_version must work");
    println!("sqlite-vec version: {}", version);

    conn.execute(
        "CREATE VIRTUAL TABLE vtest USING vec0(embedding float[4])",
        [],
    )
    .expect("create vec0 virtual table");

    let vec: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4];
    let bytes: Vec<u8> = vec.iter().flat_map(|f| f.to_le_bytes()).collect();
    conn.execute(
        "INSERT INTO vtest(rowid, embedding) VALUES (1, ?)",
        rusqlite::params![bytes],
    )
    .expect("insert vector");

    let query: Vec<f32> = vec![0.1, 0.2, 0.3, 0.41];
    let query_bytes: Vec<u8> = query.iter().flat_map(|f| f.to_le_bytes()).collect();
    let mut stmt = conn
        .prepare("SELECT rowid, distance FROM vtest WHERE embedding MATCH ? ORDER BY distance LIMIT 1")
        .expect("prepare knn query");
    let (rowid, distance): (i64, f64) = stmt
        .query_row(rusqlite::params![query_bytes], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .expect("knn query must return");

    assert_eq!(rowid, 1);
    assert!(distance < 0.1);
    println!("roundtrip ok: rowid={} distance={}", rowid, distance);
}
