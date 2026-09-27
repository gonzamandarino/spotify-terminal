"""Genera el ícono de la app de escritorio (spec 004).

Salida:
  assets/icon.ico      ícono del .exe y de los accesos directos (16-256 px)
  assets/icon-64.rgba  64x64 RGBA crudo, para el ícono de la ventana (se
                       embebe con include_bytes!, sin decodificar PNG/ICO)

Uso: python scripts/generar-icono.py   (requiere Pillow)
Los colores son los de config::theme.
"""

from pathlib import Path

from PIL import Image, ImageDraw

BACKGROUND = (0x12, 0x12, 0x12, 255)
BORDER = (0x2A, 0x2A, 0x2A, 255)
ACCENT = (0x1D, 0xB9, 0x54, 255)

# Se dibuja grande y se achica: bordes suaves en todos los tamaños.
BASE = 1024


def draw() -> Image.Image:
    img = Image.new("RGBA", (BASE, BASE), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    pad = BASE * 0.04
    d.rounded_rectangle(
        (pad, pad, BASE - pad, BASE - pad),
        radius=BASE * 0.22,
        fill=BACKGROUND,
        outline=BORDER,
        width=int(BASE * 0.02),
    )
    # Prompt ">" a la izquierda.
    w = BASE * 0.075
    d.line(
        [(BASE * 0.22, BASE * 0.34), (BASE * 0.40, BASE * 0.50), (BASE * 0.22, BASE * 0.66)],
        fill=ACCENT,
        width=int(w),
        joint="curve",
    )
    # Ecualizador: tres barras de distinto alto.
    bottom = BASE * 0.70
    bar = BASE * 0.085
    for i, height in enumerate([0.22, 0.40, 0.30]):
        x = BASE * 0.50 + i * bar * 1.55
        d.rounded_rectangle(
            (x, bottom - BASE * height, x + bar, bottom),
            radius=bar * 0.35,
            fill=ACCENT,
        )
    return img


def main() -> None:
    assets = Path(__file__).resolve().parent.parent / "assets"
    assets.mkdir(exist_ok=True)
    img = draw()
    img.save(assets / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    small = img.resize((64, 64), Image.LANCZOS)
    (assets / "icon-64.rgba").write_bytes(small.tobytes())


if __name__ == "__main__":
    main()
