mod cli;
mod commands;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use commands::{
    dsp_render::{run_analyze, run_datasheet, run_render, run_suite_command},
    inspect_validate::{run_bench, run_fuzz, run_inspect, run_screenshot, run_validate},
};

#[global_allocator]
static GLOBAL_ALLOC: hph_rt_check::RtCheckingAllocator =
    hph_rt_check::RtCheckingAllocator(std::alloc::System);

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Inspect { plugin, id } => {
            run_inspect(&plugin, id.as_deref(), cli.json)?;
        }
        Commands::Validate {
            plugin,
            id,
            sample_rate,
        } => {
            run_validate(&plugin, id.as_deref(), sample_rate, cli.json)?;
        }
        Commands::Bench {
            plugin,
            id,
            blocks,
            gui,
        } => {
            run_bench(&plugin, id.as_deref(), blocks, gui, cli.json)?;
        }
        Commands::Render {
            plugin,
            id,
            signal,
            freq,
            amp,
            duration,
            sample_rate,
            block_size,
            input_wav,
            output_wav,
            params,
            note,
        } => {
            run_render(
                &plugin,
                id.as_deref(),
                signal,
                freq,
                amp,
                duration,
                sample_rate,
                block_size,
                input_wav,
                output_wav,
                &params,
                note,
                cli.json,
            )?;
        }
        Commands::Analyze {
            plugin,
            id,
            mode,
            sample_rate,
            freq,
            tail_sec,
            params,
        } => {
            run_analyze(
                &plugin,
                id.as_deref(),
                mode,
                sample_rate,
                freq,
                tail_sec,
                &params,
                cli.json,
            )?;
        }
        Commands::Fuzz {
            plugin,
            id,
            iterations,
            seed,
            max_block_size,
            corrupt_state,
        } => {
            run_fuzz(
                &plugin,
                id.as_deref(),
                iterations,
                seed,
                max_block_size,
                corrupt_state,
                cli.json,
            )?;
        }
        Commands::Screenshot {
            plugin,
            id,
            output,
            warmup_ms,
            width,
            height,
        } => {
            run_screenshot(
                &plugin,
                id.as_deref(),
                &output,
                warmup_ms,
                width,
                height,
                cli.json,
            )?;
        }
        Commands::Suite { action } => {
            run_suite_command(&action)?;
        }
        Commands::Datasheet {
            plugin,
            id,
            output,
            sample_rate,
            sweep_param,
            reference,
            params,
        } => {
            run_datasheet(
                &plugin,
                id.as_deref(),
                &output,
                sample_rate,
                sweep_param,
                reference.as_deref(),
                &params,
                cli.json,
            )?;
        }
        Commands::Mcp => {
            hph_mcp::run_mcp_stdio_server()?;
        }
    }

    Ok(())
}
