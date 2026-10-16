use crate::cli::{ClientArgs, FileArgs};
use crate::client::client;
use crate::text::{interpretation_line, print_json};
use crate::work::{print_assertion, print_directory, print_file};
use wanshitong_api::v1::{FileEvidence, FileQuery};

pub async fn file(args: FileArgs) -> anyhow::Result<()> {
    let FileArgs {
        client: ClientArgs { url, json },
        data_source,
        path,
        interpretation,
        inactive,
        directory_files,
    } = args;
    let query = FileQuery {
        interpretation,
        data_source,
        path,
        directory_files,
    };
    let evidence = client(&url).get_file_evidence(&query).await?;
    if json {
        return print_json(&evidence);
    }

    let FileEvidence {
        interpretation,
        file,
        top_directory,
        assertions,
    } = evidence;
    println!("{}", interpretation_line(&interpretation));
    println!();
    print_file(&file, inactive);
    if let Some(top_directory) = top_directory {
        println!();
        println!("Top directory (* marks this file's directory):");
        let prefix = format!("{}: ", top_directory.data_source.name);
        // No work to say "this movie" of: the file's placements name theirs.
        print_directory(&top_directory.root, &prefix, None, 1);
    }
    if !assertions.is_empty() {
        println!();
        println!("Assertions:");
        for assertion in &assertions {
            print_assertion(assertion);
        }
    }
    Ok(())
}
