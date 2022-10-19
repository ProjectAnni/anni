use crate::{ll, AnniArguments};
use clap::{Args, CommandFactory};
use clap_complete::{generate, Shell as CompletionShell};
use clap_handler::handler;

#[derive(Args, Debug, Clone)]
#[clap(about = ll!("completions"))]
pub struct CompletionsSubcommand {
    #[clap(value_enum)]
    #[clap(help = ll!("completions-shell"))]
    shell: CompletionShell,
}

#[handler(CompletionsSubcommand)]
fn handle_completions(me: &CompletionsSubcommand) -> anyhow::Result<()> {
    generate(
        me.shell,
        &mut AnniArguments::command(),
        "anni",
        &mut std::io::stdout().lock(),
    );
    Ok(())
}
