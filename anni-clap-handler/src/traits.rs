use crate::context::Context;

#[cfg(not(feature = "async"))]
pub trait Handler: Sized + Clone + Send + 'static {
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

#[cfg(feature = "async")]
#[async_trait::async_trait]
pub trait Handler: Sized + Clone + 'static {
    async fn run(mut self) -> anyhow::Result<()> {
        self.execute(Default::default()).await
    }

    async fn execute(&mut self, mut ctx: Context) -> anyhow::Result<()> {
        ctx.insert(self.clone());
        self.handle_command(&mut ctx).await?;
        self.handle_subcommand(ctx).await
    }

    async fn handle_command(&mut self, _: &mut Context) -> anyhow::Result<()> {
        Ok(())
    }

    async fn handle_subcommand(&mut self, _: Context) -> anyhow::Result<()> {
        Ok(())
    }
}
