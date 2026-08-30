use std::fmt::Write;

/// Configuration options for specifying execution behavior, queries, and presentation modes.
///
/// `Opts` acts as the primary configuration object driving the analyzer and UI routing layers.
#[allow(clippy::struct_excessive_bools)]
pub struct Opts {
    /// The name of the binary or application executable (used primarily for help headers).
    pub bin: String,

    /// The fully qualified or exact string representation of the type path to analyze
    /// (e.g., `std::collections::HashMap`).
    pub type_name: String,

    /// An optional text pattern used to match and filter down target method names.
    pub filter: Option<String>,

    /// When `true`, instructs the tool to drop into an interactive terminal-based
    /// selection mode rather than outputting text sequentially.
    pub interactive: bool,

    /// When `true`, prints inline documentation summaries directly alongside matching methods
    /// in the default terminal layout.
    pub show_doc: bool,

    /// An optional exact method name targeting "go-to-definition" mechanics.
    /// If populated, it short-circuits standard filtering and visualization paths.
    pub goto_def: Option<String>,

    /// When `true` alongside `goto_def`, attempts to launch the host system's native
    /// editor to display the source code line definition.
    pub open_def: bool,

    /// When `true` alongside `goto_def`, builds a local documentation URL and triggers
    /// the system's default web browser to view it.
    pub open_doc: bool,

    /// Optional paths or metadata defining external dependencies where `rust-analyzer`
    /// should crawl looking for structural definitions.
    pub deps: Option<String>,

    /// When `true`, extracts and prints raw signature implementations or sample usage snippets.
    pub snippet: bool,

    /// When `true`, formats the matched results into a pretty-printed JSON payload
    /// and prints it to standard output.
    pub json: bool,

    /// An optional method name to explain (print full docs for)
    pub explain: Option<String>,

    /// When `true` alongside `explain`, opens the method's full documentation in the browser.
    pub browser: bool,
}

/// The result of parsing command-line arguments.
///
/// Expresses either a successfully built application context or an intentional early exit request
/// such as information printouts.
pub enum ParseResult {
    /// Standard execution branch carrying a fully initialized configuration payload.
    Opts(Opts),

    /// Represents an explicit request for utility instructions. Contains the formatted
    /// string payload destined for standard error output before exiting the process.
    Help(String),

    /// Represents an explicit request for application generation identifiers. Contains
    /// the version string destined for standard output before exiting the process.
    Version(String),

    /// Daemon management command
    Daemon(DaemonSubcommand),
}

/// Daemon management subcommands.
pub enum DaemonSubcommand {
    /// Start the daemon in the foreground.
    Start {
        /// Session TTL in seconds (default: 600).
        ttl_secs: u64,
    },
    /// Stop a running daemon.
    Stop,
    /// Print daemon status.
    Status,
}

/// Builds the CLI help text.
#[must_use]
pub fn usage(bin: &str) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "Usage: {bin} <type> [filter] [-i] [--doc] [--gd <method>] [--open] [--open-doc]\n"
    );
    s.push_str("\nExamples:\n");
    let _ = writeln!(s, "  {bin} u8\n");
    let _ = writeln!(s, "  {bin} String\n");
    let _ = writeln!(s, "  {bin} \"Vec<i32>\"\n");
    let _ = writeln!(s, "  {bin} \"HashMap<String,u32>\"\n");
    let _ = writeln!(s, "  {bin} u8 wrapping                 # fuzzy filter\n");
    let _ = writeln!(
        s,
        "  {bin} u8 -i                       # interactive picker\n"
    );
    let _ = writeln!(
        s,
        "  {bin} u8 --doc                    # show doc comments inline\n"
    );
    let _ = writeln!(s, "  {bin} u8 checked --doc            # filter + docs\n");
    let _ = writeln!(
        s,
        "  {bin} String --gd len             # print definition location\n"
    );
    let _ = writeln!(
        s,
        "  {bin} u8 --gd checked_add         # go to definition\n"
    );
    let _ = writeln!(s, "  {bin} u8 --gd checked_add --open  # open in $EDITOR\n");
    let _ = writeln!(
        s,
        "  {bin} u8 --gd checked_add --open-doc  # open in browser\n"
    );
    s.push_str("\n3rd party crates:\n");
    let _ = writeln!(
        s,
        "  {bin} 'serde_json::Value' --deps 'serde_json = \"1.0\"'\n"
    );
    let _ = writeln!(
        s,
        "  {bin} 'tokio::net::TcpStream' --deps 'tokio = {{ version = \"1.0\", features = [\"net\"] }}'\n"
    );
    let _ = writeln!(
        s,
        "  {bin} 'Result<u8,String>' --explain unwrap_unchecked   # print full docs\n"
    );
    let _ = writeln!(
        s,
        "  {bin} 'Result<u8,String>' --explain unwrap_unchecked --browser  # open in browser\n"
    );

    let _ = writeln!(
        s,
        "  {bin} --daemon start              # start the daemon\n"
    );
    let _ = writeln!(
        s,
        "  {bin} --daemon start --ttl 300   # start with 5 min TTL\n"
    );
    let _ = writeln!(s, "  {bin} --daemon stop               # stop the daemon\n");
    let _ = writeln!(
        s,
        "  {bin} --daemon status             # show daemon status\n"
    );
    s
}

/// Parses the environment command-line arguments into a structured [`ParseResult`].
///
/// # Errors
///
/// Returns an `Err(String)` containing utility descriptions if:
/// * An unexpected flag or a third/subsequent unflagged positional argument is supplied.
/// * Sub-commands demanding parameter arguments (like `--gd` or `--deps`) run out of argument tokens.
/// * Mutually exclusive or poorly anchored flag arrangements are passed (e.g., using `--open` or
///   `--open-doc` without also specifying a targeting `--gd` method, or invoking both simultaneously).
pub fn parse_args(version: &str) -> Result<ParseResult, String> {
    let bin = current_bin();
    let mut args = std::env::args().skip(1);

    let Some(first) = args.next() else {
        return Ok(ParseResult::Help(usage(&bin)));
    };

    if let Some(result) = check_early_exits(&bin, version, &first) {
        return result;
    }

    if first == "--daemon" {
        return parse_daemon_subcommand(&mut args);
    }

    if first.starts_with('-') {
        return Err(format!("unexpected argument '{first}'\n\n{}", usage(&bin)));
    }

    let opts = parse_flags(first, &mut args)?;
    validate_opts(opts, &bin)
}

/// Returns the name of the current executable.
fn current_bin() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "rust-meth".to_string())
}

/// Checks for --help, --version early exits.
fn check_early_exits(bin: &str, version: &str, first: &str) -> Option<Result<ParseResult, String>> {
    match first {
        "--help" | "-h" => Some(Ok(ParseResult::Help(usage(bin)))),
        "--version" | "-V" => Some(Ok(ParseResult::Version(format!("{bin} {version}")))),
        _ => None,
    }
}

/// Parses --daemon <subcommand> [--ttl <secs>].
fn parse_daemon_subcommand(args: &mut impl Iterator<Item = String>) -> Result<ParseResult, String> {
    let sub = args
        .next()
        .ok_or_else(|| "--daemon requires a subcommand: start, stop, status".to_string())?;
    match sub.as_str() {
        "start" => {
            let ttl_secs = args
                .next()
                .filter(|a| a == "--ttl")
                .and_then(|_| args.next())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(600);
            Ok(ParseResult::Daemon(DaemonSubcommand::Start { ttl_secs }))
        }
        "stop" => Ok(ParseResult::Daemon(DaemonSubcommand::Stop)),
        "status" => Ok(ParseResult::Daemon(DaemonSubcommand::Status)),
        other => Err(format!(
            "unknown daemon subcommand '{other}'. Expected: start, stop, status"
        )),
    }
}

/// Parses all flags after the type name positional argument.
fn parse_flags(type_name: String, args: &mut impl Iterator<Item = String>) -> Result<Opts, String> {
    let mut filter = None;
    let mut interactive = false;
    let mut show_doc = false;
    let mut goto_def = None;
    let mut open_def = false;
    let mut open_doc = false;
    let mut deps = None;
    let mut snippet = false;
    let mut json = false;
    let mut explain = None;
    let mut browser = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-i" | "--interactive" => interactive = true,
            "-d" | "--doc" => show_doc = true,
            "--gd" => {
                goto_def = Some(
                    args.next()
                        .ok_or_else(|| "--gd requires a method name".to_string())?,
                );
            }
            "--open" | "-o" => open_def = true,
            "--open-doc" => open_doc = true,
            "--deps" => {
                deps = Some(args.next().ok_or_else(|| {
                    "--deps requires a dependency string (e.g., 'serde_json = \"1.0\"')".to_string()
                })?);
            }
            "--snippet" => snippet = true,
            "--json" => json = true,
            "--explain" => {
                explain = Some(
                    args.next()
                        .ok_or_else(|| "--explain requires a method name".to_string())?,
                );
            }
            "--browser" => browser = true,
            _ if arg.starts_with('-') => {
                return Err(format!("unexpected flag '{arg}'"));
            }
            _ => {
                if filter.is_none() {
                    filter = Some(arg);
                } else {
                    return Err(format!("unexpected argument '{arg}'"));
                }
            }
        }
    }

    if interactive {
        filter = None;
    }

    Ok(Opts {
        bin: current_bin(),
        type_name,
        filter,
        interactive,
        show_doc,
        goto_def,
        open_def,
        open_doc,
        deps,
        snippet,
        json,
        explain,
        browser,
    })
}

/// Validates flag combinations after parsing.
fn validate_opts(opts: Opts, _bin: &str) -> Result<ParseResult, String> {
    if opts.open_def && opts.goto_def.is_none() {
        return Err("--open requires --gd <method>".to_string());
    }
    if opts.open_doc && opts.goto_def.is_none() {
        return Err("--open-doc requires --gd <method>".to_string());
    }
    if opts.open_def && opts.open_doc {
        return Err("choose only one of --open or --open-doc".to_string());
    }
    if opts.browser && opts.explain.is_none() {
        return Err("--browser requires --explain <method>".to_string());
    }
    Ok(ParseResult::Opts(opts))
}
