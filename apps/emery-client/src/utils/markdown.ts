// Shared markdown renderer — used by DocumentView and WorkItemView.
// Escapes all HTML before applying inline transforms to prevent XSS.

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function applyInline(s: string): string {
  return s
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/\*([^*]+)\*/g, "<em>$1</em>")
    .replace(/`([^`]+)`/g, "<code>$1</code>");
}

export function renderMarkdown(md: string): string {
  const lines = md.split("\n");
  const out: string[] = [];
  let inCodeBlock = false;
  const codeLines: string[] = [];

  for (const line of lines) {
    if (line.startsWith("```")) {
      if (inCodeBlock) {
        out.push(`<pre><code>${escapeHtml(codeLines.join("\n"))}</code></pre>`);
        codeLines.length = 0;
        inCodeBlock = false;
      } else {
        inCodeBlock = true;
      }
      continue;
    }
    if (inCodeBlock) {
      codeLines.push(line);
      continue;
    }

    const escaped = escapeHtml(line);
    if (escaped.startsWith("### ")) {
      out.push(`<h3>${applyInline(escaped.slice(4))}</h3>`);
    } else if (escaped.startsWith("## ")) {
      out.push(`<h2>${applyInline(escaped.slice(3))}</h2>`);
    } else if (escaped.startsWith("# ")) {
      out.push(`<h1>${applyInline(escaped.slice(2))}</h1>`);
    } else if (/^[-*] /.test(escaped)) {
      out.push(`<li>${applyInline(escaped.slice(2))}</li>`);
    } else if (/^\d+\. /.test(escaped)) {
      out.push(`<li>${applyInline(escaped.replace(/^\d+\. /, ""))}</li>`);
    } else if (escaped === "") {
      out.push('<div class="md-spacer"></div>');
    } else {
      out.push(`<p>${applyInline(escaped)}</p>`);
    }
  }

  // Close any unclosed code block
  if (inCodeBlock && codeLines.length > 0) {
    out.push(`<pre><code>${escapeHtml(codeLines.join("\n"))}</code></pre>`);
  }

  return out.join("\n");
}
