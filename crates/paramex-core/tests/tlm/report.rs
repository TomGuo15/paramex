use crate::common::tlm_fixture_dir;
use paramex_core::tlm::{analyze_dataset, load_dataset, result_csv, status_csv};

#[test]
fn result_csv_header_and_nan_empty() {
    let ds = load_dataset(&tlm_fixture_dir(), None).unwrap();
    let res = analyze_dataset(&ds, None);
    let csv = String::from_utf8(result_csv(&res)).unwrap();
    let header = csv.lines().next().unwrap();
    assert_eq!(
        header,
        "group,selected_vg,Rcontact_script_ohm,Rc_per_contact_ohm,slope_ohm_per_um,r_squared,Rcontact_median_ohm,Rc_per_contact_median_ohm,slope_median_ohm_per_um,r_squared_median,valid_lengths,warnings"
    );
    // single-length group -> NaN fit -> empty cells for the fit columns
    let row = csv.lines().nth(1).unwrap();
    assert!(row.starts_with("grp,")); // group name present
    assert!(row.contains(",,")); // NaN rendered as empty
}

#[test]
fn status_csv_header() {
    let ds = load_dataset(&tlm_fixture_dir(), None).unwrap();
    let res = analyze_dataset(&ds, None);
    let csv = String::from_utf8(status_csv(&res)).unwrap();
    assert_eq!(
        csv.lines().next().unwrap(),
        "file,group,length_um,status,message,vd_source"
    );
}
