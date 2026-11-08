//! Prints a 2DA table from the install: `dump2da <table> [row,row...]`.
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_tables::TwoDa;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let t = TwoDa::parse(&inst.read(&a[1], ResType::TWODA).unwrap()).unwrap();
    let rows: Option<Vec<usize>> = a
        .get(2)
        .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect());
    let cols: Vec<String> = t.columns.clone();
    println!("{}", cols.join("\t"));
    for r in 0..t.row_count() {
        if rows.as_ref().is_some_and(|v| !v.contains(&r)) {
            continue;
        }
        let cells: Vec<String> = cols
            .iter()
            .map(|c| t.get(r, c).unwrap_or("-").to_string())
            .collect();
        println!("{r}\t{}", cells.join("\t"));
    }
}
