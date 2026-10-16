# Bedrock design language

Bedrock takes its name from Minecraft's bedrock layer. The interface uses layers, square blocks, and a forest-and-stone palette to express a durable personal notebook.

- **Surfaces:** warm stone canvas `#eeefe7`, paper `#fffef9`, dark forest `#263d30`.
- **Accent:** lime `#d6ec6c`, supported by muted mineral greens.
- **Typography:** system sans-serif headings with compact monospace labels. Titles use strong sans-serif type.
- **Mark:** an original arrangement of staggered stone-like layers, rendered as SVG and platform icons.
- **Navigation:** a horizontal Notes / Atlas / Daily bar, a numbered note index, and a focused writing sheet.
- **Connections:** the Links panel opens on demand. The Atlas uses square blocks over a grid.
- **Reading and writing:** Write, Both and Read views share one notebook. New installations open in Read; creating a note opens Write.
- **Responsive behavior:** the notebook index becomes a drawer on phones. The header keeps Notes, Atlas, Daily, Links and Settings available. Viewports at 320 and 390 pixels were checked for horizontal overflow.

The browser, PWA, Windows native client and Android client use the same interface and icon source. See `VERIFICATION.md` for build and UI checks.
