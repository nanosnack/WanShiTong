use crate::cli::{ClientArgs, MarksArgs};
use crate::client::client;
use crate::text::print_json;
use wanshitong_api::v1::{Mark, MarkProgress, MarkedFile, MarksQuery};

pub async fn marks(args: MarksArgs) -> anyhow::Result<()> {
    let MarksArgs {
        client: ClientArgs { url, json },
        done,
    } = args;
    let marks = client(&url).list_marks(&MarksQuery { done }).await?;
    if json {
        return print_json(&marks);
    }
    if marks.marks.is_empty() {
        println!("Nothing is marked as needing correction.");
    }
    for (i, mark) in marks.marks.iter().enumerate() {
        if i > 0 {
            println!();
        }
        print_mark(mark);
    }
    Ok(())
}

fn print_mark(mark: &Mark) {
    let Mark {
        uuid,
        label,
        note,
        created_at: _,
        progress,
        files,
    } = mark;
    let progress = match progress {
        MarkProgress::Open => "open",
        MarkProgress::Pending => "pending",
        MarkProgress::Done => "done",
    };
    println!("{uuid}  {progress:<7}  {label}");
    if !note.is_empty() {
        println!("  Note: {note}");
    }
    for MarkedFile { data_source, path } in files {
        println!("  {} {}  {path}", data_source.name, data_source.uuid);
    }
}
