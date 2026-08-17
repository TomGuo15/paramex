//! Filename pairing rules for Model Fit batch-import actions.

use paramex_core::shared::normalized_file_stem;

const DEVICE_ROLE_SUFFIXES: &[&str] =
    &["-transfer", "-output", "-id-vg", "-id-vd", "-idvg", "-idvd"];

pub(super) fn device_base_name(name: &str) -> String {
    let key = normalized_file_stem(name);
    for suffix in DEVICE_ROLE_SUFFIXES {
        if let Some(base) = key.strip_suffix(suffix) {
            return base.to_string();
        }
    }
    if let Some(base) = key.strip_suffix('o') {
        if base.chars().last().is_some_and(|ch| ch.is_ascii_digit()) {
            return base.to_string();
        }
    }
    key
}

pub(super) fn dibl_pair_key(name: &str) -> String {
    let mut key = normalized_file_stem(name);
    if let Some(timestamp) = key.find(';') {
        key.truncate(timestamp);
        while key.ends_with('-') {
            key.pop();
        }
    }
    let stripped = key
        .split('-')
        .filter(|part| !dibl_role_token(part))
        .collect::<Vec<_>>()
        .join("-");
    if stripped.is_empty() {
        key
    } else {
        stripped
    }
}

fn dibl_role_token(token: &str) -> bool {
    token == "high" || token == "low" || vd_bias_token(token)
}

fn vd_bias_token(token: &str) -> bool {
    let Some(rest) = token.strip_prefix("vd") else {
        return false;
    };
    let rest = rest.strip_suffix('v').unwrap_or(rest);
    !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
}

#[cfg(test)]
mod tests {
    use super::{device_base_name, dibl_pair_key};

    #[test]
    fn device_base_name_strips_transfer_and_output_roles() {
        assert_eq!(device_base_name("dev1_transfer.csv"), "dev1");
        assert_eq!(device_base_name("Wafer3_output.xlsx"), "wafer3");
        assert_eq!(device_base_name("dev A id-vd.txt"), "dev-a");
        assert_eq!(device_base_name("2-6o.xlsx"), "2-6");
        assert_eq!(device_base_name("plain.txt"), "plain");
    }

    #[test]
    fn dibl_pair_key_strips_bias_roles_and_b1500a_timestamps() {
        assert_eq!(
            dibl_pair_key("Id-Vg-high [(1) ; 5_1_2024 4_13_47 PM].csv"),
            "id-vg-[(1)"
        );
        assert_eq!(
            dibl_pair_key("Id-Vg-low [(1) ; 5_1_2024 4_15_21 PM].csv"),
            "id-vg-[(1)"
        );
        assert_eq!(
            dibl_pair_key("Id-Vg-VD40 [(7) ; 5_1_2024.csv"),
            "id-vg-[(7)"
        );
        assert_eq!(
            dibl_pair_key("Id-Vg-VD2V [(7) ; 5_1_2024.csv"),
            "id-vg-[(7)"
        );
    }
}
