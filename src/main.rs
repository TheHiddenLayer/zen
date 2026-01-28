use std::io::{self, stdout, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, TryRecvError};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::{backend::CrosstermBackend, Terminal};

use zen::app::LogicThread;
use zen::config::Config;
use zen::render::RenderState;
use zen::{ui, zlog, Result};

const FRAME_DURATION: Duration = Duration::from_micros(16_666); // 60fps

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
zen - AI coding session manager

USAGE:
    zen [OPTIONS]
    zen reset [--force]   Reset and delete all sessions

OPTIONS:
    -t, --trust     Auto-approve agent prompts
    -d, --debug     Enable debug logging (writes to ~/.zen/zen.log)
    -h, --help      Print help
    -v, --version   Print version

RESET OPTIONS:
    --force         Delete sessions even if they have uncommitted work

ENVIRONMENT:
    ZEN_DEBUG=1     Enable debug logging (alternative to --debug)
";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Check for reset command first
    if !args.is_empty() && args[0] == "reset" {
        let debug = args.iter().any(|a| a == "-d" || a == "--debug");
        zen::log::init_with_debug(debug);
        let force = args.iter().any(|a| a == "--force");
        return run_reset(force);
    }

    for arg in &args {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(());
            }
            "-v" | "--version" => {
                println!("zen {VERSION}");
                return Ok(());
            }
            "-t" | "--trust" => {}
            "-d" | "--debug" => {}
            _ => {
                eprintln!("error: unknown option '{arg}'");
                eprint!("{HELP}");
                std::process::exit(1);
            }
        }
    }

    let trust = args.iter().any(|a| a == "-t" || a == "--trust");
    let debug = args.iter().any(|a| a == "-d" || a == "--debug");
    zen::log::init_with_debug(debug);

    if debug {
        zlog!("Zen starting (debug mode enabled)");
    } else {
        zlog!("Zen starting");
    }

    let mut config = Config::load()?;
    if trust {
        config.trust = true;
    }

    let shutdown = Arc::new(AtomicBool::new(false));
    let render_paused = Arc::new(AtomicBool::new(false));
    let render_acked = Arc::new(AtomicBool::new(false));
    let (state_tx, state_rx) = crossbeam_channel::bounded::<RenderState>(1);

    let shutdown_clone = shutdown.clone();
    let render_paused_clone = render_paused.clone();
    let render_acked_clone = render_acked.clone();
    let logic_handle = thread::spawn(move || {
        LogicThread::run(
            config,
            state_tx,
            shutdown_clone,
            render_paused_clone,
            render_acked_clone,
        )
    });

    let mut terminal = setup_terminal()?;
    let result = render_loop(
        &mut terminal,
        state_rx,
        &shutdown,
        &render_paused,
        &render_acked,
    );

    shutdown.store(true, Ordering::SeqCst);
    let _ = logic_handle.join();
    restore_terminal(&mut terminal)?;
    result
}

fn render_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state_rx: Receiver<RenderState>,
    shutdown: &AtomicBool,
    render_paused: &AtomicBool,
    render_acked: &AtomicBool,
) -> Result<()> {
    let mut state = RenderState::default();
    let mut last_version: u64 = 0;
    let mut last_frame = Instant::now();
    let mut dirty = true;

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        if render_paused.load(Ordering::Acquire) {
            render_acked.store(true, Ordering::Release);
            while render_paused.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(1));
            }
            render_acked.store(false, Ordering::Release);
            terminal.clear()?;
            dirty = true;
            continue;
        }

        match state_rx.try_recv() {
            Ok(s) => {
                dirty = dirty || s.version != last_version;
                state = s;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => break,
        }

        if last_frame.elapsed() < FRAME_DURATION {
            thread::sleep(Duration::from_micros(500));
            continue;
        }
        last_frame = Instant::now();

        if dirty {
            terminal.draw(|f| ui::draw(f, &state))?;
            last_version = state.version;
            dirty = false;
        }
    }
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.hide_cursor()?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    terminal.show_cursor()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(disable_raw_mode()?)
}

fn run_reset(force: bool) -> Result<()> {
    use zen::session::State;

    println!("Resetting zen...");
    zlog!("Reset command initiated (force={})", force);

    let (session_count, tmux_count, worktree_count, branches_count, skipped_sessions) =
        State::reset_all(force)?;

    if !skipped_sessions.is_empty() {
        println!(
            "\nWarning: Skipping {} session(s) with uncommitted work:",
            skipped_sessions.len()
        );
        for name in &skipped_sessions {
            println!("  - {}", name);
        }
        println!("Use 'zen reset --force' to delete these sessions anyway.\n");
    }

    println!("\nReset complete!");
    println!("  Sessions deleted: {}", session_count);
    if !skipped_sessions.is_empty() {
        println!("  Sessions skipped (dirty): {}", skipped_sessions.len());
    }
    println!("  Tmux sessions killed: {}", tmux_count);
    println!("  Worktrees removed: {}", worktree_count);
    println!("  Branches deleted: {}", branches_count);
    zlog!(
        "Reset command completed: {} sessions, {} tmux, {} worktrees, {} branches, {} skipped",
        session_count,
        tmux_count,
        worktree_count,
        branches_count,
        skipped_sessions.len()
    );

    Ok(())
}
