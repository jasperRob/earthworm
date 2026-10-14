use std::env;

use tracing::error;

pub fn init() -> color_eyre::Result<()> {
    let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default()
        .panic_section(format!(
            "This is a bug. Consider reporting it at {}",
            env!("CARGO_PKG_REPOSITORY")
        ))
        .capture_span_trace_by_default(false)
        .display_location_section(false)
        .display_env_section(false)
        .into_hooks();
    eyre_hook.install()?;
    std::panic::set_hook(Box::new(move |panic_info| {
        if let Ok(mut t) = crate::tui::Tui::new()
            && let Err(r) = t.exit()
        {
            error!("Unable to exit Terminal: {:?}", r);
        }

        #[cfg(not(debug_assertions))]
        {
            use human_panic::{handle_dump, metadata, print_msg};
            let metadata = metadata!();
            let file_path = handle_dump(&metadata, panic_info);
            // prints human-panic message
            print_msg(file_path, &metadata)
                .expect("human-panic: printing error message to console failed");
            eprintln!("{}", panic_hook.panic_report(panic_info)); // prints color-eyre stack trace to stderr
        }
        let msg = format!("{}", panic_hook.panic_report(panic_info));
        error!("Error: {}", strip_ansi_escapes::strip_str(msg));

        #[cfg(debug_assertions)]
        {
            // Better Panic stacktrace that is only enabled when debugging.
            better_panic::Settings::auto()
                .most_recent_first(false)
                .lineno_suffix(true)
                .verbosity(better_panic::Verbosity::Full)
                .create_panic_handler()(panic_info);
        }

        std::process::exit(libc::EXIT_FAILURE);
    }));
    Ok(())
}
