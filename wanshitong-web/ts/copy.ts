// Registers `copyButton`, the Alpine component for a button that copies the
// text in its `data-copy` attribute, and says so for a moment.

interface CopyButton {
  copied: boolean;
  copy(): Promise<void>;
}

// The Clipboard API only exists in secure contexts (HTTPS or localhost), and
// the server may well be reached over plain HTTP on the network. There, copy
// from a selected, invisible textarea instead.
async function copyText(text: string): Promise<void> {
  if (navigator.clipboard) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.append(textarea);
  textarea.select();
  try {
    if (!document.execCommand("copy")) throw new Error("copy command failed");
  } finally {
    textarea.remove();
  }
}

document.addEventListener("alpine:init", () => {
  Alpine.data("copyButton", (): CopyButton & ThisType<CopyButton & { $root: HTMLElement }> => {
    let reset: number | undefined;
    return {
      copied: false,
      async copy() {
        await copyText(this.$root.dataset.copy ?? "");
        this.copied = true;
        clearTimeout(reset);
        reset = setTimeout(() => (this.copied = false), 2000);
      },
    };
  });
});
