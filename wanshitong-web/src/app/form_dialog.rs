use crate::ui::{
    button::{ButtonSize, ButtonVariant, button},
    dialog::{
        dialog, dialog_content, dialog_description, dialog_footer, dialog_header, dialog_title,
    },
};
use topcoat::{
    Result,
    view::{Attributes, Child, View, attributes, class, component, view},
};

/// A small button that opens a form in a modal dialog. The button and the
/// dialog's Cancel use HTML invoker commands, so opening it as a modal (the
/// page behind inert, focus kept inside, Escape to close) needs no script.
///
/// Pass the `<form>` as children, with `form_id` as its ID: the dialog's
/// footer holds its submit button. The dialog starts `open` when the form
/// came back with an error, so the error shows where the form was; see
/// ts/dialog.ts. `attrs` are forwarded to the element holding the button, with
/// extra classes added to its classes.
#[component]
pub async fn form_dialog(
    /// The opening button's label.
    trigger: &str,
    title: &str,
    #[default] description: Option<&str>,
    form_id: &str,
    /// The submit button's label.
    submit: &str,
    open: bool,
    #[default] mut attrs: Attributes,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    let dialog_id = format!("{form_id}-dialog");
    let title_id = format!("{form_id}-title");
    Ok(view! {
        <div class=(class!(attrs.remove("class"))) (attrs)>
            button(
                size: ButtonSize::Sm,
                attrs: attributes! {
                    type="button"
                    commandfor=(dialog_id.as_str())
                    command="show-modal"
                },
                (trigger)
            )
            dialog(
                open: false,
                attrs: attributes! {
                    id=(dialog_id.as_str())
                    aria-labelledby=(title_id.as_str())
                    data-open=(open)
                },
                dialog_content(
                    dialog_header(
                        dialog_title(
                            attrs: attributes! { id=(title_id.as_str()) },
                            (title)
                        )
                        if let Some(description) = description {
                            dialog_description((description))
                        }
                    )
                    (child)
                    dialog_footer(
                        button(
                            variant: ButtonVariant::Ghost,
                            attrs: attributes! {
                                type="button"
                                commandfor=(dialog_id.as_str())
                                command="close"
                            },
                            "Cancel"
                        )
                        button(
                            attrs: attributes! { type="submit" form=(form_id) },
                            (submit)
                        )
                    )
                )
            )
        </div>
    })
}
