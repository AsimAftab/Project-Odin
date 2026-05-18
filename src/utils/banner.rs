use colored::Colorize;

pub fn print_banner(active_realm: Option<&str>) {
    let banner = r#"
   ██████╗ ██████╗ ██╗███╗   ██╗
   ██╔═══██╗██╔══██╗██║████╗  ██║
   ██║   ██║██║  ██║██║██╔██╗ ██║
   ██║   ██║██║  ██║██║██║╚██╗██║
   ╚██████╔╝██████╔╝██║██║ ╚████║
    ╚═════╝ ╚═════╝ ╚═╝╚═╝  ╚═══╝
    "#;

    println!("{}", banner.bright_yellow().bold());
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════╗".bright_blue()
    );
    println!(
        "║ {} ║",
        "  Allfather of the dev workstation — see, capture, restore   "
            .bright_white()
            .bold()
    );
    let version_line = format!(
        "  v{}  ·  ᚱ from Hliðskjálf, all nine realms are seen.    ",
        env!("CARGO_PKG_VERSION")
    );
    println!("║ {} ║", version_line.bright_green());
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════╝".bright_blue()
    );

    match active_realm {
        Some(name) => {
            println!(
                "  {}  bound realm: {}",
                "●".bright_green().bold(),
                name.bright_yellow().bold()
            );
        }
        None => {
            println!(
                "  {}  no realm bound — run {} to forge or bind one",
                "○".dimmed(),
                "odin asgard".cyan().bold()
            );
        }
    }
    println!();

    println!("{}", "  ◈ Realms (commands)".bright_yellow().bold());
    println!();

    let commands = vec![
        ("all-eye", "Hliðskjálf — interactive overview", "ᚢ"),
        ("asgard", "Profile realm — selector + editor", "ᚨ"),
        ("snapshot", "Capture this realm into the vault", "ᛒ"),
        ("restore", "Bind this realm to the vault", "ᛞ"),
        ("sync", "Cross the Bifrost — push to GitHub", "ᛯ"),
        ("update", "Renew Mjölnir — install updates", "ᛗ"),
        ("doctor", "Divine broken paths and tools", "ᛟ"),
        ("diff", "Compare realm to vault", "ᛜ"),
        ("ports", "List bound bindings", "ᛇ"),
        ("freeport", "Sever a binding (was `kill`)", "ᚹ"),
        ("ps", "Watch the host of processes", "ᛉ"),
        ("config", "Configure the Bifrost (GitHub)", "ᛏ"),
        ("init", "Forge a fresh vault", "ᚷ"),
    ];

    for (cmd, desc, rune) in commands {
        let padded = format!("{:<10}", cmd);
        println!(
            "  {}  {} {}",
            rune.bright_yellow(),
            padded.cyan().bold(),
            desc.white()
        );
    }

    println!();
    println!("{}", "  🜉 Get started".bright_yellow().bold());
    println!(
        "    odin all-eye        {} ascend to Hliðskjálf",
        "→".bright_green()
    );
    println!(
        "    odin asgard         {} enter the profile realm",
        "→".bright_green()
    );
    println!(
        "    odin snapshot       {} capture this realm",
        "→".bright_green()
    );
    println!("    odin --help         {} all runes", "→".bright_green());
    println!();
    println!(
        "{}",
        "  ᚱ Lore: https://github.com/AsimAftab/Project-Odin".bright_blue()
    );
    println!();
}
