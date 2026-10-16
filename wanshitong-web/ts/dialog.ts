// Opens, as modals, the dialogs marked `data-open`: ones holding a form that
// was submitted and came back with an error. Buttons open and close dialogs
// otherwise, with HTML invoker commands.
//
// Loaded deferred, so the page has been parsed.

document.querySelectorAll<HTMLDialogElement>("dialog[data-open]").forEach((dialog) => dialog.showModal());
