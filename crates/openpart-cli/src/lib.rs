use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use openpart_core::{
    build_plan, ApplyReport, Executor, Plan, PlanAction, PlanInputs,
    RealExecutor, SimulatedExecutor,
};
use openpart_service;
use serde::Serialize;
use std::ffi::OsString;
use std::fs;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(
    name = "openpart-cli",
    version = "0.1.0",
    about = "OpenPart CLI (DiskPart-compatible, safety-first)"
)]
pub struct Cli {
    #[arg(long, default_value = r"\\.\PhysicalDrive0")]
    disk: String,

    #[arg(long)]
    dry_run: bool,

    #[arg(long, default_value_t = 64)]
    chunk_mib: u64,

    #[arg(long)]
    keep_traces: bool,

    #[arg(long)]
    out_dir: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone)]
struct CliContext {
    disk: String,
    dry_run: bool,
    chunk_mib: u64,
    keep_traces: bool,
    out_dir: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    List(ListArgs),
    Resize(ResizeArgs),
    Move(MoveArgs),
    Create(CreateArgs),
    Delete(DeleteArgs),
    Convert(ConvertArgs),
    Assign(AssignArgs),
    Format(FormatArgs),
    Clone(CloneArgs),
    Wipe(WipeArgs),
    Active(ActiveArgs),
    Restore(RestoreArgs),
    Legacy(LegacyArgs),
}

#[derive(Parser, Debug)]
pub struct ListArgs {
    #[arg(long)]
    json: bool,
}

#[derive(Parser, Debug)]
pub struct ResizeArgs {
    target: String,
    size: String,
}

#[derive(Parser, Debug)]
pub struct MoveArgs {
    target: String,
    delta_mib: i64,
}

#[derive(Parser, Debug)]
pub struct CreateArgs {
    fs: String,
    size: String,
    #[arg(long)]
    label: Option<String>,
    #[arg(long)]
    letter: Option<String>,
}

#[derive(Parser, Debug)]
pub struct DeleteArgs {
    target: String,
}

#[derive(Parser, Debug)]
pub struct ConvertArgs {
    from: String,
    to: String,
    disk: String,
}

#[derive(Parser, Debug)]
pub struct AssignArgs {
    target: String,
    letter: String,
}

#[derive(Parser, Debug)]
pub struct FormatArgs {
    target: String,
    fs: String,
    #[arg(long)]
    label: Option<String>,
}

#[derive(Parser, Debug)]
pub struct CloneArgs {
    source: String,
    target: String,
}

#[derive(Parser, Debug)]
pub struct WipeArgs {
    disk: String,
}

#[derive(Parser, Debug)]
pub struct ActiveArgs {
    target: String,
}

#[derive(Parser, Debug)]
pub struct RestoreArgs {
    disk_number: u32,
    snapshot_path: String,
}

#[derive(Parser, Debug)]
pub struct LegacyArgs {
    #[arg(long, default_value = r"\\.\PhysicalDrive0")]
    disk: String,

    #[arg(long)]
    test_vhd: Option<String>,

    #[arg(long)]
    restore_gpt: Option<String>,

    #[arg(long)]
    dry_run: bool,

    #[arg(long)]
    move_unallocated_mib: Option<u64>,

    #[arg(long)]
    move_sectors: Option<u64>,

    #[arg(long)]
    extend_index: Option<u32>,

    #[arg(long)]
    move_index: Option<u32>,

    #[arg(long)]
    free_before_index: Option<u32>,

    #[arg(long)]
    free_after_index: Option<u32>,

    #[arg(long)]
    source_unalloc_lba: Option<u64>,

    #[arg(long, default_value_t = 64)]
    chunk_mib: u64,

    #[arg(long)]
    keep_traces: bool,

    #[arg(long)]
    wipe_old_data: bool,

    #[arg(long, default_value = "C")]
    system_letter: String,

    #[arg(long, default_value = "D")]
    data_letter: String,

    #[arg(long, default_value = "C:\\Users\\new\\AppData\\Local\\Temp\\")]
    out_dir: String,

    #[arg(long)]
    force_resume: bool,

    #[arg(long)]
    operation_type: Option<String>,

    #[arg(long)]
    target_lba: Option<u64>,

    #[arg(long)]
    target_sectors: Option<u64>,

    #[arg(long)]
    format_fs: Option<String>,
}

pub fn run_from<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    let Cli {
        disk,
        dry_run,
        chunk_mib,
        keep_traces,
        out_dir,
        command,
    } = cli;

    let context = CliContext {
        disk,
        dry_run,
        chunk_mib,
        keep_traces,
        out_dir,
    };

    match command {
        Commands::List(args) => list_disks(args),
        Commands::Resize(args) => run_action(&context, action_resize(args)?),
        Commands::Move(args) => run_action(&context, action_move(args)),
        Commands::Create(args) => run_action(&context, action_create(args)?),
        Commands::Delete(args) => run_action(&context, action_delete(args)),
        Commands::Convert(args) => run_action(&context, action_convert(args)),
        Commands::Assign(args) => run_action(&context, action_assign(args)),
        Commands::Format(args) => run_action(&context, action_format(args)),
        Commands::Clone(args) => run_action(&context, action_clone(args)),
        Commands::Wipe(args) => run_action(&context, action_wipe(args)),
        Commands::Active(args) => run_action(&context, action_active(args)),
        Commands::Restore(args) => run_action(&context, action_restore(args)),
        Commands::Legacy(args) => run_legacy(args),
    }
}

fn list_disks(args: ListArgs) -> Result<()> {
    let state = openpart_service::scan_disks().context("failed to scan disks")?;
    if args.json {
        let payload = serde_json::to_string_pretty(&state)?;
        println!("{}", payload);
        return Ok(());
    }

    for disk in &state.disks {
        println!("{}: {} ({:.2} GB, {:?})", disk.label, disk.model, disk.size_gb, disk.style);
        for part in &disk.partitions {
            let letter = part.letter.clone().unwrap_or_else(|| "-".to_string());
            println!(
                "  {:<10} {:<6} {:>8.2} GB  {}",
                part.label, letter, part.size_gb, part.status
            );
        }
    }

    Ok(())
}

fn run_action(context: &CliContext, action: PlanAction) -> Result<()> {
    let mut inputs = PlanInputs::default();
    inputs.action = Some(action);
    inputs.disk = context.disk.clone();
    inputs.dry_run = context.dry_run;
    inputs.chunk_mib = context.chunk_mib;
    inputs.keep_traces = context.keep_traces;
    if let Some(out_dir) = &context.out_dir {
        inputs.out_dir = out_dir.clone();
    }

    run_plan(inputs)
}

fn run_legacy(args: LegacyArgs) -> Result<()> {
    let inputs = PlanInputs {
        action: None,
        disk: args.disk,
        test_vhd: args.test_vhd,
        restore_gpt: args.restore_gpt,
        dry_run: args.dry_run,
        move_unallocated_mib: args.move_unallocated_mib,
        move_sectors: args.move_sectors,
        extend_index: args.extend_index,
        move_index: args.move_index,
        free_before_index: args.free_before_index,
        free_after_index: args.free_after_index,
        source_unalloc_lba: args.source_unalloc_lba,
        chunk_mib: args.chunk_mib,
        keep_traces: args.keep_traces,
        wipe_old_data: args.wipe_old_data,
        system_letter: args.system_letter,
        data_letter: args.data_letter,
        out_dir: args.out_dir,
        force_resume: args.force_resume,
        operation_type: args.operation_type,
        target_lba: args.target_lba,
        target_sectors: args.target_sectors,
        format_fs: args.format_fs,
        allow_battery: false,
    };

    run_plan(inputs)
}

fn run_plan(inputs: PlanInputs) -> Result<()> {
    let plan = build_plan(inputs.clone()).context("no operation specified")?;
    let executor = SimulatedExecutor::new();
    let report = executor.execute(&plan, inputs.dry_run)?;

    print_report(&plan, &report);

    if inputs.keep_traces {
        write_traces(&inputs, &plan, &report)?;
    }

    Ok(())
}

fn action_resize(args: ResizeArgs) -> Result<PlanAction> {
    let size_gb = parse_size_gb(&args.size)?;
    Ok(PlanAction::Resize {
        target: args.target,
        size_gb,
    })
}

fn action_move(args: MoveArgs) -> PlanAction {
    PlanAction::Move {
        target: args.target,
        delta_mib: args.delta_mib,
    }
}

fn action_create(args: CreateArgs) -> Result<PlanAction> {
    let size_gb = parse_size_gb(&args.size)?;
    Ok(PlanAction::Create {
        fs: args.fs,
        size_gb,
        label: args.label,
        drive_letter: args.letter,
    })
}

fn action_delete(args: DeleteArgs) -> PlanAction {
    PlanAction::Delete { target: args.target }
}

fn action_convert(args: ConvertArgs) -> PlanAction {
    PlanAction::Convert {
        from: args.from,
        to: args.to,
        disk: args.disk,
    }
}

fn action_assign(args: AssignArgs) -> PlanAction {
    PlanAction::Assign {
        target: args.target,
        letter: args.letter,
    }
}

fn action_format(args: FormatArgs) -> PlanAction {
    PlanAction::Format {
        target: args.target,
        fs: args.fs,
        label: args.label,
    }
}

fn action_clone(args: CloneArgs) -> PlanAction {
    PlanAction::Clone {
        source: args.source,
        target: args.target,
    }
}

fn action_wipe(args: WipeArgs) -> PlanAction {
    PlanAction::Wipe { disk: args.disk }
}

fn action_active(args: ActiveArgs) -> PlanAction {
    PlanAction::Activate { target: args.target }
}

fn action_restore(args: RestoreArgs) -> PlanAction {
    PlanAction::Restore {
        disk_number: args.disk_number,
        snapshot_path: args.snapshot_path,
    }
}

fn parse_size_gb(raw: &str) -> Result<f64> {
    let value = raw.trim().to_lowercase();
    let (num, unit) = value
        .chars()
        .partition::<String, _>(|ch| ch.is_ascii_digit() || *ch == '.');

    let size: f64 = num.parse().context("invalid size number")?;
    let unit = unit.trim();
    let size_gb = match unit {
        "gb" | "g" | "" => size,
        "mb" | "m" => size / 1024.0,
        "tb" | "t" => size * 1024.0,
        _ => size,
    };

    Ok(size_gb)
}

fn print_report(plan: &Plan, report: &ApplyReport) {
    println!("Operation: {}", plan.operation_type);
    for step in &report.steps {
        println!("- {}", step);
    }

    if !report.warnings.is_empty() {
        eprintln!("Warnings:");
        for warning in &report.warnings {
            eprintln!("- {}", warning);
        }
    }
}

fn write_traces(inputs: &PlanInputs, plan: &Plan, report: &ApplyReport) -> Result<()> {
    let out_dir = Path::new(&inputs.out_dir);
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create out dir {}", out_dir.display()))?;

    let plan_path = out_dir.join("openpart-plan.json");
    let report_path = out_dir.join("openpart-report.json");

    write_json(&plan_path, plan)?;
    write_json(&report_path, report)?;

    Ok(())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let data = serde_json::to_vec_pretty(value)?;
    fs::write(path, data)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}
