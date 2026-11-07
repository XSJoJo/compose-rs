"""Writes compose-ui's locale direction tables from ICU 68.2's data in androidx-main's external/icu.

Usage: python3 -I tools/locale/generate_locale_direction.py crates/compose-ui/src/locale_direction.rs [<androidx-main checkout>]
Replaces everything after the GENERATED marker line in the given file.
"""
import re
import sys

ICU = (sys.argv[2] if len(sys.argv) > 2 else '../androidx-main') + '/external/icu'
LIKELY = ICU + '/icu4c/source/data/misc/likelySubtags.txt'
PROPS = ICU + '/icu4c/source/common/uscript_props.cpp'
VERSION = ICU + '/icu4c/source/common/unicode/uvernum.h'
MARKER = '// ---- Generated'

version = re.search(r'#define U_ICU_VERSION "([^"]+)"', open(VERSION).read()).group(1)
assert version == '68.2', version

# The scripts whose properties have RTL (uscript_props.cpp's `scriptProps`, one line a script).
rtl_scripts = set()
for line in open(PROPS, encoding='utf-8'):
    m = re.match(r'\s*0x[0-9A-Fa-f]+ \|([^,]*),\s*// (\w{4})\s*$', line)
    if m and re.search(r'\bRTL\b', m.group(1)):
        rtl_scripts.add(m.group(2))
assert {'Arab', 'Hebr', 'Syrc', 'Thaa', 'Nkoo', 'Adlm', 'Yezi'} <= rtl_scripts, rtl_scripts
assert not ({'Aran', 'Syre', 'Syrj', 'Syrn', 'Phlv'} & rtl_scripts)

# likelySubtags.txt: `key{"lang_Script_REGION"}`, keys `lang`, `lang_Script`, `lang_REGION`,
# `lang_Script_REGION` (`und` for no language).
likely = {}
text = open(LIKELY, encoding='utf-8-sig').read()
for key, value in re.findall(r'^\s*([A-Za-z0-9_]+)\{"([^"]+)"\}\s*$', text, re.M):
    likely[key] = value
assert len(likely) > 1800, len(likely)


def script_of(value):
    parts = value.split('_')
    assert len(parts) == 3 and len(parts[1]) == 4, value
    return parts[1]


def is_script(subtag):
    return len(subtag) == 4 and subtag.isalpha()


def rtl(key):
    value = likely.get(key)
    return value is not None and script_of(value) in rtl_scripts


languages = set()
regional = {}
for key in likely:
    parts = key.split('_')
    if len(parts) == 1:
        languages.add(key)
    elif len(parts) == 2 and not is_script(parts[1]):
        regional[(parts[0], parts[1])] = rtl(key)
    # Keys with a script: never asked (a locale with a script is decided by it).

rtl_languages = sorted(lang for lang in languages if rtl(lang))
# The regions that change a language's direction from its own entry's (none: left to right).
overrides = sorted(
    (lang, region, value) for (lang, region), value in regional.items() if value != rtl(lang)
)


def rust_strs(items, per_line=10):
    out = []
    line = '   '
    for item in items:
        piece = f' "{item}",'
        if len(line) + len(piece) > 99:
            out.append(line)
            line = '   '
        line += piece
    if line.strip():
        out.append(line)
    return '\n'.join(out)


lines = [
    MARKER + ' once from ICU ' + version + "'s data in androidx-main's external/icu (CLDR 38):",
    "// `icu4c/source/common/uscript_props.cpp`'s scripts with `RTL`, and",
    "// `icu4c/source/data/misc/likelySubtags.txt`'s entries without a script, as D2061 says;",
    "// for another ICU, generated again the same way, not edited. ----",
    '',
    "/// The scripts ICU's `UScript.isRightToLeft` says are written right to left (their",
    '/// properties have `RTL`).',
    'const RIGHT_TO_LEFT_SCRIPTS: &[&str] = &[',
    rust_strs(sorted(rtl_scripts)),
    '];',
    '',
    "/// The languages whose likely script (their own entry's) is written right to left.",
    'const RIGHT_TO_LEFT_LANGUAGES: &[&str] = &[',
    rust_strs(rtl_languages),
    '];',
    '',
    "/// The language and region entries whose likely script's direction isn't the language's own",
    "/// (`und`: a locale naming only its region): whether they're right to left.",
    'const REGIONS_CHANGING_DIRECTION: &[(&str, &str, bool)] = &[',
]
for lang, region, value in overrides:
    lines.append(f'    ("{lang}", "{region}", {"true" if value else "false"}),')
lines.append('];')

path = sys.argv[1]
source = open(path, encoding='utf-8').read()
index = source.index(MARKER)
open(path, 'w', encoding='utf-8').write(source[:index] + '\n'.join(lines) + '\n')
print(len(rtl_scripts), 'scripts,', len(rtl_languages), 'languages,', len(overrides), 'regions',
      file=sys.stderr)
