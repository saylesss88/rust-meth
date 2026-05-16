use std::process;

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
}

/// Prints the CLI help menu with usage patterns and examples.
pub fn usage(bin: &str) {
    eprintln!("Usage: {bin} <type> [filter] [-i] [--doc] [--gd <method>] [--open] [--open-doc]");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {bin} u8");
    eprintln!("  {bin} String");
    eprintln!("  {bin} \"Vec<i32>\"");
    eprintln!("  {bin} \"HashMap<String,u32>\"");
    eprintln!("  {bin} u8 wrapping                 # fuzzy filter");
    eprintln!("  {bin} u8 -i                       # interactive picker");
    eprintln!("  {bin} u8 --doc                    # show doc comments inline");
    eprintln!("  {bin} u8 checked --doc            # filter + docs");
    eprintln!("  {bin} String --gd len             # print definition location");
    eprintln!("  {bin} u8 --gd checked_add         # go to definition");
    eprintln!("  {bin} u8 --gd checked_add --open  # open in $EDITOR");
    eprintln!("  {bin} u8 --gd checked_add --open-doc  # open in browser");
}

/// Hand-rolls argument parsing to support positional arguments and flags.
pub fn parse_args() -> Result<Opts, String> {
    let bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "rust-meth".to_string());

    let mut args = std::env::args().skip(1);

    let Some(first) = args.next() else {
        usage(&bin);
        process::exit(0);
    };

    if matches!(first.as_str(), "--help" | "-h") {
        usage(&bin);
        process::exit(0);
    }

    if matches!(first.as_str(), "--version" | "-V") {
        println!("{} {}", bin, env!("CARGO_PKG_VERSION"));
        process::exit(0);
    }

    if first.starts_with('-') {
        usage(&bin);
        return Err(format!("unexpected argument '{first}'"));
    }

    let type_name = first;
    let mut filter = None;
    let mut interactive = false;
    let mut show_doc = false;
    let mut goto_def = None;
    let mut open_def = false;
    let mut open_doc = false;

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

    Ok(Opts {
        bin,
        type_name,
        filter,
        interactive,
        show_doc,
        goto_def,
        open_def,
        open_doc,
    })
}
