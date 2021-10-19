use crate::context::Context;

pub trait Handler: Sized + Clone + 'static {
    fn run(mut self) -> anyhow::Result<()> {
        self.execute(Default::default())
    }

    fn execute(&mut self, mut ctx: Context) -> anyhow::Result<()> {
        ctx.insert(self.clone());
        self.handle_command(&mut ctx)?;
        self.handle_subcommand(ctx)
    }

    fn handle_command(&mut self, _: &mut Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn handle_subcommand(&mut self, _: Context) -> anyhow::Result<()> {
        Ok(())
    }
}
