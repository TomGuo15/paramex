use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use rust_xlsxwriter::Workbook;

fn temp_measurement_path(extension: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "paramex-output-parity-{}-{}.{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        extension
    ))
}

fn parse_modelfit_file(
    extension: &str,
    content: &[u8],
) -> Vec<paramex_core::modelfit::OutputCurve> {
    let path = temp_measurement_path(extension);
    std::fs::write(&path, content).expect("temporary measurement writes");
    let result = paramex_core::modelfit::parse_output_file(&path);
    std::fs::remove_file(&path).expect("temporary measurement is removed");
    result.expect("Model Fit projection parses")
}

#[test]
fn product_projections_share_signed_column_precedence_but_keep_intentional_policies() {
    let csv = b"Vg,Vd,abs_Id,Id\n\
                1.0000000000001,2,20,-2\n\
                1.0000000000001,1,10,-1\n\
                1.0000000000001,1,30,-3\n\
                1.0000000000002,2,60,-6\n\
                1.0000000000002,1,40,-4\n\
                1.0000000000002,1,50,-5\n";

    let transfer = paramex_core::transfer::parse_output_bytes("device-output.csv", csv)
        .expect("Transfer projection parses");
    let model = parse_modelfit_file("csv", csv);

    assert_eq!(transfer.curves.len(), 2, "Transfer uses exact Vg groups");
    assert_eq!(transfer.curves[0].vd, vec![2.0, 1.0, 1.0]);
    assert_eq!(transfer.curves[0].id, vec![-2.0, -1.0, -3.0]);
    assert_eq!(transfer.curves[1].vd, vec![2.0, 1.0, 1.0]);
    assert_eq!(transfer.curves[1].id, vec![-6.0, -4.0, -5.0]);

    assert_eq!(model.len(), 1, "Model Fit merges near-equal Vg groups");
    assert_eq!(model[0].vds, vec![1.0, 2.0]);
    assert_eq!(model[0].id, vec![3.25, 4.0]);
}

#[test]
fn ordered_grid_candidates_let_each_product_apply_its_usable_curve_policy() {
    let mut workbook = Workbook::new();
    {
        let sheet = workbook.add_worksheet().set_name("Short").unwrap();
        sheet.write_string(0, 0, "Vg").unwrap();
        sheet.write_string(0, 1, "Vd").unwrap();
        sheet.write_string(0, 2, "Id").unwrap();
        for (row, vd) in [0.0, 1.0].into_iter().enumerate() {
            sheet.write_number((row + 1) as u32, 0, 1.0).unwrap();
            sheet.write_number((row + 1) as u32, 1, vd).unwrap();
            sheet
                .write_number((row + 1) as u32, 2, vd * 1.0e-6)
                .unwrap();
        }
    }
    {
        let sheet = workbook.add_worksheet().set_name("Complete").unwrap();
        sheet.write_string(0, 0, "Vg").unwrap();
        sheet.write_string(0, 1, "Vd").unwrap();
        sheet.write_string(0, 2, "Id").unwrap();
        for (row, vd) in [0.0, 1.0, 2.0].into_iter().enumerate() {
            sheet.write_number((row + 1) as u32, 0, 2.0).unwrap();
            sheet.write_number((row + 1) as u32, 1, vd).unwrap();
            sheet
                .write_number((row + 1) as u32, 2, vd * 2.0e-6)
                .unwrap();
        }
    }
    let bytes = workbook.save_to_buffer().unwrap();

    let transfer = paramex_core::transfer::parse_output_bytes("multi.xlsx", &bytes)
        .expect("Transfer finds the first three-point candidate");
    let model = parse_modelfit_file("xlsx", &bytes);

    assert_eq!(transfer.curves.len(), 1);
    assert_eq!(transfer.curves[0].vg, 2.0);
    assert_eq!(model.len(), 1);
    assert_eq!(model[0].vg, 1.0);
}
