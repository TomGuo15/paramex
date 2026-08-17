# ParamEx

ParamEx is a native Windows desktop app for thin-film transistor (TFT)
characterization. It is a single portable executable with no Python runtime,
installer, account, or network connection.

## Download

1. Open the [GitHub Releases](https://github.com/TomGuo15/paramex/releases) page.
2. Download the Windows ZIP and extract it.
3. Run `ParamEx.exe`.

ParamEx releases are currently unsigned, so Windows may show an
unknown-publisher or SmartScreen warning. Download it only from the official
repository. Startup failures are logged to `%LOCALAPPDATA%\ParamEx\app.log`.

## Workspaces

- **Transfer** extracts threshold voltage, saturation mobility, subthreshold
  swing, on/off ratio, and hysteresis from transfer sweeps. Optional output
  curves add Idsat, output conductance, output resistance, Early voltage, and
  fit quality.
- **TLM** extracts contact resistance from transmission-line-method workbook
  sets and reports the fitted intercept, resistance per contact, slope, and
  fit quality.
- **Model Fit** fits either the AOSTFT/UMEM or Level 62/LTPS compact model and
  exports a Verilog-A model card. Output, C-V, and second-bias transfer data can
  refine the fitted device when available.

Weak or failed fits remain visible with their status; ParamEx does not hide
unfavorable measurements.

## Input data

Transfer and Model Fit accept `.csv`, `.tsv`, `.txt`, `.xlsx`, and `.xls`
files. A transfer sweep needs gate-voltage and drain-current columns such as
`Vg`/`VGS` and `Id`/`IDS`. An output sweep additionally needs drain voltage
such as `Vd`/`VDS`.

TLM loads folders arranged as `group/length_um/*.xlsx`. Each workbook needs a
`List(*)` sheet containing `vg`, `abs_id`, and `abs_is`. A `Setup(*)` sheet may
provide drain voltage; otherwise the value entered in the app is used.

The GUI shows persistent parse and fit errors next to the affected data. CSV
and Verilog-A exports are available from their results headers.

## Build from source

ParamEx is a Rust workspace:

```powershell
git clone https://github.com/TomGuo15/paramex.git
cd paramex
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
.\packaging\windows\build-local-dist.ps1
```

The portable package is written to `target/dist/ParamEx/`. Source code is split
between `crates/paramex-core` for scientific/domain logic and
`crates/paramex-gui` for the desktop app.

## License

ParamEx is released under the [MIT License](LICENSE). The release ZIP also
contains the notices for bundled open-source dependencies.
