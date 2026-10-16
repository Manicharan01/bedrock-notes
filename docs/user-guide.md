# Using Bedrock

[Documentation](README.md) · [Troubleshooting](troubleshooting.md)

## Open your notebook

Visit your server's HTTPS address in a browser. On Windows or Android, enter the same server origin, such as `https://notes.example.org`, and its access key. Do not include a path such as `/notes` or `/api`.

The server owns the confirmed notebook. Native clients need it to load content and confirm saves. Browser/PWA and native clients share the same interface.

## Write and save

Choose **New note**, give it a title, and start typing. **Write** shows Markdown, **Both** shows source and preview, and **Read** focuses on the rendered page. Changes save after a short typing pause. `Saved to SQLite` means the server acknowledged the save; `Saving` or `draft kept` does not.

Edit the title and press Enter or leave the field to rename. Resolved incoming wiki links update in the same transaction. The note actions button (`•••`) offers rename, export, and delete. Deletion asks for confirmation; export first if you need a copy.

Supported Markdown includes headings, bold, italics, lists, task checkboxes, tables, strikethrough, code blocks, and ordinary links. Raw HTML is not executed. Attachments are not supported.

## Connect ideas

```markdown
# A project worth exploring

Build on [[An earlier idea]].
Use [[An earlier idea|a different label]] when the sentence needs one.
Jump to [[An earlier idea#A heading]].

#research #projects/bedrock
```

Typing `[[` offers title suggestions. Following an unresolved link offers to create its note. **Links** opens backlinks, outgoing links, a local graph, and the heading outline. **Atlas** shows connections across the notebook; select a block to open a note or drag to rearrange it. The full graph displays at most 120 notes.

Tags outside code blocks appear under **Collections**. Select a tag to filter the index and select it again, or Clear, to remove that filter. **Daily** opens or creates a note named with today's local date.

## Keyboard and phone navigation

| Shortcut | Action |
|---|---|
| Ctrl/⌘ K | Search notes |
| Ctrl/⌘ N | Create a note |
| Ctrl/⌘ S | Save pending edits |
| Escape | Close Links or the phone's note index |
| `?` outside text fields | Show keyboard shortcuts |
| Tab while editing Markdown | Insert two spaces |

On a phone, the menu button beside the tabs opens the note index. Tap outside it or press Escape to close it. Keyboard navigation starts with a shortcut to the notebook.

## Import, export, and recovery

Open **Settings** to import Markdown files, export the current note, download a JSON notebook backup, or restore a backup. Imports merge into the notebook. If names collide, imported notes receive a suffix and links between imported notes follow those names.

JSON exports include unsaved local drafts. A JSON import is a merge, not a destructive database restore, and does not preserve the server's notebook identity. For an exact restore, follow [SQLite backup guidance](self-hosting.md#concurrent-edits-offline-use-and-backups).

Unsaved drafts are plaintext browser/device storage when available. Clearing site data, switching browser profiles, or uninstalling can remove them. Keep the tab open during a failed save and export if necessary. If another writer changed the notebook, Bedrock preserves the draft and blocks writes. Download drafts before choosing **Reload latest**.

Use **Settings → Switch server / sign out** to choose another notebook. Draft recovery is separated by server origin and notebook UUID.
