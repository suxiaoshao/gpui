#!/usr/bin/env python3
"""Regenerate app-owned SVG, Icon Composer and runtime PNG theme assets (macOS)."""
import copy
import json
from pathlib import Path
import subprocess
import xml.etree.ElementTree as ET

APP = Path(__file__).resolve().parents[1]
ROOT = APP / 'build-assets/icon'
BRAND = APP / 'assets/brand'
BASE = json.loads((ROOT / 'Gupi.icon/icon.json').read_text())
OFFICIAL = (BRAND / 'logo-color.svg').read_text()
# Geometry stays tied to Pi's official paths; flag/gradient styles only change fills.
paths = [node.attrib['d'] for node in ET.fromstring(OFFICIAL)]
shape = ''.join(f'<path d="{p}"/>' for p in paths)
colors = ['#F09082', '#4D9ABF', '#F1BE58']
# Alternate icons keep a colored base in both appearances, including when used
# as a runtime PNG. Only the classic bundle icon uses the system light/dark base.
backgrounds = {
    'classic-gradient': '#B9BEDD',
    'color': '#40516B',
    'color-gradient': '#40516B',
    'pride': '#C6B9DA',
    'ukraine': '#A67AD8',
    'ukraine-gradient': '#A67AD8',
}

# Use only the six Pride flag colors. The silhouette has ten cells, so some
# colors repeat; neighboring cells keep separate fills rather than forming stripes.
pride_cells = [
    (0, 0, '#E40303'), (1, 0, '#FF8C00'), (2, 0, '#FFED00'),
    (0, 1, '#008026'), (2, 1, '#24408E'),
    (0, 2, '#732982'), (1, 2, '#E40303'), (3, 2, '#FF8C00'),
    (0, 3, '#008026'), (3, 3, '#24408E'),
]
grid = [165.29, 282.65, 400.0, 517.36, 634.72]


def pride_svg():
    cells = ''.join(
        f'<rect x="{grid[column]:.2f}" y="{grid[row]:.2f}" '
        f'width="{grid[column + 1] - grid[column]:.2f}" '
        f'height="{grid[row + 1] - grid[row]:.2f}" fill="{color}"/>'
        for column, row, color in pride_cells
    )
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 800">'
            f'<defs><clipPath id="mark">{shape}</clipPath></defs>'
            f'<g clip-path="url(#mark)">{cells}</g></svg>\n')


def color_gradient_svg():
    # Three anchors follow the official mark: coral above, blue at the lower
    # left, gold at the lower right. Ordinary SVG gradients compose the field;
    # no mesh-gradient or renderer-specific filter is needed.
    coral, blue, gold = colors
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 800">
  <defs>
    <clipPath id="mark">{shape}</clipPath>
    <linearGradient id="base" x1="224" y1="517" x2="576" y2="517" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="{blue}"/>
      <stop offset="1" stop-color="{gold}"/>
    </linearGradient>
    <linearGradient id="top" x1="341" y1="205" x2="341" y2="540" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="{coral}"/>
      <stop offset="1" stop-color="{coral}" stop-opacity="0"/>
    </linearGradient>
  </defs>
  <g clip-path="url(#mark)">
    <rect width="800" height="800" fill="url(#base)"/>
    <rect width="800" height="800" fill="url(#top)"/>
  </g>
</svg>
'''


def background_fill(hex_color):
    channels = [int(hex_color[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    return {'automatic-gradient': 'extended-srgb:' + ','.join(
        f'{channel:.5f}' for channel in channels
    ) + ',1.00000'}


variants = [
    ('classic', 'Gupi', None, False),
    ('classic-gradient', 'GupiClassicGradient', ['#292929', '#B8B8B8'], True),
    ('color', 'GupiColor', colors, False),
    ('color-gradient', 'GupiColorGradient', colors, True),
    ('pride', 'GupiPride', [color for _, _, color in pride_cells], False),
    ('ukraine', 'GupiUkraine', ['#0057B7', '#FFDD00'], False),
    # A light cyan midpoint lifts the blue/yellow blend instead of passing
    # through their dull sRGB average. Keep the flag's original endpoint colors.
    ('ukraine-gradient', 'GupiUkraineGradient', ['#0057B7', '#83C7DB', '#FFDD00'], True),
]
xcode = Path(subprocess.check_output(['xcode-select', '-p'], text=True).strip())
ictool = xcode.parent / 'Applications/Icon Composer.app/Contents/Executables/ictool'
for slug, name, palette, gradient in variants:
    icon = ROOT / f'{name}.icon'
    if palette is not None:
        if slug == 'color':
            svg = OFFICIAL
        elif slug == 'color-gradient':
            svg = color_gradient_svg()
        elif slug == 'pride':
            svg = pride_svg()
        elif gradient:
            stops = ''.join(f'<stop offset="{i/(len(palette)-1):.6f}" stop-color="{c}"/>' for i,c in enumerate(palette))
            svg = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 800"><defs><linearGradient id="color" x1="0" y1="165.29" x2="0" y2="634.72" gradientUnits="userSpaceOnUse">{stops}</linearGradient></defs><g fill="url(#color)">{shape}</g></svg>\n'
        else:
            height=469.43/len(palette)
            stripes=''.join(f'<rect x="165" y="{165.29+i*height:.5f}" width="470" height="{height+0.01:.5f}" fill="{c}"/>' for i,c in enumerate(palette))
            svg=f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 800"><defs><clipPath id="mark">{shape}</clipPath></defs><g clip-path="url(#mark)">{stripes}</g></svg>\n'
        (BRAND / f'icon-{slug}.svg').write_text(svg)
        (icon / 'Assets').mkdir(parents=True, exist_ok=True)
        (icon / 'Assets/logo.svg').write_text(svg)
        config = copy.deepcopy(BASE)
        fill = background_fill(backgrounds[slug])
        config['fill-specializations'] = [
            {'value': fill},
            {'appearance': 'dark', 'value': fill},
        ]
        config['groups'][0]['layers'][0].pop('fill-specializations', None)
        (icon / 'icon.json').write_text(json.dumps(config, indent=2)+'\n')
    subprocess.run([str(ictool), str(icon), '--export-preview', 'macOS', 'Default', '512', '512', '1', str(BRAND / f'icon-{slug}.png')], check=True)
(ROOT / 'default-icon').write_text('Gupi\n')
