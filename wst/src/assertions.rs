//! The assertions in effect: listing them, and backing them up as a file
//! that another instance makes them again from.

use crate::cli::{AssertionsCommand, ClientArgs, ExportAssertionsArgs, ImportAssertionsArgs};
use crate::client::client;
use crate::text::{count, print_json};
use crate::work::print_assertion;
use anyhow::Context;
use std::path::Path;
use wanshitong_api::v1::{ActiveAssertions, ImportAssertions, ImportedAssertions};

pub async fn assertions(command: AssertionsCommand) -> anyhow::Result<()> {
    match command {
        AssertionsCommand::List(args) => list(args).await,
        AssertionsCommand::Export(args) => export(args).await,
        AssertionsCommand::Import(args) => import(args).await,
    }
}

async fn list(args: ClientArgs) -> anyhow::Result<()> {
    let ClientArgs { url, json } = args;
    let active = client(&url).list_assertions().await?;
    if json {
        return print_json(&active);
    }
    if active.assertions.is_empty() {
        println!("No assertions are in effect.");
    }
    for assertion in &active.assertions {
        print_assertion(assertion);
    }
    Ok(())
}

async fn export(args: ExportAssertionsArgs) -> anyhow::Result<()> {
    let ExportAssertionsArgs { url, output } = args;
    let active = client(&url).list_assertions().await?;
    let mut json = serde_json::to_string_pretty(&active)?;
    json.push('\n');
    match output {
        Some(path) => {
            std::fs::write(&path, json)
                .with_context(|| format!("couldn't write {}", path.display()))?;
            eprintln!(
                "Wrote {} to {}",
                count(active.assertions.len() as i64, "assertion"),
                path.display()
            );
        }
        None => print!("{json}"),
    }
    Ok(())
}

async fn import(args: ImportAssertionsArgs) -> anyhow::Result<()> {
    let ImportAssertionsArgs {
        client: ClientArgs { url, json },
        file,
        author,
        open,
    } = args;
    let text = std::fs::read_to_string(&file)
        .with_context(|| format!("couldn't read {}", file.display()))?;
    let ActiveAssertions { assertions } = serde_json::from_str(&text).with_context(|| {
        format!(
            "{} isn't a file that `wst assertions export` writes",
            file.display()
        )
    })?;
    if assertions.is_empty() {
        println!("Nothing to import.");
        return Ok(());
    }
    let made = count(assertions.len() as i64, "assertion");

    let client = client(&url);
    let ImportedAssertions { changeset, .. } = client
        .import_assertions(&ImportAssertions {
            author,
            description: format!("Import {made} from {}", file_name(&file)),
            assertions,
        })
        .await?;

    if open {
        let changeset = client.get_changeset(changeset).await?;
        if json {
            return print_json(&changeset);
        }
        println!(
            "Made {made} in changeset {changeset_uuid}, which is left open to review.",
            changeset_uuid = changeset.uuid
        );
        return Ok(());
    }
    let accepted = client.accept_changeset(changeset).await.with_context(|| {
        format!("couldn't accept changeset {changeset}, which holds the {made}")
    })?;
    if json {
        return print_json(&accepted);
    }
    println!(
        "Imported {made}. Interpretation {} applies them, and is canonical once it succeeds.",
        accepted.interpretation
    );
    Ok(())
}

/// The file's name, for a changeset's description, e.g. `assertions.json`.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
