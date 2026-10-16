"""Check local documentation links and anchors without dependencies or network access."""
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
FILES = list(ROOT.glob('*.md')) + list((ROOT / 'docs').rglob('*.md')) + list((ROOT / '.github').rglob('*.md'))

def anchors(path):
    used = {}
    result = set()
    text = path.read_text(encoding='utf-8')
    text = re.sub(r'```.*?```', '', text, flags=re.S)
    for title in re.findall(r'^#{1,6}\s+(.+?)\s*#*$', text, re.M):
        slug = re.sub(r'[^\w\- ]', '', title.lower()).replace(' ', '-')
        count = used.get(slug, 0)
        used[slug] = count + 1
        result.add(slug + (f'-{count}' if count else ''))
    return result

failures = []
checked = 0
for source in FILES:
    text = re.sub(r'```.*?```', '', source.read_text(encoding='utf-8'), flags=re.S)
    targets = re.findall(r'!?\[[^\]\n]*\]\(([^\s)]+)(?:\s+"[^"]*")?\)', text)
    targets += re.findall(r'<(?:img|a)\b[^>]*\b(?:src|href)="([^"]+)"', text)
    for target in targets:
        url = urlsplit(target)
        if url.scheme or url.netloc:
            continue
        dest = (source.parent / unquote(url.path)).resolve() if url.path else source
        if not dest.is_relative_to(ROOT):
            failures.append(f'{source.relative_to(ROOT)}: link escapes project: {target}')
        elif not dest.exists():
            failures.append(f'{source.relative_to(ROOT)}: missing {target}')
        elif url.fragment and dest.suffix == '.md' and unquote(url.fragment) not in anchors(dest):
            failures.append(f'{source.relative_to(ROOT)}: missing anchor {target}')
        checked += 1
if failures:
    raise SystemExit('\n'.join(failures))
print(f'Checked {checked} local links in {len(FILES)} documentation files.')
