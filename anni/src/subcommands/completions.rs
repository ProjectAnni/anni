use clap::{Args, CommandFactory};
use clap_complete::{generate, Shell as CompletionShell};
use anni_clap_handler::handler;
use crate::{AnniArguments, ll};

#[derive(Args, Debug, Clone)]
#[clap(about = ll ! ("completions"))]
#[clap(alias = "comp")]
pub struct CompletionsSubcommand {
    #[clap(arg_enum)]
    #[clap(help = ll ! ("completions-shell"))]
    shell: CompletionShell,
}

#[handler(CompletionsSubcommand)]
fn handle_completions(me: &CompletionsSubcommand) -> anyhow::Result<()> {
    generate(me.shell, &mut AnniArguments::command(), "anni", &mut std::io::stdout().lock());
    Ok(())
}
