use std::{
    env,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    thread,
};

use sts_verify::{verify_communication_mod_trace_reader, SimRealReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Pass,
    Diverged,
    Incomplete,
    Invalid,
}

struct ResultRow {
    path: PathBuf,
    verdict: Verdict,
    actions: usize,
    detail: String,
}

fn verify(path: &Path) -> ResultRow {
    let result = File::open(path)
        .map_err(|error| error.to_string())
        .and_then(|file| {
            verify_communication_mod_trace_reader(BufReader::new(file))
                .map_err(|error| error.to_string())
        });
    match result {
        Err(detail) => ResultRow {
            path: path.to_owned(),
            verdict: Verdict::Invalid,
            actions: 0,
            detail,
        },
        Ok(report) => classify(path, report),
    }
}

fn classify(path: &Path, report: SimRealReport) -> ResultRow {
    let (verdict, detail) = if let Some(failure) = &report.failure {
        (
            Verdict::Diverged,
            format!("{}: {}", failure.path, failure.reason),
        )
    } else if !report.terminal {
        (
            Verdict::Incomplete,
            "trace ended before game termination".to_owned(),
        )
    } else {
        (Verdict::Pass, String::new())
    };
    ResultRow {
        path: path.to_owned(),
        verdict,
        actions: report.total_actions,
        detail,
    }
}

fn trace_paths(input: &Path) -> Result<Vec<PathBuf>, String> {
    if input.is_file() {
        return Ok(vec![input.to_owned()]);
    }
    if !input.is_dir() {
        return Err(format!("{} is not a file or directory", input.display()));
    }
    let mut paths = fs::read_dir(input)
        .map_err(|error| error.to_string())?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"));
    paths.sort();
    if paths.is_empty() {
        return Err(format!("{} contains no .jsonl traces", input.display()));
    }
    Ok(paths)
}

fn verify_all(paths: &[PathBuf]) -> Vec<ResultRow> {
    let workers = thread::available_parallelism()
        .map_or(1, usize::from)
        .min(24)
        .min(paths.len());
    let next = AtomicUsize::new(0);
    let rows = Mutex::new((0..paths.len()).map(|_| None).collect::<Vec<_>>());
    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(path) = paths.get(index) else { break };
                rows.lock().expect("result lock")[index] = Some(verify(path));
            });
        }
    });
    rows.into_inner()
        .expect("result lock")
        .into_iter()
        .map(|row| row.expect("worker result"))
        .collect()
}

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let Some(input) = args.next() else {
        eprintln!("usage: sts_verify <trace.jsonl|trace-directory>");
        return ExitCode::from(1);
    };
    if args.next().is_some() {
        eprintln!("sts_verify accepts exactly one path");
        return ExitCode::from(1);
    }
    let paths = match trace_paths(Path::new(&input)) {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("INVALID {error}");
            return ExitCode::from(1);
        }
    };
    let rows = verify_all(&paths);
    for row in rows.iter().filter(|row| row.verdict != Verdict::Pass) {
        println!(
            "{:?} {} actions={} {}",
            row.verdict,
            row.path.display(),
            row.actions,
            row.detail
        );
    }
    let pass = rows
        .iter()
        .filter(|row| row.verdict == Verdict::Pass)
        .count();
    let divergent = rows
        .iter()
        .filter(|row| row.verdict == Verdict::Diverged)
        .count();
    let incomplete = rows
        .iter()
        .filter(|row| row.verdict == Verdict::Incomplete)
        .count();
    let invalid = rows
        .iter()
        .filter(|row| row.verdict == Verdict::Invalid)
        .count();
    let actions: usize = rows.iter().map(|row| row.actions).sum();
    println!(
        "traces={} pass={} divergent={} incomplete={} invalid={} actions={}",
        rows.len(),
        pass,
        divergent,
        incomplete,
        invalid,
        actions
    );
    if pass == rows.len() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
