use std::fmt::Write;
/// Holds the parsed command-line configuration.
#[allow(clippy::struct_excessive_bools)]
pub struct Opts {
    pub bin: String,
    pub type_name: String,
    pub filter: Option<String>,
    pub interactive: bool,
    pub show_doc: bool,
    pub goto_def: Option<String>,
    pub open_def: bool,
    pub open_doc: bool,
    pub deps: Option<String>,
}

/// The result of parsing command-line arguments.
pub enum ParseResult {
    Opts(Opts),
    Help(String),
    Version(String),
}

/// Builds the CLI help text.
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
    s
}

/// Parses command-line arguments, returning a [`ParseResult`] instead of calling `process::exit`.
pub fn parse_args() -> Result<ParseResult, String> {
    let bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "rust-meth".to_string());

    let mut args = std::env::args().skip(1);

    let Some(first) = args.next() else {
        return Ok(ParseResult::Help(usage(&bin)));
    };

    if matches!(first.as_str(), "--help" | "-h") {
        return Ok(ParseResult::Help(usage(&bin)));
    }

    if matches!(first.as_str(), "--version" | "-V") {
        return Ok(ParseResult::Version(format!(
            "{bin} {}",
            env!("CARGO_PKG_VERSION")
        )));
    }

    if first.starts_with('-') {
        return Err(format!("unexpected argument '{first}'\n\n{}", usage(&bin)));
    }

    let type_name = first;
    let mut filter = None;
    let mut interactive = false;
    let mut show_doc = false;
    let mut goto_def = None;
    let mut open_def = false;
    let mut open_doc = false;
    let mut deps = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-i" | "--interactive" => interactive = true,
            "-d" | "--doc" => show_doc = true,
            "--gd" => {
                let method = args
                    .next()
                    .ok_or_else(|| "--gd requires a method name".to_string())?;
                goto_def = Some(method);
            }
            "--open" | "-o" => open_def = true,
            "--open-doc" => open_doc = true,
            "--deps" => {
                let dep_str = args.next().ok_or_else(|| {
                    "--deps requires a dependency string (e.g., 'serde_json = \"1.0\"')".to_string()
                })?;
                deps = Some(dep_str);
            }
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

    if open_def && goto_def.is_none() {
        return Err("--open requires --gd <method>".to_string());
    }

    if open_doc && goto_def.is_none() {
        return Err("--open-doc requires --gd <method>".to_string());
    }

    if open_def && open_doc {
        return Err("choose only one of --open or --open-doc".to_string());
    }

    Ok(ParseResult::Opts(Opts {
        bin,
        type_name,
        filter,
        interactive,
        show_doc,
        goto_def,
        open_def,
        open_doc,
        deps,
    }))
}
