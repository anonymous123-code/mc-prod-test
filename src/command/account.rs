use anyhow::{Ok, Result};
use helixlauncher_core::auth::account::AccountConfig;

pub async fn list(account_config: &AccountConfig) -> Result<()> {
    let accounts = &account_config.accounts;
    println!("{accounts:?}");
    Ok(())
}

pub async fn add(_accounts: &mut AccountConfig) -> Result<()> {
    todo!()
}

pub async fn switch(_name: Option<String>, _accounts: &mut AccountConfig) -> Result<()> {
    todo!()
}
