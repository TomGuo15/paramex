use super::{
    accumulation_capacitance, parse_cv_bytes, parse_output_bytes, parse_second_transfer_bytes,
};

#[test]
fn accumulation_capacitance_finds_the_plateau_through_noise_and_depletion() {
    let mut capacitance = vec![1e-12, 1.5e-12, 3e-12, 6e-12];
    capacitance.extend([9.8e-12, 1.02e-11, 1.0e-11, 9.9e-12, 1.01e-11, 1.0e-11]);
    capacitance.push(5e-11);
    let accumulation = accumulation_capacitance(&capacitance).expect("enough points");
    assert!(
        (accumulation - 1.0e-11).abs() < 1.5e-12,
        "accumulation cap {accumulation:e} should select the plateau"
    );
}

#[test]
fn accumulation_capacitance_rejects_too_few_or_nonpositive_points() {
    assert!(accumulation_capacitance(&[1e-11, 1e-11, 1e-11]).is_none());
    assert!(accumulation_capacitance(&[0.0, -1.0, f64::NAN, 0.0, -2.0, 0.0]).is_none());
}

// Output-curve files are long-format: Vg, Vd, Id columns, rows grouped by Vg into
// sub-sweeps (sorted by Vd). Column names are matched by alias, case-insensitively.

#[test]
fn parses_long_format_output_into_grouped_curves() {
    let csv = "Vg,Vd,Id\n\
               6,0,0\n\
               6,2,1.5e-6\n\
               6,1,1.0e-6\n\
               9,0,0\n\
               9,1,2.0e-6\n\
               9,2,3.0e-6\n";
    let curves = parse_output_bytes(csv.as_bytes(), ".csv").expect("parses");
    assert_eq!(curves.len(), 2);

    let c6 = curves
        .iter()
        .find(|c| (c.vg - 6.0).abs() < 1e-9)
        .expect("vg=6 sub-sweep");
    // Rows out of Vd order are sorted ascending.
    assert_eq!(c6.vds, vec![0.0, 1.0, 2.0]);
    assert_eq!(c6.id, vec![0.0, 1.0e-6, 1.5e-6]);

    let c9 = curves
        .iter()
        .find(|c| (c.vg - 9.0).abs() < 1e-9)
        .expect("vg=9 sub-sweep");
    assert_eq!(c9.id, vec![0.0, 2.0e-6, 3.0e-6]);
}

#[test]
fn collapses_bidirectional_output_sweeps_to_unique_vd() {
    let csv = "Vg,Vd,Id\n\
               6,0,0\n\
               6,1,1e-6\n\
               6,2,2e-6\n\
               6,1,3e-6\n\
               6,0,2e-6\n";
    let curves = parse_output_bytes(csv.as_bytes(), ".csv").expect("parses");

    assert_eq!(curves[0].vds, vec![0.0, 1.0, 2.0]);
    assert_eq!(curves[0].id, vec![1.0e-6, 2.0e-6, 2.0e-6]);
    assert!(curves[0].vds.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn matches_aliased_column_names_case_insensitively() {
    let tsv = "VGS\tVDS\tabs_Id\n5\t1\t1e-6\n5\t2\t2e-6\n";
    let curves = parse_output_bytes(tsv.as_bytes(), ".tsv").expect("parses aliases");
    assert_eq!(curves.len(), 1);
    assert_eq!(curves[0].vg, 5.0);
    assert_eq!(curves[0].id.len(), 2);
}

#[test]
fn finds_the_header_below_an_instrument_preamble() {
    // Real instrument export (the case that "always errored"): metadata rows
    // ("Setup Name:", "Date:", a blank) sit ABOVE the real column header, and the
    // headers carry units. The parser scans for the header row and reads the data
    // below it — the bug was assuming the header is row 0.
    let csv = "Setup Name:,Id-Vd-low-5\n\
               Date:,2024-12-11\n\
               ,\n\
               Vd (V),Vg (V),Ig (A),Id (A),abs_id\n\
               0,-5,-7e-8,6.7e-6,6.7e-6\n\
               -0.2,-5,-6e-8,-1.6e-6,1.6e-6\n\
               0,-4,-5e-8,3.0e-6,3.0e-6\n";
    let curves = parse_output_bytes(csv.as_bytes(), ".csv").expect("parses past the preamble");
    assert_eq!(curves.len(), 2, "two Vg sub-sweeps (-5, -4)");
    let c5 = curves
        .iter()
        .find(|c| (c.vg + 5.0).abs() < 1e-9)
        .expect("vg=-5 sub-sweep");
    assert_eq!(c5.vds, vec![-0.2, 0.0]); // sorted ascending by Vd
    assert_eq!(c5.id, vec![1.6e-6, 6.7e-6]); // |Id| from the Id column (not Ig/abs_id)
}

#[test]
fn finds_a_b1500a_output_header_buried_far_below_the_preamble() {
    // A real B1500A `Id-Vd` export buries the `DataName, vd, vg, id, …` header
    // ~260 rows deep behind its setup/analysis metadata, and prefixes every data
    // row with a `DataValue` label cell (so vd/vg/id sit at columns 1/2/3). The
    // old 25-row scan limit never reached the header → "always errored".
    let mut csv = String::new();
    for i in 0..200 {
        csv.push_str(&format!("TestParameter, Setting.{i}, x\n"));
    }
    csv.push_str("DataName, vd, vg, id, is, ig\n");
    csv.push_str("DataValue, 0, -5, 6.7e-6, 6.7e-6, 1e-13\n");
    csv.push_str("DataValue, -0.2, -5, 1.6e-6, 1.6e-6, 1e-13\n");
    csv.push_str("DataValue, 0, -4, 3.0e-6, 3.0e-6, 1e-13\n");

    let curves = parse_output_bytes(csv.as_bytes(), ".csv")
        .expect("a header 200+ rows deep must still be found");
    assert_eq!(curves.len(), 2, "two Vg sub-sweeps (-5, -4)");
    let c5 = curves
        .iter()
        .find(|c| (c.vg + 5.0).abs() < 1e-9)
        .expect("vg=-5 sub-sweep");
    assert_eq!(c5.vds, vec![-0.2, 0.0]);
    assert_eq!(c5.id, vec![1.6e-6, 6.7e-6]);
}

#[test]
fn errors_when_a_required_column_is_missing() {
    let csv = "Vg,Vd,foo\n1,2,3\n";
    assert!(parse_output_bytes(csv.as_bytes(), ".csv").is_err());
}

#[test]
fn parses_a_cv_sweep_past_its_instrument_preamble() {
    // The real "C-V Sweep" export: a metadata preamble above a `VBias, Freq, C, G`
    // header (the G/conductance column is ignored).
    let csv = "C-V Sweep,C-V Sweep\n\
               RecordTime,11/30/2024 18:16:08\n\
               Count,1\n\
               VBias,Freq,C,G\n\
               -3,1000,4.6e-11,5.4e-7\n\
               -2,1000,7.5e-11,6.2e-7\n\
               -1,1000,1.0e-10,6.6e-7\n";
    let capacitance = parse_cv_bytes(csv.as_bytes(), ".csv").expect("C-V parses past preamble");
    assert_eq!(capacitance, vec![4.6e-11, 7.5e-11, 1.0e-10]);
}

#[test]
fn cv_parser_errors_without_a_capacitance_column() {
    // A transfer-shaped file (no C column) must not be misread as a C-V sweep.
    let csv = "Vg,Id\n1,1e-9\n2,2e-9\n";
    assert!(parse_cv_bytes(csv.as_bytes(), ".csv").is_err());
}

#[test]
fn second_transfer_parser_reads_the_constant_drain_bias() {
    // The B1500A `Id-Vg` shape: vg sweeps while vd is a constant column.
    let mut csv = String::from("DataName, vg, vd, id, is, ig\n");
    for i in 0..20 {
        let vg = -5.0 + 0.5 * i as f64;
        csv.push_str(&format!(
            "DataValue, {vg}, -40, {:e}, 0, 0\n",
            1e-9 * (i + 1) as f64
        ));
    }
    let second =
        parse_second_transfer_bytes(csv.as_bytes(), ".csv").expect("constant-Vd transfer parses");
    assert_eq!(second.v_ds, -40.0, "device-frame sign preserved");
    assert_eq!(second.vg.len(), 20);
    assert!(second.id_abs.iter().all(|&i| i > 0.0), "Id stored as |Id|");
}

#[test]
fn second_transfer_parser_uses_signed_current_when_magnitude_precedes_it() {
    let mut csv = String::from("Vg,Vd,abs_Id,Id\n");
    for i in 0..10 {
        csv.push_str(&format!("{},-40,99,{}\n", i, -(i as f64 + 1.0)));
    }

    let second =
        parse_second_transfer_bytes(csv.as_bytes(), ".csv").expect("constant-Vd transfer parses");

    assert_eq!(second.id_abs, (1..=10).map(f64::from).collect::<Vec<_>>());
}

#[test]
fn second_transfer_parser_rejects_a_varying_drain_column() {
    // A varying Vd is an output sweep, not a transfer — reject with a reason.
    let mut csv = String::from("Vg,Vd,Id\n");
    for i in 0..20 {
        csv.push_str(&format!("5,{},1e-6\n", 0.5 * i as f64));
    }
    let err = parse_second_transfer_bytes(csv.as_bytes(), ".csv").unwrap_err();
    assert!(err.contains("varies"), "{err}");
}

#[test]
fn second_transfer_parser_rejects_a_zero_drain_bias() {
    let mut csv = String::from("Vg,Vd,Id\n");
    for i in 0..20 {
        csv.push_str(&format!("{},0,1e-6\n", i as f64 * 0.5));
    }
    let err = parse_second_transfer_bytes(csv.as_bytes(), ".csv").unwrap_err();
    assert!(err.contains("zero"), "{err}");
}
