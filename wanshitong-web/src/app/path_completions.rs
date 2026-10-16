use crate::{styles::MONO, ui::input::input};
use topcoat::{
    Result,
    context::Cx,
    runtime::{Event, expr, shard, signal},
    view::{View, attributes, class, component, view},
};
use wanshitong_app::queries;

/// A text input for a directory path on this server, which suggests the
/// directories that could come next as you type, as a combobox. Without
/// JavaScript, it's a plain text input.
#[component]
pub async fn path_input(
    id: &str,
    name: &str,
    value: &str,
    #[default] placeholder: Option<&str>,
) -> Result<impl View> {
    Ok(view! {
        path_combobox(
            id: id.to_owned(),
            name: name.to_owned(),
            value: value.to_owned(),
            placeholder: placeholder.map(str::to_owned)
        )
    })
}

/// The input and its suggestions: a shard, re-rendered with the directories
/// that could come next as the path is typed, which the server lists.
///
/// Arrow keys pick a suggestion, and Enter or Tab accepts it. With none
/// picked, Tab accepts the first suggestion. Accepting one ends the path with
/// a `/`, so the next level's suggestions follow. Escape hides them.
///
/// The shard has its own endpoint, so its arguments come from the browser:
/// they're only rendered as attributes and completed.
#[shard]
async fn path_combobox(
    cx: &Cx,
    id: String,
    name: String,
    value: String,
    placeholder: Option<String>,
) -> Result<impl View> {
    // The path as typed. Read tracked, so the shard re-renders as it changes.
    let text = signal(cx, || value);
    // Whether the suggestions show, while the input is focused.
    let open = signal(cx, || false);
    // The highlighted suggestion, counted from 1, or 0 for none.
    let active = signal(cx, || 0usize);
    let typed = text.get();
    // Anything the server can't read completes to nothing rather than an error.
    let completions = queries::complete_directory_path(&typed).await;
    let count = completions.len();
    let listbox_id = format!("{id}-completions");
    // Whether the first suggestion only adds the `/` to what's typed: then Tab
    // moves on, for a path that's already typed out.
    let first_is_typed = completions
        .first()
        .is_some_and(|first| *first == format!("{typed}/"));
    // The suggestions' option IDs, by position, with a blank at 0 for none.
    let option_ids: Vec<String> = (0..=count)
        .map(|n| {
            if n == 0 {
                String::new()
            } else {
                format!("{listbox_id}-{n}")
            }
        })
        .collect();
    // Each suggestion with its position from 1, its option's ID and its label:
    // the directory's own name, as the rest of the path is already typed.
    let options: Vec<(usize, String, String, String)> = completions
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let trimmed = path.trim_end_matches('/');
            let label = format!("{}/", trimmed.rsplit('/').next().unwrap_or_default());
            (i + 1, option_ids[i + 1].clone(), path.clone(), label)
        })
        .collect();

    // No early returns: the runtime compiles an `if` to a function of its
    // own, which a `return` inside it only leaves.
    let keydown = expr!(|e: Event| {
        // The arrow keys move the highlight, wrapping through none, back to
        // the input's own value, and show the suggestions if they're hidden.
        let step = if e.key == "ArrowDown" {
            1
        } else if e.key == "ArrowUp" {
            count
        } else {
            0
        };
        if e.key == "Escape" {
            if open.get() {
                e.prevent_default();
                open.set(false);
                active.set(0);
            }
        } else if e.key == "Enter" {
            // With nothing highlighted, Enter submits the form as usual.
            if open.get() {
                if active.get() > 0 {
                    e.prevent_default();
                    text.set(completions[active.get() - 1].to_owned());
                    active.set(0);
                }
            }
        } else if e.key == "Tab" {
            if open.get() {
                if !e.shift_key {
                    if active.get() > 0 {
                        e.prevent_default();
                        text.set(completions[active.get() - 1].to_owned());
                        active.set(0);
                    } else if count > 0 {
                        if !first_is_typed {
                            e.prevent_default();
                            text.set(completions[0].to_owned());
                        }
                    }
                }
            }
        } else if step > 0 {
            if count > 0 {
                e.prevent_default();
                if open.get() {
                    active.set((active.get() + step) % (count + 1));
                } else {
                    open.set(true);
                }
            }
        }
        // Scrolls the suggestion an arrow key highlighted into view.
        let highlighted = if step > 0 { active.get() } else { 0 };
        if highlighted > 0 {
            let _id = option_ids[highlighted].to_owned();
            raw!("document.getElementById(${_id})?.scrollIntoView({ block: 'nearest' })");
        }
    });

    Ok(view! {
        <div class="relative">
            input(
                attrs: attributes! {
                    id=(id.as_str())
                    name=(name.as_str())
                    :value=$(text.get())
                    placeholder=(placeholder.as_deref())
                    class=(MONO)
                    autocomplete="off"
                    spellcheck="false"
                    role="combobox"
                    aria-autocomplete="list"
                    aria-controls=(listbox_id.as_str())
                    :aria-expanded=$(if open.get() { "true" } else { "false" })
                    :aria-activedescendant=$((active.get() > 0).then_some(
                        option_ids[active.get()].to_owned(),
                    ))
                    @focus=$(|_e: Event| open.set(true))
                    @blur=$(|_e: Event| {
                        open.set(false);
                        active.set(0);
                    })
                    @input=$(|e: Event| {
                        text.set(e.target.value);
                        open.set(true);
                        active.set(0);
                    })
                    @keydown=(keydown)
                }
            )
            <ul
                id=(listbox_id.as_str())
                role="listbox"
                class=(class!(
                    "absolute top-full left-0 z-50 mt-1 max-h-64 w-full overflow-y-auto",
                    "rounded-lg border border-border bg-popover p-1 text-popover-foreground",
                    "shadow-sm",
                ))
                :hidden=$(if count == 0 { true } else { !open.get() })
            >
                for (n, option_id, path, label) in options {
                    <li
                        id=(option_id.as_str())
                        role="option"
                        translate="no"
                        title=(path.as_str())
                        class=(class!(
                            "cursor-pointer truncate rounded-md px-2 py-1.5",
                            "aria-selected:bg-foreground/5",
                            MONO,
                        ))
                        :aria-selected=$(if active.get() == n {
                            "true"
                        } else {
                            "false"
                        })
                        @mousedown=$(|e: Event| {
                            // Keeps focus in the input.
                            e.prevent_default();
                            text.set(path);
                            active.set(0);
                        })
                        @mousemove=$(|_e: Event| active.set(n))
                    >
                        (label)
                    </li>
                }
            </ul>
        </div>
    })
}
