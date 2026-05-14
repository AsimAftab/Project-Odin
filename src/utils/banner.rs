use colored::Colorize;

pub fn print_banner() {
    let banner = r#"
   ██████╗ ██████╗ ██╗███╗   ██╗
   ██╔═══██╗██╔══██╗██║████╗  ██║
   ██║   ██║██║  ██║██║██╔██╗ ██║
   ██║   ██║██║  ██║██║██║╚██╗██║
   ╚██████╔╝██████╔╝██║██║ ╚████║
    ╚═════╝ ╚═════╝ ╚═╝╚═╝  ╚═══╝
    "#;

    println!("{}", banner.cyan().bold());
    println!(
        "{}",
        "╔════════════════════════════════════════════════════════════╗".bright_blue()
    );
    println!(
        "║ {} │",
        "Developer Workstation Snapshot & Restore Manager"
            .white()
            .bold()
    );
    let version_line = format!("v{} — Fast • Secure • Reliable", env!("CARGO_PKG_VERSION"));
    println!("║ {} │", version_line.bright_green());
    println!(
        "{}",
        "╚════════════════════════════════════════════════════════════╝".bright_blue()
    );
    println!();

    println!("{}", "📋 Commands:".yellow().bold());
    println!();

    let commands = vec![
        ("Snapshot", "Capture current workstation state"),
        ("Restore", "Restore from saved snapshots"),
        ("Sync", "Push/pull snapshots to GitHub"),
        ("Update", "Check for and install updates"),
        ("Doctor", "Diagnose system health"),
        ("Diff", "Compare current vs saved state"),
        ("Dashboard", "Interactive status overview"),
        ("Config", "Configure GitHub integration"),
        ("Ports", "List listening ports & processes"),
        ("Kill", "Terminate process by port or PID"),
        ("Ps", "Interactive process dashboard (htop-style)"),
        ("Export", "Export snapshots"),
        ("Init", "Initialize Odin"),
    ];

    for (cmd, desc) in commands {
        let padded = format!("{:<12}", cmd);
        println!("  {} {}", padded.cyan().bold(), desc.white());
    }

    println!();
    println!("{}", "🚀 Get Started:".yellow().bold());
    println!(
        "  odin snapshot       {} Create your first snapshot",
        "→".bright_green()
    );
    println!(
        "  odin ports          {} See listening ports",
        "→".bright_green()
    );
    println!(
        "  odin ps             {} Monitor processes",
        "→".bright_green()
    );
    println!("  odin --help         {} More info", "→".bright_green());
    println!();
    println!(
        "{}",
        "📖 Documentation: https://github.com/AsimAftab/Project-Odin".bright_blue()
    );
    println!();
}
