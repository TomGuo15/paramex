use paramex_core::modelfit::{extract_accumulation_capacitance_file, SUPPORTED_EXTENSIONS};

#[test]
fn cv_file_ingest_returns_the_accumulation_capacitance() {
    let path = std::env::temp_dir().join(format!("paramex-cv-{}.CSV", std::process::id()));
    let csv = "VBias,C\n\
               -4,1e-12\n\
               -3,2e-12\n\
               -2,4e-12\n\
               -1,7e-12\n\
               0,1e-11\n\
               1,1e-11\n\
               2,1e-11\n\
               3,1e-11\n\
               4,1e-11\n\
               5,1e-11\n";
    std::fs::write(&path, csv).expect("temporary C-V fixture writes");

    let result = extract_accumulation_capacitance_file(&path);
    std::fs::write(&path, "VBias,C\n0,1e-12\n1,2e-12\n").expect("temporary C-V fixture rewrites");
    let insufficient = extract_accumulation_capacitance_file(&path);
    std::fs::remove_file(&path).expect("temporary C-V fixture removes");

    assert_eq!(result, Ok(1.0e-11));
    assert_eq!(
        insufficient,
        Err("no usable accumulation region in the C-V sweep".to_string())
    );
    assert!(SUPPORTED_EXTENSIONS.contains(&".csv"));
}

#[test]
fn file_ingest_rejects_unsupported_containers_before_reading() {
    let path = std::env::temp_dir().join("paramex-cv-unsupported.json");
    assert_eq!(
        extract_accumulation_capacitance_file(&path),
        Err("Unsupported file extension: .json".to_string())
    );
}
