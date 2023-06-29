use anyhow::{Ok, Result};
use helixlauncher_core::auth::account::Account;

pub async fn list(accounts: &Vec<Account>) -> Result<()> {
    println!("{accounts:?}");
    Ok(())
}

pub async fn add(_accounts: &mut Vec<Account>) -> Result<()> {
    todo!()
}

pub(crate) async fn switch(_name: Option<String>, _accounts: &mut Vec<Account>) -> Result<()> {
    todo!()
}
