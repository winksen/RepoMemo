import { useEffect, useRef, useState } from "react";
import { EditorContent, useEditor, useEditorState } from "@tiptap/react";
import type { Editor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "@tiptap/markdown";
import { TaskItem, TaskList } from "@tiptap/extension-list";
import { Placeholder } from "@tiptap/extensions";
import {
  IconBold as Bold,
  IconBraces as CodeBlock,
  IconCode as Code,
  IconHeading as Heading,
  IconItalic as Italic,
  IconLink as LinkIcon,
  IconList as List,
  IconListCheck as Checklist,
  IconListNumbers as ListNumbers,
  IconQuote as Quote,
} from "@tabler/icons-react";

/**
 * WYSIWYG note editor. Text is shown formatted while typing and is stored and
 * emitted as Markdown, so notes stay plain text files that can be indexed.
 */
export function RichNoteEditor({
  onChange,
  placeholder,
  value,
}: {
  onChange: (markdown: string) => void;
  placeholder?: string;
  value: string;
}) {
  const lastEmitted = useRef(value);
  const [linkOpen, setLinkOpen] = useState(false);
  const [linkUrl, setLinkUrl] = useState("https://");

  const editor = useEditor({
    extensions: [
      StarterKit.configure({ link: { openOnClick: false, autolink: true } }),
      TaskList,
      TaskItem.configure({ nested: true }),
      Markdown,
      Placeholder.configure({ placeholder: placeholder ?? "" }),
    ],
    content: value,
    contentType: "markdown",
    editorProps: { attributes: { "aria-label": "Note body", class: "rm-rich-content" } },
    onUpdate: ({ editor: current }) => {
      const markdown = current.isEmpty ? "" : current.getMarkdown();
      lastEmitted.current = markdown;
      onChange(markdown);
    },
  });

  // Clear or replace the content when the parent resets the value (after saving).
  useEffect(() => {
    if (editor && value !== lastEmitted.current) {
      lastEmitted.current = value;
      editor.commands.setContent(value, { contentType: "markdown" });
    }
  }, [editor, value]);

  const active = useEditorState({
    editor,
    selector: ({ editor: current }) => current ? {
      bold: current.isActive("bold"),
      italic: current.isActive("italic"),
      heading: current.isActive("heading"),
      bullet: current.isActive("bulletList"),
      numbered: current.isActive("orderedList"),
      task: current.isActive("taskList"),
      quote: current.isActive("blockquote"),
      code: current.isActive("code"),
      codeBlock: current.isActive("codeBlock"),
      link: current.isActive("link"),
    } : null,
  });

  if (!editor) return null;

  function openLink(current: Editor) {
    setLinkUrl(current.getAttributes("link").href ?? "https://");
    setLinkOpen(true);
  }

  function applyLink(current: Editor) {
    const url = linkUrl.trim();
    if (!url || url === "https://") current.chain().focus().extendMarkRange("link").unsetLink().run();
    else current.chain().focus().extendMarkRange("link").setLink({ href: url }).run();
    setLinkOpen(false);
  }

  const tools: Array<{ label: string; icon: JSX.Element; on: boolean; run: () => void }> = [
    { label: "Bold (Ctrl+B)", icon: <Bold size={16} />, on: !!active?.bold, run: () => editor.chain().focus().toggleBold().run() },
    { label: "Italic (Ctrl+I)", icon: <Italic size={16} />, on: !!active?.italic, run: () => editor.chain().focus().toggleItalic().run() },
    { label: "Heading", icon: <Heading size={16} />, on: !!active?.heading, run: () => editor.chain().focus().toggleHeading({ level: 2 }).run() },
    { label: "Bulleted list", icon: <List size={16} />, on: !!active?.bullet, run: () => editor.chain().focus().toggleBulletList().run() },
    { label: "Numbered list", icon: <ListNumbers size={16} />, on: !!active?.numbered, run: () => editor.chain().focus().toggleOrderedList().run() },
    { label: "Checklist", icon: <Checklist size={16} />, on: !!active?.task, run: () => editor.chain().focus().toggleTaskList().run() },
    { label: "Quote", icon: <Quote size={16} />, on: !!active?.quote, run: () => editor.chain().focus().toggleBlockquote().run() },
    { label: "Inline code", icon: <Code size={16} />, on: !!active?.code, run: () => editor.chain().focus().toggleCode().run() },
    { label: "Code block", icon: <CodeBlock size={16} />, on: !!active?.codeBlock, run: () => editor.chain().focus().toggleCodeBlock().run() },
    { label: "Link (Ctrl+K)", icon: <LinkIcon size={16} />, on: !!active?.link, run: () => openLink(editor) },
  ];

  return (
    <div className="rm-rich-editor" onKeyDown={(event) => {
      if ((event.ctrlKey || event.metaKey) && event.key === "k") { event.preventDefault(); openLink(editor); }
    }}>
      <div aria-label="Formatting" className="rm-rich-toolbar" role="toolbar">
        {tools.map((tool) => (
          <button aria-label={tool.label} aria-pressed={tool.on} className={tool.on ? "on" : ""} key={tool.label} onClick={tool.run} onMouseDown={(event) => event.preventDefault()} title={tool.label} type="button">{tool.icon}</button>
        ))}
      </div>
      {linkOpen ? (
        <div className="rm-rich-link">
          <input aria-label="Link address" autoFocus onChange={(event) => setLinkUrl(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); applyLink(editor); } if (event.key === "Escape") setLinkOpen(false); }} placeholder="https://" value={linkUrl} />
          <button onClick={() => applyLink(editor)} type="button">Apply</button>
          <button onClick={() => setLinkOpen(false)} type="button">Cancel</button>
        </div>
      ) : null}
      <EditorContent editor={editor} />
    </div>
  );
}
