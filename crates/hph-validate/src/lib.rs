pub mod benchmark;
pub mod checks;
pub mod fuzz;
pub mod stream;
pub mod types;

pub use benchmark::run_benchmarks;
pub use checks as suite;
pub use checks::{check_headroom_and_overload, run_full_validation};
pub use fuzz::run_fuzz_torture;
pub use stream::{
    infer_channel_counts, infer_multibus_layout, mix_down_output_buses, process_audio_stream,
    MultiBusLayout,
};
pub use types::{
    BenchmarkReport, DspBenchBlockStats, FuzzConfig, FuzzFailure, FuzzReport,
    ValidationCheckResult, ValidationReport,
};
