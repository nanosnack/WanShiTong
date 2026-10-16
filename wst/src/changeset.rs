use crate::claims::parse_claim;
use crate::cli::{
    AssertArgs, ChangesetArgs, ChangesetCommand, ClientArgs, ListChangesetsArgs, NewChangesetArgs,
    RetractArgs,
};
use crate::client::client;
use crate::diff::print_diff;
use crate::text::{count, print_json};
use crate::work::print_assertion;
use anyhow::Context;
use uuid::Uuid;
use wanshitong_api::v1::{
    Changeset, ChangesetState, ChangesetSummary, ChangesetsQuery, FileRef, NewAssertion,
    NewAssertions, NewChangeset, NewRetraction,
};

pub async fn changeset(command: ChangesetCommand) -> anyhow::Result<()> {
    match command {
        ChangesetCommand::New(args) => new(args).await,
        ChangesetCommand::List(args) => list(args).await,
        ChangesetCommand::Show(args) => show(args).await,
        ChangesetCommand::Accept(args) => accept(args).await,
        ChangesetCommand::Discard(args) => discard(args).await,
        ChangesetCommand::Diff(args) => diff(args).await,
    }
}

async fn new(args: NewChangesetArgs) -> anyhow::Result<()> {
    let NewChangesetArgs {
        client: ClientArgs { url, json },
        author,
        description,
    } = args;
    let changeset = client(&url)
        .create_changeset(&NewChangeset {
            author,
            description,
        })
        .await?;
    if json {
        return print_json(&changeset);
    }
    println!("{}", changeset.uuid);
    Ok(())
}

async fn list(args: ListChangesetsArgs) -> anyhow::Result<()> {
    let ListChangesetsArgs {
        client: ClientArgs { url, json },
        open,
    } = args;
    let query = ChangesetsQuery {
        state: open.then_some(ChangesetState::Open),
    };
    let changesets = client(&url).list_changesets(&query).await?;
    if json {
        return print_json(&changesets);
    }
    if changesets.changesets.is_empty() {
        println!("No changesets.");
    }
    for ChangesetSummary {
        uuid,
        author,
        description,
        state,
        created_at: _,
        closed_at: _,
        assertion_count,
        retraction_count,
    } in &changesets.changesets
    {
        println!(
            "{uuid}  {:<9}  {author}: {description} ({}, {})",
            state_text(*state),
            count(*assertion_count as i64, "assertion"),
            count(*retraction_count as i64, "retraction"),
        );
    }
    Ok(())
}

async fn show(args: ChangesetArgs) -> anyhow::Result<()> {
    let ChangesetArgs {
        client: ClientArgs { url, json },
        changeset,
    } = args;
    let changeset = client(&url).get_changeset(changeset).await?;
    if json {
        return print_json(&changeset);
    }
    print_changeset(&changeset);
    Ok(())
}

async fn accept(args: ChangesetArgs) -> anyhow::Result<()> {
    let ChangesetArgs {
        client: ClientArgs { url, json },
        changeset,
    } = args;
    let accepted = client(&url).accept_changeset(changeset).await?;
    if json {
        return print_json(&accepted);
    }
    println!(
        "Accepted. Interpretation {} applies it, and is canonical once it succeeds.",
        accepted.interpretation
    );
    Ok(())
}

async fn diff(args: ChangesetArgs) -> anyhow::Result<()> {
    let ChangesetArgs {
        client: ClientArgs { url, json },
        changeset,
    } = args;
    let diff = client(&url).diff_changeset(changeset).await?;
    if json {
        return print_json(&diff);
    }
    print_diff(&diff);
    Ok(())
}

async fn discard(args: ChangesetArgs) -> anyhow::Result<()> {
    let ChangesetArgs {
        client: ClientArgs { url, json },
        changeset,
    } = args;
    let changeset = client(&url).discard_changeset(changeset).await?;
    if json {
        return print_json(&changeset);
    }
    println!("Discarded.");
    Ok(())
}

pub async fn assert(args: AssertArgs) -> anyhow::Result<()> {
    let AssertArgs {
        client: ClientArgs { url, json },
        changeset,
        file,
        rationale,
        confidence,
        claim,
    } = args;
    let files = file
        .chunks(2)
        .map(|file| {
            let [data_source, path] = file else {
                unreachable!("clap takes two values for each --file");
            };
            let data_source = data_source
                .parse::<Uuid>()
                .with_context(|| format!("not a data source UUID: {data_source:?}"))?;
            Ok(FileRef {
                data_source,
                path: path.clone(),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let assertions = NewAssertions {
        assertions: vec![NewAssertion {
            files,
            claim: parse_claim(&claim)?,
            rationale,
            confidence,
        }],
    };
    let created = client(&url).add_assertions(changeset, &assertions).await?;
    if json {
        return print_json(&created);
    }
    for uuid in created.assertions {
        println!("{uuid}");
    }
    Ok(())
}

pub async fn retract(args: RetractArgs) -> anyhow::Result<()> {
    let RetractArgs {
        client: ClientArgs { url, json },
        changeset,
        assertion,
    } = args;
    let changeset = client(&url)
        .retract(changeset, &NewRetraction { assertion })
        .await?;
    if json {
        return print_json(&changeset);
    }
    println!("Retracted.");
    Ok(())
}

fn print_changeset(changeset: &Changeset) {
    let Changeset {
        uuid,
        author,
        description,
        state,
        created_at: _,
        closed_at: _,
        assertions,
        retractions,
    } = changeset;
    println!("Changeset {uuid} ({}) by {author}", state_text(*state));
    if !description.is_empty() {
        println!("{description}");
    }
    if !assertions.is_empty() {
        println!();
        println!("Assertions:");
        for assertion in assertions {
            print_assertion(assertion);
        }
    }
    if !retractions.is_empty() {
        println!();
        println!("Retracts:");
        for uuid in retractions {
            println!("  {uuid}");
        }
    }
}

fn state_text(state: ChangesetState) -> &'static str {
    match state {
        ChangesetState::Open => "open",
        ChangesetState::Accepted => "accepted",
        ChangesetState::Discarded => "discarded",
    }
}
