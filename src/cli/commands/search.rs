use crate::cli::commands::CliContext;
use crate::cli::CliError;

pub struct SearchCommand {
    pub query: String,
    pub limit: usize,
    pub offset: usize,
}

#[async_trait::async_trait]
impl crate::cli::commands::CliCommand for SearchCommand {
    async fn execute(&self, _ctx: &CliContext) -> Result<(), CliError> {
        println!("Searching for: {} (limit: {}, offset: {})", self.query, self.limit, self.offset);
        // TODO: Implement search functionality
        Ok(())
    }
}
